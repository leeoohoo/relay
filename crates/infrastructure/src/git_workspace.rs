mod command;
mod manager;
mod validation;

use std::{collections::HashMap, fs, path::Path};

use uuid::Uuid;

use ai_chat_shared::{AppError, AppResult};

use command::{ref_exists, run_git, run_git_bytes};
pub use manager::{is_git_authentication_error, GitWorkspaceManager, PreparedGitWorkspace};
use validation::file_error;

#[cfg(test)]
use crate::git_credentials::GitCredentialStore;
#[cfg(test)]
use ai_chat_domain::company::CompanyProjectGitConfig;
#[cfg(test)]
use command::{is_retryable_git_operation, is_transient_git_network_error};
#[cfg(test)]
use manager::agent_repository_environment;
#[cfg(test)]
use std::path::PathBuf;
#[cfg(test)]
use validation::{path_string, validate_project_root};

const AGENT_GIT_DIR_NAME: &str = ".relay-git";

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
    archive_legacy_git_marker(&backup_root, worktree_path, agent_id)?;
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

fn archive_legacy_git_marker(
    backup_root: &Path,
    worktree_path: &Path,
    agent_id: Uuid,
) -> AppResult<()> {
    fs::create_dir_all(backup_root).map_err(file_error)?;
    let backup_path = backup_root.join(format!(
        "{}-{}.git-metadata",
        agent_id,
        Uuid::new_v4().simple()
    ));
    fs::rename(worktree_path.join(".git"), backup_path).map_err(file_error)
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
    fn duplicate_project_git_metadata_is_archived_and_workspace_recovers() {
        let root = std::env::temp_dir().join(format!(
            "relay-duplicate-project-git-{}",
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
        let duplicate_git = prepared.path.join(".git");
        fs::create_dir_all(&duplicate_git).expect("duplicate metadata");
        fs::write(
            duplicate_git.join("sentinel"),
            "preserve duplicate metadata\n",
        )
        .expect("duplicate sentinel");

        let recovered = manager
            .prepare_project_workspace(Uuid::new_v4(), git.project_id, agent_id, "@backend", &git)
            .expect("workspace should recover from duplicate metadata");

        assert!(recovered.path.join(AGENT_GIT_DIR_NAME).is_dir());
        assert!(!recovered.path.join(".git").exists());
        let backups = project_root.join(".relay/legacy-gitlinks");
        let backup = backups
            .read_dir()
            .expect("backup directory")
            .next()
            .expect("archived duplicate metadata")
            .expect("backup entry")
            .path();
        assert_eq!(
            fs::read_to_string(backup.join("sentinel")).expect("archived sentinel"),
            "preserve duplicate metadata\n"
        );
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn duplicate_general_git_marker_is_archived_and_workspace_recovers() {
        let root = std::env::temp_dir().join(format!(
            "relay-duplicate-general-git-{}",
            Uuid::new_v4().simple()
        ));
        fs::create_dir_all(&root).expect("test root");
        let manager = test_manager(&root);
        let company_id = Uuid::new_v4();
        let agent_id = Uuid::new_v4();
        let prepared = manager
            .prepare_general_workspace(company_id, agent_id)
            .expect("general workspace");
        fs::write(prepared.path.join(".git"), "legacy git marker\n").expect("duplicate git marker");

        let recovered = manager
            .prepare_general_workspace(company_id, agent_id)
            .expect("general workspace should recover from duplicate metadata");

        assert!(recovered.path.join(AGENT_GIT_DIR_NAME).is_dir());
        assert!(!recovered.path.join(".git").exists());
        let backup = manager
            .general_workspace_root
            .join("legacy-gitlinks")
            .read_dir()
            .expect("backup directory")
            .next()
            .expect("archived duplicate marker")
            .expect("backup entry")
            .path();
        assert_eq!(
            fs::read_to_string(backup).expect("archived marker"),
            "legacy git marker\n"
        );
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

    #[test]
    fn recognizes_only_explicit_git_authentication_failures() {
        assert!(is_git_authentication_error(&AppError::Validation(
            "Git command failed: The requested URL returned error: 403".into()
        )));
        assert!(is_git_authentication_error(&AppError::Validation(
            "fatal: Authentication failed for repository".into()
        )));
        assert!(!is_git_authentication_error(&AppError::Validation(
            "fatal: unable to access repository: Could not resolve host".into()
        )));
        assert!(!is_git_authentication_error(&AppError::Validation(
            "The requested URL returned error: 404".into()
        )));
    }
}
