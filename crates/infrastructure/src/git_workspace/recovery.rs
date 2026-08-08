use std::{collections::HashMap, fs, path::Path};

use uuid::Uuid;

use ai_chat_shared::{AppError, AppResult};

use super::{
    archive_legacy_git_marker,
    command::{ref_exists, run_git},
    configure_agent_repository, preferred_remote_base, restore_missing_tracked_files,
    validation::file_error,
    AGENT_GIT_DIR_NAME,
};

pub(super) fn attach_detached_agent_repository(
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

pub(super) fn agent_worktree_has_no_user_files(worktree_path: &Path) -> AppResult<bool> {
    for entry in fs::read_dir(worktree_path).map_err(file_error)? {
        let entry = entry.map_err(file_error)?;
        if entry.file_name() != AGENT_GIT_DIR_NAME {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(super) fn migrate_legacy_linked_worktree(
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
