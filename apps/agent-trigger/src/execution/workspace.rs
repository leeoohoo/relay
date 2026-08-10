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
        Err(error) => Err(error),
    }
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
