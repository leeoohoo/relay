use std::{sync::Arc, time::Duration};

use reqwest::{blocking::Client, StatusCode, Url};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use ai_chat_shared::{AppError, AppResult};

use crate::git_credentials::{managed_token_profile_name, GitCredentialStore};

#[derive(Debug, Clone)]
pub struct ProjectGitProvisionRequest {
    pub project_id: Uuid,
    pub project_name: String,
    pub description: String,
    pub repository_identifier: Option<String>,
    pub is_public: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProvisionedProjectGit {
    pub remote_url: String,
    pub default_branch: String,
    pub auth_profile: String,
    pub repository_identifier: String,
}

pub trait ProjectGitProvisioner: Send + Sync {
    fn provision(&self, request: ProjectGitProvisionRequest) -> AppResult<ProvisionedProjectGit>;
}

#[derive(Debug, Clone)]
pub struct GitnessProjectGitProvisioner {
    base_url: Url,
    clone_base_url: Url,
    parent_ref: String,
    username: String,
    provider_token: String,
    credential_store: GitCredentialStore,
}

#[derive(Debug, Deserialize)]
struct GitnessRepository {
    default_branch: String,
}

#[derive(Debug, Deserialize)]
struct GitnessTokenResponse {
    access_token: String,
}

impl GitnessProjectGitProvisioner {
    pub fn from_env(
        credential_store: GitCredentialStore,
    ) -> AppResult<Option<Arc<dyn ProjectGitProvisioner>>> {
        let provider_kind = std::env::var("RELAY_GIT_PROVIDER_KIND").unwrap_or_default();
        if provider_kind.trim().is_empty() {
            return Ok(None);
        }
        if !provider_kind.eq_ignore_ascii_case("gitness") {
            return Err(AppError::Validation(
                "RELAY_GIT_PROVIDER_KIND currently supports only gitness".into(),
            ));
        }
        let base_url = required_env("RELAY_GIT_PROVIDER_BASE_URL")?;
        let base_url = Url::parse(base_url.trim()).map_err(|error| {
            AppError::Validation(format!("invalid RELAY_GIT_PROVIDER_BASE_URL: {error}"))
        })?;
        if base_url.scheme() != "https" {
            return Err(AppError::Validation(
                "RELAY_GIT_PROVIDER_BASE_URL must use https".into(),
            ));
        }
        let clone_base_url = std::env::var("RELAY_GIT_PROVIDER_CLONE_BASE_URL")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(|value| {
                Url::parse(value.trim()).map_err(|error| {
                    AppError::Validation(format!(
                        "invalid RELAY_GIT_PROVIDER_CLONE_BASE_URL: {error}"
                    ))
                })
            })
            .transpose()?
            .unwrap_or_else(|| base_url.clone());
        if clone_base_url.scheme() != "https" {
            return Err(AppError::Validation(
                "RELAY_GIT_PROVIDER_CLONE_BASE_URL must use https".into(),
            ));
        }
        let parent_ref = validate_parent_ref(&required_env("RELAY_GIT_PROVIDER_PARENT_REF")?)?;
        let username = validate_username(&required_env("RELAY_GIT_PROVIDER_USERNAME")?)?;
        let provider_token = required_env("RELAY_GIT_PROVIDER_TOKEN")?;
        validate_token(&provider_token)?;
        Ok(Some(Arc::new(Self {
            base_url,
            clone_base_url,
            parent_ref,
            username,
            provider_token,
            credential_store,
        })))
    }

    fn api_url(&self, path: &str) -> AppResult<Url> {
        self.base_url
            .join(path)
            .map_err(|error| AppError::Validation(format!("invalid Gitness API URL: {error}")))
    }

    fn create_or_get_repository(
        &self,
        client: &Client,
        identifier: &str,
        description: &str,
        is_public: bool,
    ) -> AppResult<GitnessRepository> {
        let response = client
            .post(self.api_url("api/v1/repos")?)
            .bearer_auth(&self.provider_token)
            .json(&serde_json::json!({
                "parent_ref": self.parent_ref,
                "identifier": identifier,
                "default_branch": "main",
                "description": description,
                "is_public": is_public,
                "readme": true,
            }))
            .send();
        let response = match response {
            Ok(response) => response,
            Err(create_error) => {
                return self
                    .get_repository_with_retry(client, identifier)
                    .map_err(|_| provider_error(create_error));
            }
        };
        if response.status().is_success() {
            return response.json().map_err(provider_error);
        }
        if !matches!(
            response.status(),
            StatusCode::BAD_REQUEST | StatusCode::CONFLICT
        ) {
            return Err(response_error("create Gitness repository", response));
        }

        self.get_repository_with_retry(client, identifier)
    }

    fn get_repository_with_retry(
        &self,
        client: &Client,
        identifier: &str,
    ) -> AppResult<GitnessRepository> {
        let repo_ref = format!("{}%2F{}", self.parent_ref.replace('/', "%2F"), identifier);
        let url = self.api_url(&format!("api/v1/repos/{repo_ref}"))?;
        let mut last_error = None;
        for attempt in 0..4 {
            match client
                .get(url.clone())
                .bearer_auth(&self.provider_token)
                .send()
            {
                Ok(response) if response.status().is_success() => {
                    return response.json().map_err(provider_error);
                }
                Ok(response)
                    if response.status() == StatusCode::TOO_MANY_REQUESTS
                        || response.status().is_server_error() =>
                {
                    last_error = Some(response_error("read existing Gitness repository", response));
                }
                Ok(response) => {
                    return Err(response_error("read existing Gitness repository", response));
                }
                Err(error) => last_error = Some(provider_error(error)),
            }
            if attempt < 3 {
                std::thread::sleep(Duration::from_millis(250 * (1 << attempt)));
            }
        }
        Err(last_error.unwrap_or_else(|| {
            AppError::Validation("failed to read existing Gitness repository".into())
        }))
    }

    fn ensure_project_token(&self, client: &Client, project_id: Uuid) -> AppResult<String> {
        if self.credential_store.has_managed_git_token(project_id) {
            return Ok(managed_token_profile_name(project_id));
        }
        let identifier = format!(
            "relay-project-{}-{}",
            &project_id.simple().to_string()[..12],
            &Uuid::new_v4().simple().to_string()[..8]
        );
        let response = client
            .post(self.api_url("api/v1/user/tokens")?)
            .bearer_auth(&self.provider_token)
            .json(&serde_json::json!({ "identifier": identifier }))
            .send()
            .map_err(provider_error)?;
        if !response.status().is_success() {
            return Err(response_error("create Gitness project token", response));
        }
        let token: GitnessTokenResponse = response.json().map_err(provider_error)?;
        self.credential_store.store_managed_git_token(
            project_id,
            &self.username,
            &token.access_token,
        )
    }

    fn provision_blocking(
        &self,
        request: ProjectGitProvisionRequest,
    ) -> AppResult<ProvisionedProjectGit> {
        let client = Client::builder()
            .timeout(Duration::from_secs(20))
            .build()
            .map_err(provider_error)?;
        let repository_identifier = request
            .repository_identifier
            .as_deref()
            .map(validate_repository_identifier)
            .transpose()?
            .unwrap_or_else(|| {
                generated_repository_identifier(&request.project_name, request.project_id)
            });
        let repository = self.create_or_get_repository(
            &client,
            &repository_identifier,
            &request.description,
            request.is_public,
        )?;
        let remote_url = self
            .clone_base_url
            .join(&format!(
                "git/{}/{}.git",
                self.parent_ref, repository_identifier
            ))
            .map_err(|error| AppError::Validation(format!("invalid Gitness clone URL: {error}")))?
            .to_string();
        let auth_profile = self.ensure_project_token(&client, request.project_id)?;
        Ok(ProvisionedProjectGit {
            remote_url,
            default_branch: if repository.default_branch.trim().is_empty() {
                "main".into()
            } else {
                repository.default_branch
            },
            auth_profile,
            repository_identifier,
        })
    }
}

impl ProjectGitProvisioner for GitnessProjectGitProvisioner {
    fn provision(&self, request: ProjectGitProvisionRequest) -> AppResult<ProvisionedProjectGit> {
        let provisioner = self.clone();
        std::thread::spawn(move || provisioner.provision_blocking(request))
            .join()
            .map_err(|_| AppError::Validation("Gitness provider worker panicked".into()))?
    }
}

fn generated_repository_identifier(project_name: &str, project_id: Uuid) -> String {
    let mut slug = project_name
        .chars()
        .flat_map(char::to_lowercase)
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    while slug.contains("--") {
        slug = slug.replace("--", "-");
    }
    slug = slug.trim_matches('-').chars().take(36).collect();
    if slug.is_empty() {
        slug = "project".into();
    }
    format!("{}-{}", slug, &project_id.simple().to_string()[..8])
}

fn validate_repository_identifier(raw: &str) -> AppResult<String> {
    let value = raw.trim().to_ascii_lowercase();
    if value.is_empty()
        || value.chars().count() > 64
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-')
        || value.starts_with('-')
        || value.ends_with('-')
    {
        return Err(AppError::Validation(
            "repository_identifier must contain lowercase letters, numbers, or hyphens".into(),
        ));
    }
    Ok(value)
}

fn validate_parent_ref(raw: &str) -> AppResult<String> {
    let value = raw.trim();
    if value.is_empty()
        || value != raw
        || value.chars().count() > 512
        || value.split('/').any(|segment| {
            segment.is_empty()
                || !segment.chars().all(|character| {
                    character.is_ascii_alphanumeric() || matches!(character, '-' | '_')
                })
        })
    {
        return Err(AppError::Validation(
            "RELAY_GIT_PROVIDER_PARENT_REF is invalid".into(),
        ));
    }
    Ok(value.to_string())
}

fn validate_username(raw: &str) -> AppResult<String> {
    let value = raw.trim();
    if value.is_empty()
        || value != raw
        || value.chars().count() > 256
        || value
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
    {
        return Err(AppError::Validation(
            "RELAY_GIT_PROVIDER_USERNAME is invalid".into(),
        ));
    }
    Ok(value.to_string())
}

fn validate_token(raw: &str) -> AppResult<()> {
    if !(20..=4096).contains(&raw.chars().count())
        || raw
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
    {
        return Err(AppError::Validation(
            "RELAY_GIT_PROVIDER_TOKEN is invalid".into(),
        ));
    }
    Ok(())
}

fn required_env(name: &str) -> AppResult<String> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| AppError::Validation(format!("{name} is required")))
}

fn response_error(action: &str, response: reqwest::blocking::Response) -> AppError {
    let status = response.status();
    let detail = response
        .text()
        .unwrap_or_default()
        .chars()
        .take(500)
        .collect::<String>();
    AppError::Validation(format!("failed to {action}: HTTP {status} {detail}"))
}

fn provider_error(error: impl std::fmt::Display) -> AppError {
    AppError::Validation(format!("Gitness provider error: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_identifier_is_safe_and_project_specific() {
        let project_id = Uuid::parse_str("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee").unwrap();
        assert_eq!(
            generated_repository_identifier("产品 设计 / Web", project_id),
            "web-aaaaaaaa"
        );
    }
}
