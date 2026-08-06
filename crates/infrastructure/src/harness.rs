use std::{
    collections::HashMap,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

use chrono::Utc;
use reqwest::{Method, StatusCode};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use ai_chat_application::PlatformRepository;
use ai_chat_domain::agent_identity::{
    HumanHarnessAccount, HumanUser, HUMAN_HARNESS_STATUS_ACTIVE, HUMAN_HARNESS_STATUS_FAILED,
    HUMAN_HARNESS_STATUS_PROVISIONING,
};
use ai_chat_shared::{AppError, AppResult};

use crate::config::{ApiConfig, HarnessMode};
use crate::git_credentials::GitCredentialStore;
use crate::gitness::{generated_repository_identifier, ProvisionedProjectGit};

const MAX_HARNESS_ERROR_CHARS: usize = 1_000;

#[derive(Debug, Clone)]
struct HarnessProvisioningConfig {
    mode: HarnessMode,
    api_base_url: Option<String>,
    public_base_url: Option<String>,
    space_prefix: String,
    admin_email: Option<String>,
    admin_password: Option<String>,
}

#[derive(Clone)]
pub struct HarnessProvisioner<R> {
    repo: R,
    config: HarnessProvisioningConfig,
    credentials: HarnessCredentialStore,
    client: reqwest::Client,
    user_locks: Arc<Mutex<HashMap<Uuid, Arc<tokio::sync::Mutex<()>>>>>,
}

impl<R: PlatformRepository> HarnessProvisioner<R> {
    pub fn from_config(repo: R, config: &ApiConfig) -> AppResult<Self> {
        let credentials =
            HarnessCredentialStore::at(PathBuf::from(config.harness_credentials_root.as_str()))?;
        let request_timeout = Duration::from_secs(config.harness_request_timeout_seconds);
        let client = reqwest::Client::builder()
            .connect_timeout(request_timeout.min(Duration::from_secs(10)))
            .timeout(request_timeout)
            .build()
            .map_err(|error| AppError::Internal(format!("Harness HTTP client error: {error}")))?;
        Ok(Self {
            repo,
            config: HarnessProvisioningConfig {
                mode: config.harness_mode.clone(),
                api_base_url: config.harness_base_url.clone(),
                public_base_url: config.harness_public_base_url.clone(),
                space_prefix: config.harness_space_prefix.clone(),
                admin_email: config.harness_admin_email.clone(),
                admin_password: config.harness_admin_password.clone(),
            },
            credentials,
            client,
            user_locks: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub fn is_enabled(&self) -> bool {
        self.config.mode != HarnessMode::Disabled
    }

    pub fn mode_key(&self) -> &'static str {
        self.config.mode.provider_key().unwrap_or("disabled")
    }

    pub fn account(&self, human_user_id: Uuid) -> AppResult<Option<HumanHarnessAccount>> {
        self.repo.get_human_harness_account_result(human_user_id)
    }

    pub async fn ensure_account(&self, user: &HumanUser) -> AppResult<Option<HumanHarnessAccount>> {
        let Some(provider_mode) = self.config.mode.provider_key() else {
            return Ok(None);
        };
        let user_lock = {
            let mut locks = self
                .user_locks
                .lock()
                .map_err(|_| AppError::Internal("Harness provisioning lock was poisoned".into()))?;
            locks
                .entry(user.id)
                .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
                .clone()
        };
        let _user_guard = user_lock.lock().await;
        let api_base_url = self.config.api_base_url.as_deref().ok_or_else(|| {
            AppError::Validation(
                "HARNESS_BASE_URL is required when HARNESS_MODE is official or self_hosted".into(),
            )
        })?;
        let public_base_url = self
            .config
            .public_base_url
            .as_deref()
            .unwrap_or(api_base_url);
        let identity = HarnessIdentity::for_user(user, self.config.space_prefix.as_str());
        let prior = self.repo.get_human_harness_account_result(user.id)?;
        if prior.as_ref().is_some_and(|account| {
            account.status == HUMAN_HARNESS_STATUS_ACTIVE
                && account.provider_mode == provider_mode
                && account.harness_base_url == public_base_url
                && account.harness_uid == identity.uid
                && account.space_identifier == identity.space_identifier
                && self.credentials.has_access_token(user.id)
        }) {
            return Ok(prior);
        }

        let now = Utc::now();
        let mut account = HumanHarnessAccount {
            human_user_id: user.id,
            provider_mode: provider_mode.to_string(),
            harness_base_url: public_base_url.to_string(),
            harness_uid: identity.uid.clone(),
            harness_email: identity.email.clone(),
            space_identifier: identity.space_identifier.clone(),
            status: HUMAN_HARNESS_STATUS_PROVISIONING.to_string(),
            attempt_count: prior
                .as_ref()
                .map(|account| account.attempt_count.saturating_add(1))
                .unwrap_or(1),
            last_error: None,
            last_attempt_at: Some(now),
            provisioned_at: prior.as_ref().and_then(|account| account.provisioned_at),
            created_at: prior
                .as_ref()
                .map(|account| account.created_at)
                .unwrap_or(now),
            updated_at: now,
        };

        let password = match self.credentials.read_password(user.id)? {
            Some(password) => password,
            None => {
                let password = generate_harness_password();
                self.credentials
                    .store_password(user.id, password.as_str())?;
                password
            }
        };
        self.repo.upsert_human_harness_account(account.clone())?;

        match self
            .provision_inner(api_base_url, user, &identity, password.as_str())
            .await
        {
            Ok(access_token) => {
                self.credentials
                    .store_access_token(user.id, access_token.as_str())?;
                let completed_at = Utc::now();
                account.status = HUMAN_HARNESS_STATUS_ACTIVE.to_string();
                account.last_error = None;
                account.provisioned_at = Some(completed_at);
                account.updated_at = completed_at;
                self.repo.upsert_human_harness_account(account.clone())?;
                Ok(Some(account))
            }
            Err(error) => {
                account.status = HUMAN_HARNESS_STATUS_FAILED.to_string();
                account.last_error = Some(truncate_error(error.to_string().as_str()));
                account.updated_at = Utc::now();
                self.repo.upsert_human_harness_account(account.clone())?;
                Err(AppError::Internal(format!(
                    "Harness account provisioning failed: {error}"
                )))
            }
        }
    }

    pub async fn ensure_active_account(&self, user: &HumanUser) -> AppResult<HumanHarnessAccount> {
        self.ensure_account(user)
            .await?
            .filter(|account| account.status == HUMAN_HARNESS_STATUS_ACTIVE)
            .ok_or_else(|| {
                AppError::Validation("当前用户的 Harness 账户尚未就绪，无法自动创建项目仓库".into())
            })
    }

    pub async fn provision_project_git(
        &self,
        human_user_id: Uuid,
        project_id: Uuid,
        project_name: &str,
        description: &str,
        git_credentials: &GitCredentialStore,
    ) -> AppResult<ProvisionedProjectGit> {
        let account = self
            .repo
            .get_human_harness_account_result(human_user_id)?
            .filter(|account| account.status == HUMAN_HARNESS_STATUS_ACTIVE)
            .ok_or_else(|| {
                AppError::Validation("当前用户的 Harness 账户尚未就绪，无法自动创建项目仓库".into())
            })?;
        let access_token = self
            .credentials
            .read_access_token(human_user_id)?
            .ok_or_else(|| AppError::Validation("Harness access token is unavailable".into()))?;
        let api_base_url = self.config.api_base_url.as_deref().ok_or_else(|| {
            AppError::Validation("HARNESS_BASE_URL is required for Git provisioning".into())
        })?;
        let repository_identifier = generated_repository_identifier(project_name, project_id);
        let create = HarnessCreateRepositoryRequest {
            parent_ref: account.space_identifier.as_str(),
            identifier: repository_identifier.as_str(),
            default_branch: "main",
            description,
            is_public: false,
            readme: false,
        };
        let repository = match self
            .request_json::<HarnessRepositoryResponse, _>(
                Method::POST,
                format!("{api_base_url}/api/v1/repos").as_str(),
                Some(access_token.as_str()),
                Some(&create),
            )
            .await
        {
            Ok(repository) => repository,
            Err(error) if error.is_already_exists() => self
                .request_json::<HarnessRepositoryResponse, ()>(
                    Method::GET,
                    format!(
                        "{api_base_url}/api/v1/repos/{}%2F{}",
                        account.space_identifier.replace('/', "%2F"),
                        repository_identifier
                    )
                    .as_str(),
                    Some(access_token.as_str()),
                    None,
                )
                .await
                .map_err(|error| AppError::Internal(format!("read Harness repository: {error}")))?,
            Err(error) => {
                return Err(AppError::Internal(format!(
                    "create Harness repository: {error}"
                )))
            }
        };
        let token_identifier = format!(
            "relay-project-{}-{}",
            short_identifier(project_id),
            short_identifier(Uuid::new_v4())
        );
        let token_request = HarnessCreateAccessTokenRequest {
            identifier: token_identifier.as_str(),
        };
        let project_token = self
            .request_json::<HarnessTokenResponse, _>(
                Method::POST,
                format!("{api_base_url}/api/v1/user/tokens").as_str(),
                Some(access_token.as_str()),
                Some(&token_request),
            )
            .await
            .map_err(|error| {
                AppError::Internal(format!("create Harness project token: {error}"))
            })?;
        let auth_profile = git_credentials.store_managed_git_token(
            project_id,
            account.harness_uid.as_str(),
            project_token.access_token.as_str(),
        )?;
        let public_base =
            reqwest::Url::parse(account.harness_base_url.as_str()).map_err(|error| {
                AppError::Validation(format!("invalid Harness public URL: {error}"))
            })?;
        let remote_url = public_base
            .join(&format!(
                "git/{}/{}.git",
                account.space_identifier, repository_identifier
            ))
            .map_err(|error| AppError::Validation(format!("invalid Harness Git URL: {error}")))?
            .to_string();
        let internal_base = reqwest::Url::parse(api_base_url).map_err(|error| {
            AppError::Validation(format!("invalid Harness internal URL: {error}"))
        })?;
        let push_url = internal_base
            .join(&format!(
                "git/{}/{}.git",
                account.space_identifier, repository_identifier
            ))
            .map_err(|error| AppError::Validation(format!("invalid Harness push URL: {error}")))?
            .to_string();
        Ok(ProvisionedProjectGit {
            remote_url,
            push_url: Some(push_url),
            default_branch: if repository.default_branch.trim().is_empty() {
                "main".into()
            } else {
                repository.default_branch
            },
            auth_profile,
            repository_identifier,
        })
    }

    async fn provision_inner(
        &self,
        base_url: &str,
        user: &HumanUser,
        identity: &HarnessIdentity,
        password: &str,
    ) -> Result<String, HarnessRequestError> {
        let login_token = self
            .register_or_login(base_url, user, identity, password)
            .await?;
        self.ensure_space(base_url, login_token.as_str(), identity, user)
            .await?;
        self.create_access_token(base_url, login_token.as_str(), user)
            .await
    }

    async fn register_or_login(
        &self,
        base_url: &str,
        user: &HumanUser,
        identity: &HarnessIdentity,
        password: &str,
    ) -> Result<String, HarnessRequestError> {
        let request = HarnessRegisterRequest {
            uid: identity.uid.as_str(),
            email: identity.email.as_str(),
            display_name: user.display_name.as_str(),
            password,
        };
        match self
            .request_json::<HarnessTokenResponse, _>(
                Method::POST,
                format!("{base_url}/api/v1/register").as_str(),
                None,
                Some(&request),
            )
            .await
        {
            Ok(response) => non_empty_token(response.access_token),
            Err(error) if error.is_already_exists() => {
                match self.login(base_url, identity.uid.as_str(), password).await {
                    Ok(token) => Ok(token),
                    Err(login_error) => {
                        self.reset_password_with_admin(
                            base_url,
                            identity.uid.as_str(),
                            password,
                            &login_error,
                        )
                        .await?;
                        self.login(base_url, identity.uid.as_str(), password).await
                    }
                }
            }
            Err(error) => Err(error),
        }
    }

    async fn login(
        &self,
        base_url: &str,
        login_identifier: &str,
        password: &str,
    ) -> Result<String, HarnessRequestError> {
        let request = HarnessLoginRequest {
            login_identifier,
            password,
        };
        let response = self
            .request_json::<HarnessTokenResponse, _>(
                Method::POST,
                format!("{base_url}/api/v1/login").as_str(),
                None,
                Some(&request),
            )
            .await?;
        non_empty_token(response.access_token)
    }

    async fn reset_password_with_admin(
        &self,
        base_url: &str,
        user_uid: &str,
        password: &str,
        login_error: &HarnessRequestError,
    ) -> Result<(), HarnessRequestError> {
        let Some(admin_email) = self.config.admin_email.as_deref() else {
            return Err(HarnessRequestError::transport(format!(
                "Harness user login failed and HARNESS_ADMIN_EMAIL is not configured: {login_error}"
            )));
        };
        let Some(admin_password) = self.config.admin_password.as_deref() else {
            return Err(HarnessRequestError::transport(format!(
                "Harness user login failed and HARNESS_ADMIN_PASSWORD is not configured: {login_error}"
            )));
        };
        let admin_token = self.login(base_url, admin_email, admin_password).await?;
        let request = HarnessUpdateUserRequest { password };
        self.request_json::<Value, _>(
            Method::PATCH,
            format!("{base_url}/api/v1/admin/users/{user_uid}").as_str(),
            Some(admin_token.as_str()),
            Some(&request),
        )
        .await
        .map(|_| ())
    }

    async fn ensure_space(
        &self,
        base_url: &str,
        access_token: &str,
        identity: &HarnessIdentity,
        user: &HumanUser,
    ) -> Result<(), HarnessRequestError> {
        let description = format!("Relay workspace for {}", user.display_name);
        let request = HarnessCreateSpaceRequest {
            identifier: identity.space_identifier.as_str(),
            parent_ref: "",
            description: description.as_str(),
            is_public: false,
        };
        match self
            .request_json::<Value, _>(
                Method::POST,
                format!("{base_url}/api/v1/spaces").as_str(),
                Some(access_token),
                Some(&request),
            )
            .await
        {
            Ok(_) => Ok(()),
            Err(error) if error.is_already_exists() => self
                .request_json::<Value, ()>(
                    Method::GET,
                    format!("{base_url}/api/v1/spaces/{}", identity.space_identifier).as_str(),
                    Some(access_token),
                    None,
                )
                .await
                .map(|_| ()),
            Err(error) => Err(error),
        }
    }

    async fn create_access_token(
        &self,
        base_url: &str,
        access_token: &str,
        user: &HumanUser,
    ) -> Result<String, HarnessRequestError> {
        let identifier = format!(
            "relay-{}-{}",
            short_identifier(user.id),
            short_identifier(Uuid::new_v4())
        );
        let request = HarnessCreateAccessTokenRequest {
            identifier: identifier.as_str(),
        };
        let response = self
            .request_json::<HarnessTokenResponse, _>(
                Method::POST,
                format!("{base_url}/api/v1/user/tokens").as_str(),
                Some(access_token),
                Some(&request),
            )
            .await?;
        non_empty_token(response.access_token)
    }

    async fn request_json<TResponse, TBody>(
        &self,
        method: Method,
        endpoint: &str,
        bearer_token: Option<&str>,
        body: Option<&TBody>,
    ) -> Result<TResponse, HarnessRequestError>
    where
        TResponse: DeserializeOwned,
        TBody: Serialize + ?Sized,
    {
        let mut request = self.client.request(method, endpoint);
        if let Some(token) = bearer_token {
            request = request.bearer_auth(token);
        }
        if let Some(body) = body {
            request = request.json(body);
        }
        let response = request
            .send()
            .await
            .map_err(HarnessRequestError::transport)?;
        let status = response.status();
        if !status.is_success() {
            let message = response
                .text()
                .await
                .unwrap_or_else(|error| error.to_string());
            return Err(HarnessRequestError {
                status: Some(status),
                message: truncate_error(message.as_str()),
            });
        }
        response
            .json::<TResponse>()
            .await
            .map_err(HarnessRequestError::transport)
    }
}

#[derive(Debug)]
struct HarnessIdentity {
    uid: String,
    email: String,
    space_identifier: String,
}

impl HarnessIdentity {
    fn for_user(user: &HumanUser, space_prefix: &str) -> Self {
        let uid = format!("relay-{}", short_identifier(user.id));
        let prefix = sanitize_identifier(space_prefix, "u-");
        Self {
            uid: uid.clone(),
            email: user.email.trim().to_ascii_lowercase(),
            space_identifier: format!("{prefix}{uid}"),
        }
    }
}

#[derive(Debug, Serialize)]
struct HarnessRegisterRequest<'a> {
    uid: &'a str,
    email: &'a str,
    display_name: &'a str,
    password: &'a str,
}

#[derive(Debug, Serialize)]
struct HarnessLoginRequest<'a> {
    login_identifier: &'a str,
    password: &'a str,
}

#[derive(Debug, Serialize)]
struct HarnessUpdateUserRequest<'a> {
    password: &'a str,
}

#[derive(Debug, Serialize)]
struct HarnessCreateSpaceRequest<'a> {
    identifier: &'a str,
    parent_ref: &'a str,
    description: &'a str,
    is_public: bool,
}

#[derive(Debug, Serialize)]
struct HarnessCreateAccessTokenRequest<'a> {
    identifier: &'a str,
}

#[derive(Debug, Serialize)]
struct HarnessCreateRepositoryRequest<'a> {
    parent_ref: &'a str,
    identifier: &'a str,
    default_branch: &'a str,
    description: &'a str,
    is_public: bool,
    readme: bool,
}

#[derive(Debug, Deserialize)]
struct HarnessRepositoryResponse {
    default_branch: String,
}

#[derive(Debug, Deserialize)]
struct HarnessTokenResponse {
    access_token: String,
}

#[derive(Debug)]
struct HarnessRequestError {
    status: Option<StatusCode>,
    message: String,
}

impl HarnessRequestError {
    fn transport(error: impl std::fmt::Display) -> Self {
        Self {
            status: None,
            message: error.to_string(),
        }
    }

    fn is_already_exists(&self) -> bool {
        let message = self.message.to_ascii_lowercase();
        self.status == Some(StatusCode::CONFLICT)
            || message.contains("already")
            || message.contains("exist")
            || message.contains("duplicate")
            || message.contains("unique")
    }
}

impl std::fmt::Display for HarnessRequestError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.status {
            Some(status) => write!(formatter, "{} {}", status.as_u16(), self.message),
            None => formatter.write_str(self.message.as_str()),
        }
    }
}

fn non_empty_token(token: String) -> Result<String, HarnessRequestError> {
    let token = token.trim().to_string();
    if token.is_empty() {
        Err(HarnessRequestError::transport(
            "Harness response did not contain an access token",
        ))
    } else {
        Ok(token)
    }
}

fn short_identifier(id: Uuid) -> String {
    id.simple().to_string().chars().take(12).collect()
}

fn sanitize_identifier(value: &str, fallback: &str) -> String {
    let value = value.trim().to_ascii_lowercase();
    let mut normalized = String::new();
    for character in value.chars() {
        if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.') {
            normalized.push(character);
        }
    }
    if normalized.is_empty() {
        fallback.to_string()
    } else {
        normalized
    }
}

fn generate_harness_password() -> String {
    format!(
        "Relay-{}-{}!",
        Uuid::new_v4().simple(),
        Uuid::new_v4().simple()
    )
}

fn truncate_error(error: &str) -> String {
    error.trim().chars().take(MAX_HARNESS_ERROR_CHARS).collect()
}

#[derive(Clone)]
struct HarnessCredentialStore {
    root: PathBuf,
}

impl HarnessCredentialStore {
    fn at(root: PathBuf) -> AppResult<Self> {
        let root = if root.is_absolute() {
            root
        } else {
            std::env::current_dir()
                .map_err(credential_error)?
                .join(root)
        };
        ensure_private_directory(root.as_path())?;
        Ok(Self { root })
    }

    fn password_path(&self, human_user_id: Uuid) -> PathBuf {
        self.root
            .join(format!("{}.password", human_user_id.simple()))
    }

    fn token_path(&self, human_user_id: Uuid) -> PathBuf {
        self.root
            .join(format!("{}.access-token", human_user_id.simple()))
    }

    fn read_password(&self, human_user_id: Uuid) -> AppResult<Option<String>> {
        read_secret(self.password_path(human_user_id).as_path())
    }

    fn store_password(&self, human_user_id: Uuid, password: &str) -> AppResult<()> {
        atomic_write_secret(self.password_path(human_user_id).as_path(), password)
    }

    #[cfg(test)]
    fn remove_password(&self, human_user_id: Uuid) -> AppResult<()> {
        remove_secret(self.password_path(human_user_id).as_path())
    }

    fn store_access_token(&self, human_user_id: Uuid, token: &str) -> AppResult<()> {
        atomic_write_secret(self.token_path(human_user_id).as_path(), token)
    }

    fn has_access_token(&self, human_user_id: Uuid) -> bool {
        self.token_path(human_user_id).is_file()
    }

    fn read_access_token(&self, human_user_id: Uuid) -> AppResult<Option<String>> {
        read_secret(self.token_path(human_user_id).as_path())
    }

    #[cfg(test)]
    fn remove_access_token(&self, human_user_id: Uuid) -> AppResult<()> {
        remove_secret(self.token_path(human_user_id).as_path())
    }
}

fn ensure_private_directory(path: &Path) -> AppResult<()> {
    fs::create_dir_all(path).map_err(credential_error)?;
    set_mode(path, 0o700)
}

fn read_secret(path: &Path) -> AppResult<Option<String>> {
    match fs::read_to_string(path) {
        Ok(secret) => {
            let secret = secret.trim().to_string();
            if secret.is_empty() {
                Err(AppError::Internal(format!(
                    "Harness credential file is empty: {}",
                    path.display()
                )))
            } else {
                Ok(Some(secret))
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(credential_error(error)),
    }
}

fn atomic_write_secret(path: &Path, secret: &str) -> AppResult<()> {
    if secret.trim().is_empty() || secret.chars().count() > 8_192 {
        return Err(AppError::Validation(
            "Harness credential must contain 1 to 8192 characters".into(),
        ));
    }
    let parent = path
        .parent()
        .ok_or_else(|| AppError::Internal("Harness credential path has no parent".into()))?;
    ensure_private_directory(parent)?;
    let temporary_path = parent.join(format!(".tmp-{}", Uuid::new_v4().simple()));
    let result = (|| -> AppResult<()> {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary_path).map_err(credential_error)?;
        file.write_all(secret.as_bytes())
            .map_err(credential_error)?;
        file.sync_all().map_err(credential_error)?;
        set_mode(temporary_path.as_path(), 0o600)?;
        fs::rename(temporary_path.as_path(), path).map_err(credential_error)
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary_path);
    }
    result
}

#[cfg(test)]
fn remove_secret(path: &Path) -> AppResult<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(credential_error(error)),
    }
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> AppResult<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).map_err(credential_error)
}

#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: u32) -> AppResult<()> {
    Ok(())
}

fn credential_error(error: std::io::Error) -> AppError {
    AppError::Internal(format!("Harness credential filesystem error: {error}"))
}

#[cfg(test)]
mod tests;
