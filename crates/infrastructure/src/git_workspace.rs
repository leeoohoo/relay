use std::{
    collections::HashMap,
    fs::{self, File},
    path::{Path, PathBuf},
    process::{Command, Output},
    thread,
    time::Duration,
};

use fs2::FileExt;
use serde::Deserialize;
use uuid::Uuid;

use ai_chat_domain::company::CompanyProjectGitConfig;
use ai_chat_shared::{AppError, AppResult};

use crate::git_credentials::{
    is_github_token_profile, is_managed_token_profile, GitCredentialStore,
};

const AGENT_GIT_DIR_NAME: &str = ".relay-git";

#[derive(Debug, Clone)]
pub struct GitWorkspaceManager {
    general_workspace_root: PathBuf,
    allowed_local_roots: Vec<PathBuf>,
    auth_profiles: HashMap<String, GitAuthProfile>,
    credential_store: GitCredentialStore,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct GitAuthProfile {
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
            return Err(AppError::Conflict(
                "Agent worktree contains both .git and .relay-git metadata".into(),
            ));
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
            clone_agent_repository(
                &worktree_path,
                &inbox_branch,
                &git.default_branch,
                &git.remote_url,
                &auth_environment,
                &repository_environment,
            )?;
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
            return Err(AppError::Conflict(
                "general Agent workspace contains both .git and .relay-git metadata".into(),
            ));
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

fn attach_detached_agent_repository(
    worktree_path: &Path,
    inbox_branch: &str,
    environment: &HashMap<String, String>,
) -> AppResult<String> {
    let head = run_git(
        Some(worktree_path),
        environment,
        ["rev-parse".into(), "HEAD".into()],
    )?;
    let short_head = run_git(
        Some(worktree_path),
        environment,
        ["rev-parse".into(), "--short=12".into(), "HEAD".into()],
    )?;
    let branch_namespace = inbox_branch.strip_suffix("/inbox").unwrap_or(inbox_branch);
    let mut recovery_branch = format!("{branch_namespace}/recovery-{}", short_head.trim());
    let git_dir = worktree_path.join(AGENT_GIT_DIR_NAME);
    let mut recovery_ref = format!("refs/heads/{recovery_branch}");
    if ref_exists(&git_dir, environment, &recovery_ref)? {
        let existing_head = run_git(
            Some(worktree_path),
            environment,
            ["rev-parse".into(), recovery_ref.clone()],
        )?;
        if existing_head.trim() != head.trim() {
            recovery_branch = format!("{branch_namespace}/recovery-{}", head.trim());
            recovery_ref = format!("refs/heads/{recovery_branch}");
        }
    }
    if ref_exists(&git_dir, environment, &recovery_ref)? {
        let existing_head = run_git(
            Some(worktree_path),
            environment,
            ["rev-parse".into(), recovery_ref],
        )?;
        if existing_head.trim() != head.trim() {
            return Err(AppError::Conflict(format!(
                "cannot attach detached Agent repository because recovery branch {recovery_branch} points to a different commit"
            )));
        }
        run_git(
            Some(worktree_path),
            environment,
            ["switch".into(), recovery_branch.clone()],
        )?;
    } else {
        run_git(
            Some(worktree_path),
            environment,
            ["switch".into(), "-c".into(), recovery_branch.clone()],
        )?;
    }
    configure_agent_repository(worktree_path, &recovery_branch, environment)?;
    Ok(recovery_branch)
}

fn agent_worktree_has_no_user_files(worktree_path: &Path) -> AppResult<bool> {
    for entry in fs::read_dir(worktree_path).map_err(file_error)? {
        let entry = entry.map_err(file_error)?;
        if entry.file_name() != AGENT_GIT_DIR_NAME {
            return Ok(false);
        }
    }
    Ok(true)
}

fn migrate_legacy_linked_worktree(
    relay_root: &Path,
    worktree_path: &Path,
    agent_id: Uuid,
    branch: &str,
    default_branch: &str,
    remote_url: &str,
    environment: &HashMap<String, String>,
) -> AppResult<()> {
    let backup_root = relay_root.join("legacy-gitlinks");
    fs::create_dir_all(&backup_root).map_err(file_error)?;
    let backup_path = backup_root.join(format!("{}-{}.gitlink", agent_id, Uuid::new_v4().simple()));
    fs::rename(worktree_path.join(".git"), backup_path).map_err(file_error)?;
    fs::create_dir_all(worktree_path.join(AGENT_GIT_DIR_NAME)).map_err(file_error)?;
    run_git(
        Some(worktree_path),
        environment,
        ["init".into(), "-b".into(), branch.into()],
    )?;
    run_git(
        Some(worktree_path),
        environment,
        [
            "remote".into(),
            "add".into(),
            "origin".into(),
            remote_url.into(),
        ],
    )?;
    run_git(
        Some(worktree_path),
        environment,
        ["fetch".into(), "--prune".into(), "origin".into()],
    )?;
    let git_dir = worktree_path.join(AGENT_GIT_DIR_NAME);
    let base_ref = preferred_remote_base(&git_dir, environment, branch, default_branch)?;
    run_git(
        Some(worktree_path),
        environment,
        ["reset".into(), "--mixed".into(), base_ref],
    )?;
    restore_missing_tracked_files(worktree_path, environment)?;
    configure_agent_repository(worktree_path, branch, environment)
}

fn restore_missing_tracked_files(
    worktree_path: &Path,
    environment: &HashMap<String, String>,
) -> AppResult<()> {
    let deleted_paths = run_git_bytes(
        Some(worktree_path),
        environment,
        [
            "diff".into(),
            "--name-only".into(),
            "--diff-filter=D".into(),
            "-z".into(),
        ],
    )?;
    let deleted_paths = deleted_paths
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| {
            String::from_utf8(path.to_vec()).map_err(|_| {
                AppError::Validation(
                    "Git workspace contains a deleted tracked path that is not valid UTF-8".into(),
                )
            })
        })
        .collect::<AppResult<Vec<_>>>()?;
    for paths in deleted_paths.chunks(100) {
        let mut args = vec!["restore".into(), "--worktree".into(), "--".into()];
        args.extend(paths.iter().cloned());
        run_git(Some(worktree_path), environment, args)?;
    }
    Ok(())
}

fn ensure_origin_remote(
    worktree_path: &Path,
    remote_url: &str,
    environment: &HashMap<String, String>,
) -> AppResult<()> {
    let remotes = run_git(Some(worktree_path), environment, ["remote".into()])?;
    if !remotes.lines().any(|remote| remote.trim() == "origin") {
        return run_git(
            Some(worktree_path),
            environment,
            [
                "remote".into(),
                "add".into(),
                "origin".into(),
                remote_url.into(),
            ],
        )
        .map(|_| ());
    }
    let current_remote = run_git(
        Some(worktree_path),
        environment,
        ["remote".into(), "get-url".into(), "origin".into()],
    )?;
    if current_remote.trim() != remote_url {
        run_git(
            Some(worktree_path),
            environment,
            [
                "remote".into(),
                "set-url".into(),
                "origin".into(),
                remote_url.into(),
            ],
        )?;
    }
    Ok(())
}

fn preferred_remote_base(
    git_dir: &Path,
    environment: &HashMap<String, String>,
    branch: &str,
    default_branch: &str,
) -> AppResult<String> {
    let remote_branch_ref = format!("refs/remotes/origin/{branch}");
    if ref_exists(git_dir, environment, &remote_branch_ref)? {
        return Ok(remote_branch_ref);
    }
    remote_default_ref(git_dir, environment, default_branch)
}

fn remote_default_ref(
    git_dir: &Path,
    environment: &HashMap<String, String>,
    default_branch: &str,
) -> AppResult<String> {
    let reference = format!("refs/remotes/origin/{default_branch}");
    ref_exists(git_dir, environment, &reference)?
        .then_some(reference)
        .ok_or_else(|| {
            AppError::NotFound(format!(
                "default branch {default_branch} was not found in the project Git remote"
            ))
        })
}

fn configure_agent_repository(
    worktree_path: &Path,
    branch: &str,
    environment: &HashMap<String, String>,
) -> AppResult<()> {
    exclude_agent_git_metadata(worktree_path)?;
    run_git(
        Some(worktree_path),
        environment,
        ["config".into(), "push.default".into(), "current".into()],
    )?;
    run_git(
        Some(worktree_path),
        environment,
        [
            "config".into(),
            "push.autoSetupRemote".into(),
            "true".into(),
        ],
    )?;
    let git_dir = worktree_path.join(AGENT_GIT_DIR_NAME);
    let remote_branch_ref = format!("refs/remotes/origin/{branch}");
    if ref_exists(&git_dir, environment, &remote_branch_ref)? {
        run_git(
            Some(worktree_path),
            environment,
            [
                "branch".into(),
                "--set-upstream-to".into(),
                format!("origin/{branch}"),
                branch.into(),
            ],
        )?;
    }
    Ok(())
}

fn exclude_agent_git_metadata(worktree_path: &Path) -> AppResult<()> {
    let info_directory = worktree_path.join(AGENT_GIT_DIR_NAME).join("info");
    fs::create_dir_all(&info_directory).map_err(file_error)?;
    let exclude_path = info_directory.join("exclude");
    let mut existing = fs::read_to_string(&exclude_path).unwrap_or_default();
    let pattern = format!("/{AGENT_GIT_DIR_NAME}/");
    if existing.lines().any(|line| line.trim() == pattern) {
        return Ok(());
    }
    if !existing.is_empty() && !existing.ends_with('\n') {
        existing.push('\n');
    }
    existing.push_str(&pattern);
    existing.push('\n');
    fs::write(exclude_path, existing).map_err(file_error)
}

fn agent_repository_environment(
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

fn absolute_path(path: PathBuf) -> AppResult<PathBuf> {
    if path.is_absolute() {
        return Ok(path);
    }
    std::env::current_dir()
        .map(|current_dir| current_dir.join(path))
        .map_err(file_error)
}

fn validate_project_root(raw: &str, allowed_roots: &[PathBuf]) -> AppResult<PathBuf> {
    let path = PathBuf::from(raw);
    if !path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir | std::path::Component::CurDir
            )
        })
    {
        return Err(AppError::Validation(
            "project host_local_path must be an absolute normalized path".into(),
        ));
    }
    if path.parent().is_none()
        || std::env::var("HOME")
            .ok()
            .filter(|home| !home.trim().is_empty())
            .is_some_and(|home| Path::new(&home) == path)
    {
        return Err(AppError::Validation(
            "project host_local_path cannot be the filesystem root or Trigger user's home".into(),
        ));
    }
    if !allowed_roots.is_empty()
        && !allowed_roots
            .iter()
            .any(|root| path.starts_with(root) && path != *root)
    {
        return Err(AppError::Validation(
            "project host_local_path is outside AGENT_TRIGGER_ALLOWED_LOCAL_ROOTS".into(),
        ));
    }
    fs::create_dir_all(&path).map_err(file_error)?;
    let canonical = fs::canonicalize(&path).map_err(file_error)?;
    if canonical.parent().is_none()
        || std::env::var("HOME")
            .ok()
            .and_then(|home| fs::canonicalize(home).ok())
            .is_some_and(|home| canonical == home)
    {
        return Err(AppError::Validation(
            "canonical project host_local_path is too broad".into(),
        ));
    }
    if !allowed_roots.is_empty() {
        let mut allowed = false;
        for root in allowed_roots {
            let canonical_root = fs::canonicalize(root).map_err(|error| {
                AppError::Validation(format!(
                    "failed to resolve allowed local root {}: {error}",
                    root.display()
                ))
            })?;
            if canonical.starts_with(&canonical_root) && canonical != canonical_root {
                allowed = true;
                break;
            }
        }
        if !allowed {
            return Err(AppError::Validation(
                "canonical project host_local_path escapes AGENT_TRIGGER_ALLOWED_LOCAL_ROOTS"
                    .into(),
            ));
        }
    }
    Ok(canonical)
}

fn run_git<I>(
    cwd: Option<&Path>,
    environment: &HashMap<String, String>,
    args: I,
) -> AppResult<String>
where
    I: IntoIterator<Item = String>,
{
    let args = args.into_iter().collect::<Vec<_>>();
    let max_attempts = if is_retryable_git_operation(&args) {
        3
    } else {
        1
    };
    for attempt in 0..max_attempts {
        let output = execute_git(cwd, environment, &args)?;
        if output.status.success() {
            return git_output(output);
        }
        let retryable = is_transient_git_network_error(&output.stderr);
        if !retryable || attempt + 1 == max_attempts {
            return git_output(output);
        }
        thread::sleep(Duration::from_millis(500 * (attempt as u64 + 1)));
    }
    unreachable!("Git retry loop always returns")
}

fn execute_git(
    cwd: Option<&Path>,
    environment: &HashMap<String, String>,
    args: &[String],
) -> AppResult<Output> {
    let mut command = Command::new("git");
    command.args(args).env("GIT_TERMINAL_PROMPT", "0");
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    for (key, value) in environment {
        command.env(key, value);
    }
    command.output().map_err(|error| {
        AppError::Validation(format!(
            "failed to start Git: {}",
            sanitize_error(&error.to_string())
        ))
    })
}

fn is_retryable_git_operation(args: &[String]) -> bool {
    args.first().is_some_and(|operation| operation == "fetch")
}

fn is_transient_git_network_error(stderr: &[u8]) -> bool {
    let stderr = String::from_utf8_lossy(stderr).to_ascii_lowercase();
    [
        "connection reset by peer",
        "recv failure",
        "failed to connect",
        "could not resolve host",
        "remote end hung up unexpectedly",
        "gnutls recv error",
        "tls connection was non-properly terminated",
        "the requested url returned error: 502",
        "the requested url returned error: 503",
        "the requested url returned error: 504",
    ]
    .iter()
    .any(|pattern| stderr.contains(pattern))
}

fn run_git_bytes<I>(
    cwd: Option<&Path>,
    environment: &HashMap<String, String>,
    args: I,
) -> AppResult<Vec<u8>>
where
    I: IntoIterator<Item = String>,
{
    let mut command = Command::new("git");
    command.args(args).env("GIT_TERMINAL_PROMPT", "0");
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    for (key, value) in environment {
        command.env(key, value);
    }
    let output = command.output().map_err(|error| {
        AppError::Validation(format!(
            "failed to start Git: {}",
            sanitize_error(&error.to_string())
        ))
    })?;
    if output.status.success() {
        return Ok(output.stdout);
    }
    let stderr = sanitize_error(&String::from_utf8_lossy(&output.stderr));
    Err(AppError::Validation(format!(
        "Git command failed with status {}: {}",
        output.status,
        truncate(&stderr, 1_000)
    )))
}

fn git_output(output: Output) -> AppResult<String> {
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).trim().to_string());
    }
    let stderr = sanitize_error(&String::from_utf8_lossy(&output.stderr));
    Err(AppError::Validation(format!(
        "Git command failed with status {}: {}",
        output.status,
        truncate(&stderr, 1_000)
    )))
}

fn ref_exists(
    mirror_path: &Path,
    environment: &HashMap<String, String>,
    reference: &str,
) -> AppResult<bool> {
    let mut command = Command::new("git");
    command
        .arg("--git-dir")
        .arg(mirror_path)
        .args(["show-ref", "--verify", "--quiet", reference])
        .env("GIT_TERMINAL_PROMPT", "0");
    for (key, value) in environment {
        command.env(key, value);
    }
    let status = command
        .status()
        .map_err(|error| AppError::Validation(format!("failed to inspect Git refs: {error}")))?;
    match status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => Err(AppError::Validation(
            "Git ref inspection failed unexpectedly".into(),
        )),
    }
}

fn validate_profile_name(value: &str) -> AppResult<()> {
    if value.is_empty()
        || value.chars().count() > 64
        || !value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        })
    {
        return Err(AppError::Validation(
            "Git auth profile name contains unsupported characters".into(),
        ));
    }
    Ok(())
}

fn validate_profile_environment(environment: &HashMap<String, String>) -> AppResult<()> {
    const ALLOWED: &[&str] = &[
        "GIT_SSH_COMMAND",
        "GIT_ASKPASS",
        "GIT_CONFIG_GLOBAL",
        "GIT_CONFIG_NOSYSTEM",
        "SSH_AUTH_SOCK",
    ];
    for (key, value) in environment {
        if !ALLOWED.contains(&key.as_str()) {
            return Err(AppError::Validation(format!(
                "Git auth profile environment key {key} is not allowed"
            )));
        }
        if value.is_empty() || value.chars().any(char::is_control) {
            return Err(AppError::Validation(format!(
                "Git auth profile environment value for {key} is invalid"
            )));
        }
    }
    Ok(())
}

fn sanitize_branch_component(value: &str) -> String {
    let mut normalized = value
        .trim()
        .trim_start_matches('@')
        .to_ascii_lowercase()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    while normalized.contains("--") {
        normalized = normalized.replace("--", "-");
    }
    let normalized = normalized.trim_matches('-');
    if normalized.is_empty() {
        "agent".into()
    } else {
        normalized.chars().take(48).collect()
    }
}

fn path_string(path: &Path) -> AppResult<String> {
    path.to_str()
        .map(str::to_string)
        .ok_or_else(|| AppError::Validation("Git workspace path must be valid UTF-8".into()))
}

fn file_error(error: std::io::Error) -> AppError {
    AppError::Validation(format!("Git workspace filesystem error: {error}"))
}

fn sanitize_error(value: &str) -> String {
    value.replace(['\r', '\n'], " ")
}

fn truncate(value: &str, max_characters: usize) -> String {
    value.chars().take(max_characters).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retries_only_fetch_for_transient_network_failures() {
        assert!(is_retryable_git_operation(&[
            "fetch".into(),
            "--prune".into(),
            "origin".into(),
        ]));
        assert!(!is_retryable_git_operation(&[
            "clone".into(),
            "https://example.test/repo.git".into(),
        ]));
        assert!(is_transient_git_network_error(
            b"fatal: unable to access remote: Recv failure: Connection reset by peer"
        ));
        assert!(!is_transient_git_network_error(
            b"fatal: Authentication failed for remote"
        ));
    }

    fn seed_remote(root: &Path) -> PathBuf {
        let remote = root.join("remote.git");
        let seed = root.join("seed");
        run_git(
            None,
            &HashMap::new(),
            [
                "init".into(),
                "--bare".into(),
                "-b".into(),
                "main".into(),
                path_string(&remote).expect("remote path"),
            ],
        )
        .expect("bare remote");
        run_git(
            None,
            &HashMap::new(),
            [
                "init".into(),
                "-b".into(),
                "main".into(),
                path_string(&seed).expect("seed path"),
            ],
        )
        .expect("seed repository");
        fs::write(seed.join("README.md"), "# Seed\n").expect("seed readme");
        fs::write(seed.join("REMOTE_ONLY.md"), "remote file\n").expect("remote-only file");
        run_git(
            Some(&seed),
            &HashMap::new(),
            ["config".into(), "user.name".into(), "Relay Test".into()],
        )
        .expect("git user name");
        run_git(
            Some(&seed),
            &HashMap::new(),
            [
                "config".into(),
                "user.email".into(),
                "relay-test@localhost".into(),
            ],
        )
        .expect("git user email");
        run_git(
            Some(&seed),
            &HashMap::new(),
            ["add".into(), "README.md".into(), "REMOTE_ONLY.md".into()],
        )
        .expect("stage seed");
        run_git(
            Some(&seed),
            &HashMap::new(),
            ["commit".into(), "-m".into(), "seed".into()],
        )
        .expect("commit seed");
        run_git(
            Some(&seed),
            &HashMap::new(),
            [
                "remote".into(),
                "add".into(),
                "origin".into(),
                path_string(&remote).expect("remote url"),
            ],
        )
        .expect("seed remote");
        run_git(
            Some(&seed),
            &HashMap::new(),
            ["push".into(), "origin".into(), "main".into()],
        )
        .expect("push seed");
        remote
    }

    fn project_git_config(project_root: &Path, remote: &Path) -> CompanyProjectGitConfig {
        let now = chrono::Utc::now();
        CompanyProjectGitConfig {
            project_id: Uuid::new_v4(),
            remote_url: path_string(remote).expect("remote url"),
            default_branch: "main".into(),
            git_host: "local".into(),
            host_local_path: path_string(project_root).expect("project root"),
            auth_profile: None,
            allow_agent_push: true,
            branch_prefix: "relay/".into(),
            created_by_agent_id: None,
            created_by_human_user_id: None,
            updated_by_agent_id: None,
            updated_by_human_user_id: None,
            created_at: now,
            updated_at: now,
        }
    }

    fn test_manager(root: &Path) -> GitWorkspaceManager {
        GitWorkspaceManager {
            general_workspace_root: root.join("general"),
            allowed_local_roots: vec![root.to_path_buf()],
            auth_profiles: HashMap::new(),
            credential_store: GitCredentialStore::at(root.join("credentials"))
                .expect("credential store"),
        }
    }

    #[test]
    fn project_root_must_be_below_an_allowed_host_root() {
        let allowed_root =
            std::env::temp_dir().join(format!("relay-workspace-root-{}", Uuid::new_v4().simple()));
        let project_root = allowed_root.join("project-a");
        fs::create_dir_all(&allowed_root).expect("allowed root");
        let resolved = validate_project_root(
            project_root.to_str().expect("utf8 project root"),
            std::slice::from_ref(&allowed_root),
        )
        .expect("project root should be accepted");
        assert_eq!(
            resolved,
            fs::canonicalize(&project_root).expect("canonical")
        );
        assert!(validate_project_root(
            allowed_root.to_str().expect("utf8 allowed root"),
            std::slice::from_ref(&allowed_root),
        )
        .is_err());
        fs::remove_dir_all(allowed_root).expect("cleanup");
    }

    #[test]
    fn general_workspace_is_initialized_as_a_git_repository() {
        let root = std::env::temp_dir().join(format!(
            "relay-general-workspace-{}",
            Uuid::new_v4().simple()
        ));
        let manager = GitWorkspaceManager {
            general_workspace_root: root.clone(),
            allowed_local_roots: Vec::new(),
            auth_profiles: HashMap::new(),
            credential_store: GitCredentialStore::at(root.join("credentials"))
                .expect("credential store"),
        };
        let prepared = manager
            .prepare_general_workspace(Uuid::new_v4(), Uuid::new_v4())
            .expect("general workspace");
        assert!(prepared.path.join(AGENT_GIT_DIR_NAME).is_dir());
        assert!(!prepared.path.join(".git").exists());
        assert_eq!(
            run_git(
                Some(&prepared.path),
                &prepared.auth_environment,
                ["rev-parse".into(), "--is-inside-work-tree".into()]
            )
            .expect("general Git worktree"),
            "true"
        );
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn project_workspace_uses_an_independent_writable_git_directory() {
        let root = std::env::temp_dir().join(format!(
            "relay-project-workspace-{}",
            Uuid::new_v4().simple()
        ));
        fs::create_dir_all(&root).expect("test root");
        let remote = seed_remote(&root);
        let project_root = root.join("project");
        let git = project_git_config(&project_root, &remote);
        let manager = test_manager(&root);
        let prepared = manager
            .prepare_project_workspace(
                Uuid::new_v4(),
                git.project_id,
                Uuid::new_v4(),
                "@backend",
                &git,
            )
            .expect("project workspace");

        assert!(prepared.path.join(AGENT_GIT_DIR_NAME).is_dir());
        assert!(!prepared.path.join(".git").exists());
        assert_eq!(prepared.branch, "relay/backend/inbox");
        assert_eq!(
            run_git(
                Some(&prepared.path),
                &prepared.auth_environment,
                ["branch".into(), "--show-current".into()]
            )
            .expect("current branch"),
            prepared.branch
        );
        assert_eq!(
            run_git(
                Some(&prepared.path),
                &prepared.auth_environment,
                ["status".into(), "--porcelain".into()]
            )
            .expect("clean Agent worktree"),
            ""
        );
        fs::write(prepared.path.join("agent.txt"), "work\n").expect("agent file");
        run_git(
            Some(&prepared.path),
            &prepared.auth_environment,
            ["add".into(), "agent.txt".into()],
        )
        .expect("Agent should be able to create the Git index lock");
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn project_workspace_recovers_after_an_empty_remote_gets_its_default_branch() {
        let root = std::env::temp_dir().join(format!(
            "relay-empty-remote-recovery-{}",
            Uuid::new_v4().simple()
        ));
        fs::create_dir_all(&root).expect("test root");
        let remote = root.join("remote.git");
        run_git(
            None,
            &HashMap::new(),
            [
                "init".into(),
                "--bare".into(),
                "-b".into(),
                "main".into(),
                path_string(&remote).expect("remote path"),
            ],
        )
        .expect("empty bare remote");

        let project_root = root.join("project");
        let git = project_git_config(&project_root, &remote);
        let manager = test_manager(&root);
        let agent_id = Uuid::new_v4();
        let first_error = manager
            .prepare_project_workspace(Uuid::new_v4(), git.project_id, agent_id, "@backend", &git)
            .expect_err("empty remote should not have a default branch ref yet");
        assert!(first_error
            .to_string()
            .contains("default branch main was not found"));

        let worktree = project_root
            .join(".relay/worktrees")
            .join(agent_id.to_string());
        let environment = agent_repository_environment(&HashMap::new(), &worktree)
            .expect("repository environment");
        assert_eq!(
            run_git(
                Some(&worktree),
                &environment,
                ["branch".into(), "--show-current".into()]
            )
            .expect("current unborn branch"),
            "main"
        );

        let seed = root.join("late-seed");
        run_git(
            None,
            &HashMap::new(),
            [
                "init".into(),
                "-b".into(),
                "main".into(),
                path_string(&seed).expect("seed path"),
            ],
        )
        .expect("seed repository");
        fs::write(seed.join("README.md"), "# Added later\n").expect("seed readme");
        run_git(
            Some(&seed),
            &HashMap::new(),
            ["config".into(), "user.name".into(), "Relay Test".into()],
        )
        .expect("git user name");
        run_git(
            Some(&seed),
            &HashMap::new(),
            [
                "config".into(),
                "user.email".into(),
                "relay-test@localhost".into(),
            ],
        )
        .expect("git user email");
        run_git(
            Some(&seed),
            &HashMap::new(),
            ["add".into(), "README.md".into()],
        )
        .expect("stage seed");
        run_git(
            Some(&seed),
            &HashMap::new(),
            ["commit".into(), "-m".into(), "seed".into()],
        )
        .expect("commit seed");
        run_git(
            Some(&seed),
            &HashMap::new(),
            [
                "remote".into(),
                "add".into(),
                "origin".into(),
                path_string(&remote).expect("remote url"),
            ],
        )
        .expect("seed remote");
        run_git(
            Some(&seed),
            &HashMap::new(),
            ["push".into(), "origin".into(), "main".into()],
        )
        .expect("push late default branch");

        let recovered = manager
            .prepare_project_workspace(Uuid::new_v4(), git.project_id, agent_id, "@backend", &git)
            .expect("half-initialized workspace should recover");
        assert_eq!(recovered.branch, "relay/backend/inbox");
        assert_eq!(
            run_git(
                Some(&recovered.path),
                &recovered.auth_environment,
                ["branch".into(), "--show-current".into()]
            )
            .expect("recovered branch"),
            recovered.branch
        );
        assert_eq!(
            fs::read_to_string(recovered.path.join("README.md")).expect("recovered readme"),
            "# Added later\n"
        );
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn project_workspace_does_not_auto_repair_a_real_default_branch_checkout() {
        let root = std::env::temp_dir().join(format!(
            "relay-protected-default-{}",
            Uuid::new_v4().simple()
        ));
        fs::create_dir_all(&root).expect("test root");
        let remote = seed_remote(&root);
        let project_root = root.join("project");
        let git = project_git_config(&project_root, &remote);
        let manager = test_manager(&root);
        let agent_id = Uuid::new_v4();
        let prepared = manager
            .prepare_project_workspace(Uuid::new_v4(), git.project_id, agent_id, "@backend", &git)
            .expect("project workspace");
        run_git(
            Some(&prepared.path),
            &prepared.auth_environment,
            [
                "switch".into(),
                "-C".into(),
                "main".into(),
                "origin/main".into(),
            ],
        )
        .expect("simulate a real protected default branch checkout");

        let error = manager
            .prepare_project_workspace(Uuid::new_v4(), git.project_id, agent_id, "@backend", &git)
            .expect_err("a real default branch checkout must remain protected");
        assert!(error
            .to_string()
            .contains("checked out on the protected default branch"));
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn project_workspace_attaches_a_detached_head_without_losing_local_changes() {
        let root = std::env::temp_dir().join(format!(
            "relay-detached-agent-head-{}",
            Uuid::new_v4().simple()
        ));
        fs::create_dir_all(&root).expect("test root");
        let remote = seed_remote(&root);
        let project_root = root.join("project");
        let git = project_git_config(&project_root, &remote);
        let manager = test_manager(&root);
        let agent_id = Uuid::new_v4();
        let prepared = manager
            .prepare_project_workspace(Uuid::new_v4(), git.project_id, agent_id, "@backend", &git)
            .expect("project workspace");
        run_git(
            Some(&prepared.path),
            &prepared.auth_environment,
            ["switch".into(), "--detach".into(), "origin/main".into()],
        )
        .expect("detach Agent HEAD");
        fs::write(prepared.path.join("local-note.txt"), "preserve me\n")
            .expect("local detached work");
        let detached_head = run_git(
            Some(&prepared.path),
            &prepared.auth_environment,
            ["rev-parse".into(), "HEAD".into()],
        )
        .expect("detached head");

        let resumed = manager
            .prepare_project_workspace(Uuid::new_v4(), git.project_id, agent_id, "@backend", &git)
            .expect("detached Agent repository should be attached safely");

        assert!(resumed.branch.starts_with("relay/backend/recovery-"));
        assert_eq!(
            run_git(
                Some(&resumed.path),
                &resumed.auth_environment,
                ["rev-parse".into(), "HEAD".into()]
            )
            .expect("attached head"),
            detached_head
        );
        assert_eq!(
            fs::read_to_string(resumed.path.join("local-note.txt")).expect("preserved local work"),
            "preserve me\n"
        );
        assert_eq!(
            run_git(
                Some(&resumed.path),
                &resumed.auth_environment,
                ["branch".into(), "--show-current".into()]
            )
            .expect("recovery branch"),
            resumed.branch
        );
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn project_workspace_preserves_an_active_agent_prefixed_branch() {
        let root = std::env::temp_dir().join(format!(
            "relay-active-agent-branch-{}",
            Uuid::new_v4().simple()
        ));
        fs::create_dir_all(&root).expect("test root");
        let remote = seed_remote(&root);
        let project_root = root.join("project");
        let git = project_git_config(&project_root, &remote);
        let manager = test_manager(&root);
        let agent_id = Uuid::new_v4();
        let prepared = manager
            .prepare_project_workspace(Uuid::new_v4(), git.project_id, agent_id, "@backend", &git)
            .expect("project workspace");
        let active_branch = "relay/backend-integration";
        run_git(
            Some(&prepared.path),
            &prepared.auth_environment,
            ["switch".into(), "-c".into(), active_branch.into()],
        )
        .expect("switch to active Agent branch");

        let resumed = manager
            .prepare_project_workspace(Uuid::new_v4(), git.project_id, agent_id, "@backend", &git)
            .expect("active Agent branch should be preserved");

        assert_eq!(resumed.branch, active_branch);
        assert_eq!(
            run_git(
                Some(&resumed.path),
                &resumed.auth_environment,
                ["branch".into(), "--show-current".into()]
            )
            .expect("current branch"),
            active_branch
        );
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn project_workspace_rejects_a_branch_outside_the_agent_prefix() {
        let root = std::env::temp_dir().join(format!(
            "relay-outside-agent-prefix-{}",
            Uuid::new_v4().simple()
        ));
        fs::create_dir_all(&root).expect("test root");
        let remote = seed_remote(&root);
        let project_root = root.join("project");
        let git = project_git_config(&project_root, &remote);
        let manager = test_manager(&root);
        let agent_id = Uuid::new_v4();
        let prepared = manager
            .prepare_project_workspace(Uuid::new_v4(), git.project_id, agent_id, "@backend", &git)
            .expect("project workspace");
        run_git(
            Some(&prepared.path),
            &prepared.auth_environment,
            ["switch".into(), "-c".into(), "feature/manual".into()],
        )
        .expect("switch outside Agent prefix");

        let error = manager
            .prepare_project_workspace(Uuid::new_v4(), git.project_id, agent_id, "@backend", &git)
            .expect_err("branch outside Agent prefix must be rejected");

        assert!(error
            .to_string()
            .contains("outside the configured Agent branch prefix relay/"));
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn existing_dot_git_directory_is_relocated_without_losing_work() {
        let root = std::env::temp_dir().join(format!(
            "relay-existing-dot-git-{}",
            Uuid::new_v4().simple()
        ));
        fs::create_dir_all(&root).expect("test root");
        let remote = seed_remote(&root);
        let project_root = root.join("project");
        let git = project_git_config(&project_root, &remote);
        let manager = test_manager(&root);
        let agent_id = Uuid::new_v4();
        let prepared = manager
            .prepare_project_workspace(Uuid::new_v4(), git.project_id, agent_id, "@backend", &git)
            .expect("project workspace");
        fs::write(prepared.path.join("agent.txt"), "preserve me\n").expect("agent file");
        fs::rename(
            prepared.path.join(AGENT_GIT_DIR_NAME),
            prepared.path.join(".git"),
        )
        .expect("simulate previous independent repository");

        let migrated = manager
            .prepare_project_workspace(Uuid::new_v4(), git.project_id, agent_id, "@backend", &git)
            .expect("relocated workspace");

        assert!(migrated.path.join(AGENT_GIT_DIR_NAME).is_dir());
        assert!(!migrated.path.join(".git").exists());
        assert_eq!(
            fs::read_to_string(migrated.path.join("agent.txt")).expect("preserved agent file"),
            "preserve me\n"
        );
        run_git(
            Some(&migrated.path),
            &migrated.auth_environment,
            ["add".into(), "agent.txt".into()],
        )
        .expect("relocated repository should remain writable");
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn legacy_linked_worktree_is_migrated_without_losing_agent_files() {
        let root =
            std::env::temp_dir().join(format!("relay-legacy-worktree-{}", Uuid::new_v4().simple()));
        fs::create_dir_all(&root).expect("test root");
        let remote = seed_remote(&root);
        let project_root = root.join("project");
        let relay_root = project_root.join(".relay");
        let mirror = relay_root.join("mirror.git");
        let agent_id = Uuid::new_v4();
        let worktree = relay_root.join("worktrees").join(agent_id.to_string());
        fs::create_dir_all(&relay_root).expect("relay root");
        run_git(
            None,
            &HashMap::new(),
            [
                "clone".into(),
                "--mirror".into(),
                path_string(&remote).expect("remote url"),
                path_string(&mirror).expect("mirror path"),
            ],
        )
        .expect("legacy mirror");
        run_git(
            None,
            &HashMap::new(),
            [
                "--git-dir".into(),
                path_string(&mirror).expect("mirror path"),
                "worktree".into(),
                "add".into(),
                "-b".into(),
                "relay/backend/inbox".into(),
                path_string(&worktree).expect("worktree path"),
                "main".into(),
            ],
        )
        .expect("legacy linked worktree");
        fs::write(worktree.join("agent.txt"), "preserve me\n").expect("agent file");
        fs::write(worktree.join("README.md"), "# Agent edit\n").expect("modified readme");
        fs::remove_file(worktree.join("REMOTE_ONLY.md")).expect("remove tracked file");

        let git = project_git_config(&project_root, &remote);
        let prepared = test_manager(&root)
            .prepare_project_workspace(Uuid::new_v4(), git.project_id, agent_id, "@backend", &git)
            .expect("migrated workspace");

        assert!(prepared.path.join(AGENT_GIT_DIR_NAME).is_dir());
        assert!(!prepared.path.join(".git").exists());
        assert_eq!(
            fs::read_to_string(prepared.path.join("agent.txt")).expect("preserved file"),
            "preserve me\n"
        );
        assert_eq!(
            fs::read_to_string(prepared.path.join("README.md")).expect("preserved readme edit"),
            "# Agent edit\n"
        );
        assert_eq!(
            fs::read_to_string(prepared.path.join("REMOTE_ONLY.md"))
                .expect("restored remote-only file"),
            "remote file\n"
        );
        assert!(relay_root
            .join("legacy-gitlinks")
            .read_dir()
            .expect("legacy backups")
            .next()
            .is_some());
        run_git(
            Some(&prepared.path),
            &prepared.auth_environment,
            ["add".into(), "agent.txt".into()],
        )
        .expect("migrated Agent should be able to create the Git index lock");
        fs::remove_dir_all(root).expect("cleanup");
    }
}
