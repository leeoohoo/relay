use super::*;

impl<R: PlatformRepository> HarnessProvisioner<R> {
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
