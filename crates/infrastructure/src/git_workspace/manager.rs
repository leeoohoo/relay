use std::{
    collections::HashMap,
    fs::{self, File},
    path::{Path, PathBuf},
};

use fs2::FileExt;
use serde::Deserialize;
use uuid::Uuid;

use ai_chat_domain::company::CompanyProjectGitConfig;
use ai_chat_shared::{AppError, AppResult};

use crate::git_credentials::{
    is_github_token_profile, is_managed_token_profile, GitCredentialStore,
};

use super::command::{ref_exists, run_git};
use super::validation::{
    absolute_path, file_error, path_string, sanitize_branch_component,
    validate_profile_environment, validate_profile_name, validate_project_root,
};
use super::{
    agent_worktree_has_no_user_files, archive_legacy_git_marker, attach_detached_agent_repository,
    configure_agent_repository, ensure_origin_remote, migrate_legacy_linked_worktree,
    preferred_remote_base, remote_default_ref, AGENT_GIT_DIR_NAME,
};

#[derive(Debug, Clone)]
pub struct GitWorkspaceManager {
    pub(super) general_workspace_root: PathBuf,
    pub(super) allowed_local_roots: Vec<PathBuf>,
    pub(super) auth_profiles: HashMap<String, GitAuthProfile>,
    pub(super) credential_store: GitCredentialStore,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub(super) struct GitAuthProfile {
    #[serde(default)]
    env: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct PreparedGitWorkspace {
    pub path: PathBuf,
    pub worktree_key: String,
    pub branch: String,
    pub auth_environment: HashMap<String, String>,
}

impl GitWorkspaceManager {
    pub fn from_env() -> AppResult<Self> {
        let general_workspace_root = std::env::var("AGENT_TRIGGER_GENERAL_WORKSPACE_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(".relay-agent-trigger/general"));
        let general_workspace_root = absolute_path(general_workspace_root)?;
        let allowed_local_roots = std::env::var("AGENT_TRIGGER_ALLOWED_LOCAL_ROOTS")
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .map(|path| {
                if path.is_absolute() {
                    Ok(path)
                } else {
                    Err(AppError::Validation(
                        "AGENT_TRIGGER_ALLOWED_LOCAL_ROOTS entries must be absolute paths".into(),
                    ))
                }
            })
            .collect::<AppResult<Vec<_>>>()?;
        let auth_profiles = std::env::var("AGENT_TRIGGER_GIT_AUTH_PROFILES_JSON")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(|value| {
                serde_json::from_str::<HashMap<String, GitAuthProfile>>(&value).map_err(|error| {
                    AppError::Validation(format!(
                        "invalid AGENT_TRIGGER_GIT_AUTH_PROFILES_JSON: {error}"
                    ))
                })
            })
            .transpose()?
            .unwrap_or_default();
        for (name, profile) in &auth_profiles {
            validate_profile_name(name)?;
            validate_profile_environment(&profile.env)?;
        }
        let credential_store = GitCredentialStore::from_env()?;
        Ok(Self {
            general_workspace_root,
            allowed_local_roots,
            auth_profiles,
            credential_store,
        })
    }

    pub fn prepare_project_workspace(
        &self,
        _company_id: Uuid,
        project_id: Uuid,
        agent_id: Uuid,
        agent_handle: &str,
        git: &CompanyProjectGitConfig,
    ) -> AppResult<PreparedGitWorkspace> {
        let project_root = validate_project_root(&git.host_local_path, &self.allowed_local_roots)?;
        let relay_root = project_root.join(".relay");
        fs::create_dir_all(&relay_root).map_err(file_error)?;
        let lock_path = relay_root.join("workspace.lock");
        let lock = File::options()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&lock_path)
            .map_err(file_error)?;
        lock.lock_exclusive().map_err(file_error)?;

        let auth_environment = self.auth_environment(git.auth_profile.as_deref())?;
        let worktree_path = relay_root.join("worktrees").join(agent_id.to_string());
        let inbox_branch = format!(
            "{}{}/inbox",
            git.branch_prefix,
            sanitize_branch_component(agent_handle)
        );
        let git_marker = worktree_path.join(".git");
        let agent_git_dir = worktree_path.join(AGENT_GIT_DIR_NAME);
        if git_marker.exists() && agent_git_dir.exists() {
            archive_legacy_git_marker(
                &relay_root.join("legacy-gitlinks"),
                &worktree_path,
                agent_id,
            )?;
        }
        if git_marker.is_dir() {
            fs::rename(&git_marker, &agent_git_dir).map_err(file_error)?;
        }
        let repository_environment =
            agent_repository_environment(&auth_environment, &worktree_path)?;
        let branch = if git_marker.is_file() {
            migrate_legacy_linked_worktree(
                &relay_root,
                &worktree_path,
                agent_id,
                &inbox_branch,
                &git.default_branch,
                &git.remote_url,
                &repository_environment,
            )?;
            inbox_branch
        } else if agent_git_dir.is_dir() {
            prepare_existing_agent_repository(
                &worktree_path,
                &inbox_branch,
                &git.default_branch,
                &git.branch_prefix,
                &git.remote_url,
                &repository_environment,
            )?
        } else {
            if worktree_path.exists()
                && fs::read_dir(&worktree_path)
                    .map_err(file_error)?
                    .next()
                    .is_some()
            {
                return Err(AppError::Conflict(
                    "Agent worktree path exists and is not an initialized Git worktree".into(),
                ));
            }
            if let Some(parent) = worktree_path.parent() {
                fs::create_dir_all(parent).map_err(file_error)?;
            }
            if let Err(error) = clone_agent_repository(
                &worktree_path,
                &inbox_branch,
                &git.default_branch,
                &git.remote_url,
                &auth_environment,
                &repository_environment,
            ) {
                if is_git_authentication_error(&error) {
                    cleanup_failed_clone(&worktree_path)?;
                }
                return Err(error);
            }
            inbox_branch
        };

        let worktree_key = format!("{project_id}/{agent_id}");
        drop(lock);
        Ok(PreparedGitWorkspace {
            path: worktree_path,
            worktree_key,
            branch,
            auth_environment: repository_environment,
        })
    }

    pub fn prepare_general_workspace(
        &self,
        company_id: Uuid,
        agent_id: Uuid,
    ) -> AppResult<PreparedGitWorkspace> {
        let path = self
            .general_workspace_root
            .join(company_id.to_string())
            .join(agent_id.to_string());
        fs::create_dir_all(&path).map_err(file_error)?;
        let legacy_git_dir = path.join(".git");
        let agent_git_dir = path.join(AGENT_GIT_DIR_NAME);
        if legacy_git_dir.exists() && agent_git_dir.exists() {
            archive_legacy_git_marker(
                &self.general_workspace_root.join("legacy-gitlinks"),
                &path,
                agent_id,
            )?;
        }
        if legacy_git_dir.is_dir() && !agent_git_dir.exists() {
            fs::rename(&legacy_git_dir, &agent_git_dir).map_err(file_error)?;
        }
        let repository_environment = agent_repository_environment(&HashMap::new(), &path)?;
        if !agent_git_dir.exists() {
            fs::create_dir_all(&agent_git_dir).map_err(file_error)?;
            run_git(Some(&path), &repository_environment, ["init".into()])?;
        }
        Ok(PreparedGitWorkspace {
            worktree_key: format!("_inbox/{company_id}/{agent_id}"),
            path,
            branch: "inbox".into(),
            auth_environment: repository_environment,
        })
    }

    pub fn credential_store(&self) -> &GitCredentialStore {
        &self.credential_store
    }

    fn auth_environment(&self, profile_name: Option<&str>) -> AppResult<HashMap<String, String>> {
        let Some(profile_name) = profile_name else {
            return Ok(HashMap::new());
        };
        if profile_name == "public" {
            return Ok(HashMap::new());
        }
        if is_github_token_profile(profile_name) || is_managed_token_profile(profile_name) {
            return self.credential_store.auth_environment(profile_name);
        }
        self.auth_profiles
            .get(profile_name)
            .map(|profile| profile.env.clone())
            .ok_or_else(|| {
                AppError::Validation(format!(
                    "Git auth profile {profile_name} is not registered on the Trigger host"
                ))
            })
    }
}

pub fn is_git_authentication_error(error: &AppError) -> bool {
    let message = error.to_string().to_ascii_lowercase();
    [
        "the requested url returned error: 401",
        "the requested url returned error: 403",
        "authentication failed",
        "could not read username",
        "invalid username or password",
        "http basic: access denied",
    ]
    .iter()
    .any(|pattern| message.contains(pattern))
}

fn cleanup_failed_clone(worktree_path: &Path) -> AppResult<()> {
    if !worktree_path.exists() {
        return Ok(());
    }
    fs::remove_dir_all(worktree_path).map_err(file_error)
}

fn clone_agent_repository(
    worktree_path: &Path,
    branch: &str,
    default_branch: &str,
    remote_url: &str,
    clone_environment: &HashMap<String, String>,
    repository_environment: &HashMap<String, String>,
) -> AppResult<()> {
    run_git(
        None,
        clone_environment,
        [
            "clone".into(),
            "--no-checkout".into(),
            remote_url.into(),
            path_string(worktree_path)?,
        ],
    )?;
    let git_dir = worktree_path.join(AGENT_GIT_DIR_NAME);
    fs::rename(worktree_path.join(".git"), &git_dir).map_err(file_error)?;
    let remote_branch_ref = format!("refs/remotes/origin/{branch}");
    if ref_exists(&git_dir, repository_environment, &remote_branch_ref)? {
        run_git(
            Some(worktree_path),
            repository_environment,
            [
                "switch".into(),
                "--track".into(),
                "-c".into(),
                branch.into(),
                format!("origin/{branch}"),
            ],
        )?;
    } else {
        let default_ref = remote_default_ref(&git_dir, repository_environment, default_branch)?;
        run_git(
            Some(worktree_path),
            repository_environment,
            ["switch".into(), "-c".into(), branch.into(), default_ref],
        )?;
    }
    configure_agent_repository(worktree_path, branch, repository_environment)
}

fn prepare_existing_agent_repository(
    worktree_path: &Path,
    branch: &str,
    default_branch: &str,
    branch_prefix: &str,
    remote_url: &str,
    environment: &HashMap<String, String>,
) -> AppResult<String> {
    ensure_origin_remote(worktree_path, remote_url, environment)?;
    run_git(
        Some(worktree_path),
        environment,
        ["fetch".into(), "--prune".into(), "origin".into()],
    )?;
    let current_branch = run_git(
        Some(worktree_path),
        environment,
        ["branch".into(), "--show-current".into()],
    )?;
    if current_branch.trim().is_empty() {
        return attach_detached_agent_repository(worktree_path, branch, environment);
    }
    let git_dir = worktree_path.join(AGENT_GIT_DIR_NAME);
    if current_branch.trim() == default_branch {
        let local_default_ref = format!("refs/heads/{default_branch}");
        if !ref_exists(&git_dir, environment, &local_default_ref)?
            && agent_worktree_has_no_user_files(worktree_path)?
        {
            let base_ref = preferred_remote_base(&git_dir, environment, branch, default_branch)?;
            run_git(
                Some(worktree_path),
                environment,
                ["switch".into(), "-c".into(), branch.into(), base_ref],
            )?;
            configure_agent_repository(worktree_path, branch, environment)?;
            return Ok(branch.to_owned());
        }
        return Err(AppError::Conflict(
            "existing Agent repository is checked out on the protected default branch".into(),
        ));
    }
    if current_branch.trim() != branch {
        if current_branch.trim().starts_with(branch_prefix) {
            let current_branch = current_branch.trim().to_owned();
            configure_agent_repository(worktree_path, &current_branch, environment)?;
            return Ok(current_branch);
        }
        return Err(AppError::Conflict(format!(
            "existing Agent repository is checked out on unexpected branch {} outside the configured Agent branch prefix {}",
            current_branch.trim(),
            branch_prefix
        )));
    }
    if !ref_exists(&git_dir, environment, &format!("refs/heads/{branch}"))? {
        let base_ref = preferred_remote_base(&git_dir, environment, branch, default_branch)?;
        run_git(
            Some(worktree_path),
            environment,
            ["reset".into(), "--mixed".into(), base_ref],
        )?;
    }
    configure_agent_repository(worktree_path, branch, environment)?;
    Ok(branch.to_owned())
}

pub(super) fn agent_repository_environment(
    base: &HashMap<String, String>,
    worktree_path: &Path,
) -> AppResult<HashMap<String, String>> {
    let mut environment = base.clone();
    environment.insert(
        "GIT_DIR".into(),
        path_string(&worktree_path.join(AGENT_GIT_DIR_NAME))?,
    );
    environment.insert("GIT_WORK_TREE".into(), path_string(worktree_path)?);
    Ok(environment)
}
