use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) async fn prepare_project_workspace_with_credential_recovery(
    harness: &TriggerHarnessProvisioner,
    workspace_manager: &GitWorkspaceManager,
    company_id: Uuid,
    project_id: Uuid,
    agent_id: Uuid,
    fallback_human_user_id: Uuid,
    agent_handle: &str,
    project_name: &str,
    project_description: &str,
    git: &ai_chat_domain::company::CompanyProjectGitConfig,
) -> AppResult<PreparedGitWorkspace> {
    let prepare = || {
        workspace_manager.prepare_project_workspace(
            company_id,
            project_id,
            agent_id,
            agent_handle,
            git,
        )
    };
    match prepare() {
        Ok(workspace) => Ok(workspace),
        Err(error) if should_refresh_managed_git_credentials(git, &error) => {
            let human_user_id = git
                .created_by_human_user_id
                .unwrap_or(fallback_human_user_id);
            tracing::warn!(
                project_id = %project_id,
                agent_id = %agent_id,
                human_user_id = %human_user_id,
                "managed Git credentials were rejected; refreshing the project token once"
            );
            harness
                .refresh_project_git_credentials(
                    human_user_id,
                    project_id,
                    workspace_manager.credential_store(),
                )
                .await?;
            prepare().map_err(|retry_error| {
                AppError::Validation(format!(
                    "Git authentication still failed after refreshing the managed project credential: {retry_error}"
                ))
            })
        }
        Err(error) if should_initialize_managed_default_branch(git, &error) => {
            let human_user_id = git
                .created_by_human_user_id
                .unwrap_or(fallback_human_user_id);
            tracing::warn!(
                project_id = %project_id,
                agent_id = %agent_id,
                default_branch = %git.default_branch,
                "managed Git repository has no default branch; initializing it once"
            );
            harness
                .ensure_project_default_branch(
                    human_user_id,
                    project_id,
                    git.remote_url.as_str(),
                    git.default_branch.as_str(),
                    project_name,
                    project_description,
                    workspace_manager.credential_store(),
                )
                .await?;
            prepare().map_err(|retry_error| {
                AppError::Validation(format!(
                    "project default branch is still unavailable after repository initialization: {retry_error}"
                ))
            })
        }
        Err(error) => Err(error),
    }
}

fn should_initialize_managed_default_branch(
    git: &ai_chat_domain::company::CompanyProjectGitConfig,
    error: &AppError,
) -> bool {
    git.auth_profile
        .as_deref()
        .is_some_and(is_managed_token_profile)
        && is_missing_default_branch_error(error)
}

fn should_refresh_managed_git_credentials(
    git: &ai_chat_domain::company::CompanyProjectGitConfig,
    error: &AppError,
) -> bool {
    git.auth_profile
        .as_deref()
        .is_some_and(is_managed_token_profile)
        && is_git_authentication_error(error)
}
