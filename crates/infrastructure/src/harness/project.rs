use super::*;
use crate::project_git::ensure_remote_default_branch;

impl<R: PlatformRepository> HarnessProvisioner<R> {
    #[allow(clippy::too_many_arguments)]
    pub async fn ensure_project_default_branch(
        &self,
        human_user_id: Uuid,
        project_id: Uuid,
        remote_url: &str,
        default_branch: &str,
        project_name: &str,
        description: &str,
        git_credentials: &GitCredentialStore,
    ) -> AppResult<bool> {
        let access = self
            .repository_access(human_user_id, remote_url)?
            .ok_or_else(|| {
                AppError::Validation("project is not a managed Harness repository".into())
            })?;
        let internal_base = reqwest::Url::parse(access.api_base_url.as_str()).map_err(|error| {
            AppError::Validation(format!("invalid Harness internal URL: {error}"))
        })?;
        let push_url = internal_base
            .join(&format!("git/{}.git", access.repository_path))
            .map_err(|error| AppError::Validation(format!("invalid Harness push URL: {error}")))?;
        let auth_profile = git_credentials
            .has_managed_git_token(project_id)
            .then(|| crate::git_credentials::managed_token_profile_name(project_id))
            .ok_or_else(|| {
                AppError::Validation(
                    "managed Git credential is unavailable for repository initialization".into(),
                )
            })?;
        ensure_remote_default_branch(
            project_id,
            push_url.as_str(),
            default_branch,
            auth_profile.as_str(),
            project_name,
            description,
            git_credentials,
        )
    }

    pub async fn cleanup_project_git_resources_by_identifier(
        &self,
        human_user_id: Uuid,
        project_id: Uuid,
        repository_identifier: &str,
        access_token_identifier: &str,
        git_credentials: &GitCredentialStore,
    ) -> AppResult<()> {
        self.cleanup_project_git_resources(
            human_user_id,
            project_id,
            repository_identifier,
            Some(access_token_identifier),
            git_credentials,
        )
        .await
    }

    pub async fn cleanup_provisioned_project_git(
        &self,
        human_user_id: Uuid,
        project_id: Uuid,
        provisioned: &ProvisionedProjectGit,
        git_credentials: &GitCredentialStore,
    ) -> AppResult<()> {
        self.cleanup_project_git_resources(
            human_user_id,
            project_id,
            provisioned.repository_identifier.as_str(),
            Some(provisioned.access_token_identifier.as_str()),
            git_credentials,
        )
        .await
    }

    pub(super) async fn cleanup_project_git_resources(
        &self,
        human_user_id: Uuid,
        project_id: Uuid,
        repository_identifier: &str,
        access_token_identifier: Option<&str>,
        git_credentials: &GitCredentialStore,
    ) -> AppResult<()> {
        self.cleanup_project_git_resources_inner(
            human_user_id,
            project_id,
            repository_identifier,
            access_token_identifier,
            git_credentials,
            true,
        )
        .await
    }

    pub(super) async fn cleanup_project_git_credentials(
        &self,
        human_user_id: Uuid,
        project_id: Uuid,
        access_token_identifier: Option<&str>,
        git_credentials: &GitCredentialStore,
    ) -> AppResult<()> {
        self.cleanup_project_git_resources_inner(
            human_user_id,
            project_id,
            "",
            access_token_identifier,
            git_credentials,
            false,
        )
        .await
    }

    async fn cleanup_project_git_resources_inner(
        &self,
        human_user_id: Uuid,
        project_id: Uuid,
        repository_identifier: &str,
        access_token_identifier: Option<&str>,
        git_credentials: &GitCredentialStore,
        delete_repository: bool,
    ) -> AppResult<()> {
        let mut failures = Vec::new();
        if let Err(error) = git_credentials.remove_project_tokens(project_id) {
            failures.push(format!("remove local Git credentials: {error}"));
        }
        let account = self.active_account(human_user_id);
        let access_token = self.account_access_token(human_user_id);
        match (account, access_token) {
            (Ok(account), Ok(access_token)) => {
                let api_base_url = self.config.api_base_url.as_deref().ok_or_else(|| {
                    AppError::Validation("HARNESS_BASE_URL is required for Git compensation".into())
                })?;
                if let Some(identifier) = access_token_identifier {
                    let endpoint = format!("{api_base_url}/api/v1/user/tokens/{identifier}");
                    if let Err(error) = self
                        .request_without_response(
                            Method::DELETE,
                            endpoint.as_str(),
                            Some(access_token.as_str()),
                        )
                        .await
                    {
                        if !error.is_not_found() {
                            failures.push(format!("revoke Harness project token: {error}"));
                        }
                    }
                }
                if delete_repository {
                    let repository_ref = format!(
                        "{}%2F{}",
                        account.space_identifier.replace('/', "%2F"),
                        repository_identifier
                    );
                    let endpoint = format!("{api_base_url}/api/v1/repos/{repository_ref}");
                    if let Err(error) = self
                        .request_without_response(
                            Method::DELETE,
                            endpoint.as_str(),
                            Some(access_token.as_str()),
                        )
                        .await
                    {
                        if !error.is_not_found() {
                            failures.push(format!("delete Harness repository: {error}"));
                        }
                    }
                }
            }
            (account, access_token) => {
                if let Err(error) = account {
                    failures.push(format!("load Harness account for cleanup: {error}"));
                }
                if let Err(error) = access_token {
                    failures.push(format!("load Harness access token for cleanup: {error}"));
                }
            }
        }
        if failures.is_empty() {
            Ok(())
        } else {
            Err(AppError::Internal(format!(
                "project provisioning cleanup failed: {}",
                failures.join("; ")
            )))
        }
    }

    pub async fn refresh_project_git_credentials(
        &self,
        human_user_id: Uuid,
        project_id: Uuid,
        git_credentials: &GitCredentialStore,
    ) -> AppResult<String> {
        if !self.is_enabled() {
            return Err(AppError::Validation(
                "Harness is disabled; managed Git credentials cannot be refreshed".into(),
            ));
        }
        let mut account = self.active_account(human_user_id)?;
        let api_base_url = self.config.api_base_url.as_deref().ok_or_else(|| {
            AppError::Validation("HARNESS_BASE_URL is required for Git provisioning".into())
        })?;
        let mut access_token = self.account_access_token(human_user_id)?;
        let project_token = match self
            .create_project_access_token(api_base_url, access_token.as_str(), project_id)
            .await
        {
            Ok(token) => token,
            Err(error) if error.is_unauthorized() => {
                self.credentials.remove_access_token(human_user_id)?;
                let user = self
                    .repo
                    .get_human_user_result(human_user_id)?
                    .ok_or_else(|| AppError::NotFound("Human user not found".into()))?;
                account = self.ensure_active_account(&user).await?;
                access_token = self.account_access_token(human_user_id)?;
                self.create_project_access_token(api_base_url, access_token.as_str(), project_id)
                    .await
                    .map_err(|retry_error| {
                        AppError::Internal(format!(
                            "refresh Harness project token after account recovery: {retry_error}"
                        ))
                    })?
            }
            Err(error) => {
                return Err(AppError::Internal(format!(
                    "refresh Harness project token: {error}"
                )))
            }
        };
        git_credentials.store_managed_git_token(
            project_id,
            account.harness_uid.as_str(),
            project_token.access_token.as_str(),
        )
    }

    fn active_account(&self, human_user_id: Uuid) -> AppResult<HumanHarnessAccount> {
        self.repo
            .get_human_harness_account_result(human_user_id)?
            .filter(|account| account.status == HUMAN_HARNESS_STATUS_ACTIVE)
            .ok_or_else(|| {
                AppError::Validation("当前用户的 Harness 账户尚未就绪，无法刷新项目凭证".into())
            })
    }

    fn account_access_token(&self, human_user_id: Uuid) -> AppResult<String> {
        self.credentials
            .read_access_token(human_user_id)?
            .ok_or_else(|| AppError::Validation("Harness access token is unavailable".into()))
    }

    pub(super) async fn create_project_access_token(
        &self,
        api_base_url: &str,
        access_token: &str,
        project_id: Uuid,
    ) -> Result<HarnessCreatedToken, HarnessRequestError> {
        let token_identifier = format!(
            "relay-project-{}-{}",
            short_identifier(project_id),
            short_identifier(Uuid::new_v4())
        );
        self.create_project_access_token_with_identifier(
            api_base_url,
            access_token,
            token_identifier,
        )
        .await
    }

    pub(super) async fn create_project_access_token_with_identifier(
        &self,
        api_base_url: &str,
        access_token: &str,
        token_identifier: String,
    ) -> Result<HarnessCreatedToken, HarnessRequestError> {
        let token_request = HarnessCreateAccessTokenRequest {
            identifier: token_identifier.as_str(),
        };
        let response = self
            .request_json::<HarnessTokenResponse, _>(
                Method::POST,
                format!("{api_base_url}/api/v1/user/tokens").as_str(),
                Some(access_token),
                Some(&token_request),
            )
            .await?;
        Ok(HarnessCreatedToken {
            identifier: token_identifier,
            access_token: non_empty_token(response.access_token)?,
        })
    }
}
