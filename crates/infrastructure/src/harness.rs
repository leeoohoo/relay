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

const MAX_HARNESS_ERROR_CHARS: usize = 1_000;

#[derive(Debug, Clone)]
struct HarnessProvisioningConfig {
    mode: HarnessMode,
    api_base_url: Option<String>,
    public_base_url: Option<String>,
    space_prefix: String,
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
                self.credentials.remove_password(user.id)?;
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
                let request = HarnessLoginRequest {
                    login_identifier: identity.uid.as_str(),
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
            Err(error) => Err(error),
        }
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

    fn remove_password(&self, human_user_id: Uuid) -> AppResult<()> {
        remove_secret(self.password_path(human_user_id).as_path())
    }

    fn store_access_token(&self, human_user_id: Uuid, token: &str) -> AppResult<()> {
        atomic_write_secret(self.token_path(human_user_id).as_path(), token)
    }

    fn has_access_token(&self, human_user_id: Uuid) -> bool {
        self.token_path(human_user_id).is_file()
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
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    use ai_chat_application::{AuthPlatformRepository, MemoryPlatformRepository};
    use axum::{routing::post, Json, Router};

    #[test]
    fn identity_is_stable_and_unique_per_human() {
        let user = HumanUser {
            id: Uuid::parse_str("12345678-90ab-cdef-1234-567890abcdef").unwrap(),
            email: "Human@Example.com".into(),
            display_name: "Human".into(),
            created_at: Utc::now(),
        };
        let identity = HarnessIdentity::for_user(&user, "u-");
        assert_eq!(identity.uid, "relay-1234567890ab");
        assert_eq!(identity.email, "human@example.com");
        assert_eq!(identity.space_identifier, "u-relay-1234567890ab");
    }

    #[test]
    fn credential_store_keeps_secrets_out_of_repository_records() {
        let root = std::env::temp_dir().join(format!(
            "relay-harness-credentials-{}",
            Uuid::new_v4().simple()
        ));
        let store = HarnessCredentialStore::at(root.clone()).unwrap();
        let user_id = Uuid::new_v4();
        store.store_password(user_id, "secret-password").unwrap();
        assert_eq!(
            store.read_password(user_id).unwrap().as_deref(),
            Some("secret-password")
        );
        store.store_access_token(user_id, "secret-token").unwrap();
        assert!(store.has_access_token(user_id));
        store.remove_password(user_id).unwrap();
        assert_eq!(store.read_password(user_id).unwrap(), None);
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn provisions_once_and_reuses_the_persisted_harness_account() {
        let register_calls = Arc::new(AtomicUsize::new(0));
        let register_counter = register_calls.clone();
        let app = Router::new()
            .route(
                "/api/v1/register",
                post(move || {
                    register_counter.fetch_add(1, Ordering::SeqCst);
                    async { Json(serde_json::json!({"access_token": "login-token"})) }
                }),
            )
            .route(
                "/api/v1/spaces",
                post(|| async { Json(serde_json::json!({"identifier": "space"})) }),
            )
            .route(
                "/api/v1/user/tokens",
                post(|| async { Json(serde_json::json!({"access_token": "project-token"})) }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

        let credentials_root = std::env::temp_dir().join(format!(
            "relay-harness-provision-test-{}",
            Uuid::new_v4().simple()
        ));
        let repo = MemoryPlatformRepository::default();
        let provisioner = HarnessProvisioner {
            repo: repo.clone(),
            config: HarnessProvisioningConfig {
                mode: HarnessMode::Official,
                api_base_url: Some(format!("http://{address}")),
                public_base_url: Some("https://harness.example.test".into()),
                space_prefix: "u-".into(),
            },
            credentials: HarnessCredentialStore::at(credentials_root.clone()).unwrap(),
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .unwrap(),
            user_locks: Arc::new(Mutex::new(HashMap::new())),
        };
        let user = HumanUser {
            id: Uuid::new_v4(),
            email: "human@example.test".into(),
            display_name: "Human".into(),
            created_at: Utc::now(),
        };
        repo.insert_human_user(user.clone()).unwrap();

        let first = provisioner.ensure_account(&user).await.unwrap().unwrap();
        assert_eq!(first.status, HUMAN_HARNESS_STATUS_ACTIVE);
        assert_eq!(first.provider_mode, "official");
        assert_eq!(first.harness_base_url, "https://harness.example.test");
        assert!(provisioner.credentials.has_access_token(user.id));

        let second = provisioner.ensure_account(&user).await.unwrap().unwrap();
        assert_eq!(second.attempt_count, 1);
        assert_eq!(register_calls.load(Ordering::SeqCst), 1);

        server.abort();
        let _ = fs::remove_dir_all(credentials_root);
    }

    #[tokio::test]
    async fn failed_provisioning_is_persisted_and_can_be_retried() {
        let register_calls = Arc::new(AtomicUsize::new(0));
        let register_counter = register_calls.clone();
        let app = Router::new()
            .route(
                "/api/v1/register",
                post(move || {
                    let attempt = register_counter.fetch_add(1, Ordering::SeqCst);
                    async move {
                        if attempt == 0 {
                            Err(StatusCode::SERVICE_UNAVAILABLE)
                        } else {
                            Ok(Json(serde_json::json!({"access_token": "login-token"})))
                        }
                    }
                }),
            )
            .route(
                "/api/v1/spaces",
                post(|| async { Json(serde_json::json!({"identifier": "space"})) }),
            )
            .route(
                "/api/v1/user/tokens",
                post(|| async { Json(serde_json::json!({"access_token": "project-token"})) }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

        let credentials_root = std::env::temp_dir().join(format!(
            "relay-harness-retry-test-{}",
            Uuid::new_v4().simple()
        ));
        let repo = MemoryPlatformRepository::default();
        let provisioner = HarnessProvisioner {
            repo: repo.clone(),
            config: HarnessProvisioningConfig {
                mode: HarnessMode::SelfHosted,
                api_base_url: Some(format!("http://{address}")),
                public_base_url: Some("http://127.0.0.1:3000".into()),
                space_prefix: "u-".into(),
            },
            credentials: HarnessCredentialStore::at(credentials_root.clone()).unwrap(),
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .unwrap(),
            user_locks: Arc::new(Mutex::new(HashMap::new())),
        };
        let user = HumanUser {
            id: Uuid::new_v4(),
            email: "retry@example.test".into(),
            display_name: "Retry Human".into(),
            created_at: Utc::now(),
        };
        repo.insert_human_user(user.clone()).unwrap();

        assert!(provisioner.ensure_account(&user).await.is_err());
        let failed = provisioner.account(user.id).unwrap().unwrap();
        assert_eq!(failed.status, HUMAN_HARNESS_STATUS_FAILED);
        assert_eq!(failed.attempt_count, 1);
        assert!(provisioner
            .credentials
            .read_password(user.id)
            .unwrap()
            .is_some());

        let active = provisioner.ensure_account(&user).await.unwrap().unwrap();
        assert_eq!(active.status, HUMAN_HARNESS_STATUS_ACTIVE);
        assert_eq!(active.attempt_count, 2);
        assert_eq!(register_calls.load(Ordering::SeqCst), 2);
        assert_eq!(
            provisioner.credentials.read_password(user.id).unwrap(),
            None
        );

        server.abort();
        let _ = fs::remove_dir_all(credentials_root);
    }
}
