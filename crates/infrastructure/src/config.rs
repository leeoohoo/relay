use serde::{Deserialize, Serialize};

#[derive(Clone, Default)]
pub struct RuntimeSecretResolverConfig {
    pub secrets_json: Option<String>,
    pub vault_addr: Option<String>,
    pub vault_token: Option<String>,
    pub vault_namespace: Option<String>,
    pub allow_insecure_vault_http: bool,
    pub vault_timeout_seconds: u64,
}

impl std::fmt::Debug for RuntimeSecretResolverConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RuntimeSecretResolverConfig")
            .field("secrets_json_configured", &self.secrets_json.is_some())
            .field("vault_addr", &self.vault_addr)
            .field("vault_token_configured", &self.vault_token.is_some())
            .field("vault_namespace", &self.vault_namespace)
            .field("allow_insecure_vault_http", &self.allow_insecure_vault_http)
            .field("vault_timeout_seconds", &self.vault_timeout_seconds)
            .finish()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepositoryMode {
    Memory,
    Postgres,
}

impl RepositoryMode {
    fn from_env_value(value: &str) -> Self {
        match value.trim().to_lowercase().as_str() {
            "postgres" | "pg" => Self::Postgres,
            _ => Self::Memory,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OwnershipProofMode {
    Stub,
    Manual,
    Remote,
}

impl OwnershipProofMode {
    fn from_env_value(value: &str) -> Self {
        match value.trim().to_lowercase().as_str() {
            "manual" => Self::Manual,
            "remote" => Self::Remote,
            _ => Self::Stub,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    pub host: String,
    pub port: u16,
    pub database_url: String,
    pub repository_mode: RepositoryMode,
    pub ownership_proof_mode: OwnershipProofMode,
    pub ownership_proof_remote_url: Option<String>,
    pub ownership_proof_remote_token: Option<String>,
    pub enable_dev_endpoints: bool,
    pub admin_api_token: Option<String>,
    pub require_email_verification: bool,
    pub email_delivery_webhook_url: Option<String>,
    pub public_base_url: String,
    pub allowed_origins: Vec<String>,
    pub request_timeout_seconds: u64,
    pub max_request_body_bytes: usize,
    pub app_env: String,
    pub enable_autonomy_worker: bool,
    pub autonomy_worker_interval_seconds: u64,
    pub autonomy_worker_batch_size: usize,
    pub agent_runtime_openai_base_url: String,
    pub agent_runtime_allow_insecure_provider_http: bool,
    #[serde(skip)]
    pub agent_runtime_secret_resolver: RuntimeSecretResolverConfig,
}

impl ApiConfig {
    pub fn from_env() -> Self {
        let host = std::env::var("API_HOST").unwrap_or_else(|_| "0.0.0.0".into());
        let port = std::env::var("API_PORT")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(8080);
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://postgres:postgres@127.0.0.1:5432/ai_chat".into());
        let repository_mode = std::env::var("REPOSITORY_MODE")
            .map(|value| RepositoryMode::from_env_value(&value))
            .unwrap_or(RepositoryMode::Memory);
        let ownership_proof_mode = std::env::var("WEIBO_PROOF_PROVIDER_MODE")
            .map(|value| OwnershipProofMode::from_env_value(&value))
            .unwrap_or(OwnershipProofMode::Stub);
        let ownership_proof_remote_url = std::env::var("WEIBO_PROOF_REMOTE_URL")
            .ok()
            .filter(|value| !value.trim().is_empty());
        let ownership_proof_remote_token = std::env::var("WEIBO_PROOF_REMOTE_TOKEN")
            .ok()
            .filter(|value| !value.trim().is_empty());
        let enable_dev_endpoints = std::env::var("ENABLE_DEV_ENDPOINTS")
            .map(|value| {
                matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                )
            })
            .unwrap_or(false);
        let admin_api_token = std::env::var("ADMIN_API_TOKEN")
            .ok()
            .filter(|value| !value.trim().is_empty());
        let require_email_verification = std::env::var("REQUIRE_EMAIL_VERIFICATION")
            .map(|value| {
                matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                )
            })
            .unwrap_or(false);
        let email_delivery_webhook_url = std::env::var("EMAIL_DELIVERY_WEBHOOK_URL")
            .ok()
            .filter(|value| !value.trim().is_empty());
        let public_base_url =
            std::env::var("PUBLIC_BASE_URL").unwrap_or_else(|_| format!("http://127.0.0.1:{port}"));
        let allowed_origins = csv_env("API_ALLOWED_ORIGINS", &[]);
        let request_timeout_seconds = std::env::var("API_REQUEST_TIMEOUT_SECONDS")
            .ok()
            .and_then(|value| value.parse().ok())
            .filter(|value| *value > 0)
            .unwrap_or(30);
        let max_request_body_bytes = std::env::var("API_MAX_REQUEST_BODY_BYTES")
            .ok()
            .and_then(|value| value.parse().ok())
            .filter(|value| *value > 0)
            .unwrap_or(1024 * 1024);
        let app_env = std::env::var("APP_ENV").unwrap_or_else(|_| "development".into());
        let enable_autonomy_worker = std::env::var("ENABLE_AUTONOMY_WORKER")
            .map(|value| {
                matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                )
            })
            .unwrap_or(false);
        let autonomy_worker_interval_seconds = std::env::var("AUTONOMY_WORKER_INTERVAL_SECONDS")
            .ok()
            .and_then(|value| value.parse().ok())
            .filter(|value| *value >= 5)
            .unwrap_or(30);
        let autonomy_worker_batch_size = std::env::var("AUTONOMY_WORKER_BATCH_SIZE")
            .or_else(|_| std::env::var("AUTONOMY_WORKER_MAX_EVENTS"))
            .ok()
            .and_then(|value| value.parse().ok())
            .map(|value: usize| value.clamp(1, 1_000))
            .unwrap_or(100);
        let agent_runtime_openai_base_url = std::env::var("AGENT_RUNTIME_OPENAI_BASE_URL")
            .unwrap_or_else(|_| "https://api.openai.com/v1".into());
        let is_production = app_env.eq_ignore_ascii_case("production");
        let agent_runtime_allow_insecure_provider_http =
            !is_production && bool_env("AGENT_RUNTIME_ALLOW_INSECURE_PROVIDER_HTTP", false);
        let agent_runtime_secret_resolver = RuntimeSecretResolverConfig {
            secrets_json: optional_env("AGENT_RUNTIME_SECRETS_JSON"),
            vault_addr: optional_env("AGENT_RUNTIME_VAULT_ADDR"),
            vault_token: optional_env("AGENT_RUNTIME_VAULT_TOKEN")
                .or_else(|| optional_env("VAULT_TOKEN")),
            vault_namespace: optional_env("AGENT_RUNTIME_VAULT_NAMESPACE"),
            allow_insecure_vault_http: !is_production
                && bool_env("AGENT_RUNTIME_ALLOW_INSECURE_VAULT_HTTP", false),
            vault_timeout_seconds: std::env::var("AGENT_RUNTIME_VAULT_TIMEOUT_SECONDS")
                .ok()
                .and_then(|value| value.parse().ok())
                .map(|value: u64| value.clamp(1, 30))
                .unwrap_or(5),
        };

        Self {
            host,
            port,
            database_url,
            repository_mode,
            ownership_proof_mode,
            ownership_proof_remote_url,
            ownership_proof_remote_token,
            enable_dev_endpoints,
            admin_api_token,
            require_email_verification,
            email_delivery_webhook_url,
            public_base_url,
            allowed_origins,
            request_timeout_seconds,
            max_request_body_bytes,
            app_env,
            enable_autonomy_worker,
            autonomy_worker_interval_seconds,
            autonomy_worker_batch_size,
            agent_runtime_openai_base_url,
            agent_runtime_allow_insecure_provider_http,
            agent_runtime_secret_resolver,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpConfig {
    pub api_base_url: String,
    pub agent_key: Option<String>,
    pub allowed_hosts: Vec<String>,
    pub allowed_origins: Vec<String>,
    pub database_url: String,
    pub repository_mode: RepositoryMode,
    pub ownership_proof_mode: OwnershipProofMode,
    pub ownership_proof_remote_url: Option<String>,
    pub ownership_proof_remote_token: Option<String>,
}

impl McpConfig {
    pub fn from_env() -> Self {
        Self {
            api_base_url: std::env::var("MCP_API_BASE_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:8080".into()),
            agent_key: std::env::var("MCP_AGENT_KEY").ok(),
            allowed_hosts: csv_env(
                "MCP_ALLOWED_HOSTS",
                &["localhost", "127.0.0.1", "::1", "0.0.0.0", "server"],
            ),
            allowed_origins: csv_env("MCP_ALLOWED_ORIGINS", &[]),
            database_url: std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgres://postgres:postgres@127.0.0.1:5432/ai_chat".into()),
            repository_mode: std::env::var("REPOSITORY_MODE")
                .map(|value| RepositoryMode::from_env_value(&value))
                .unwrap_or(RepositoryMode::Memory),
            ownership_proof_mode: std::env::var("WEIBO_PROOF_PROVIDER_MODE")
                .map(|value| OwnershipProofMode::from_env_value(&value))
                .unwrap_or(OwnershipProofMode::Stub),
            ownership_proof_remote_url: std::env::var("WEIBO_PROOF_REMOTE_URL")
                .ok()
                .filter(|value| !value.trim().is_empty()),
            ownership_proof_remote_token: std::env::var("WEIBO_PROOF_REMOTE_TOKEN")
                .ok()
                .filter(|value| !value.trim().is_empty()),
        }
    }
}

fn csv_env(name: &str, defaults: &[&str]) -> Vec<String> {
    std::env::var(name)
        .ok()
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .filter(|values| !values.is_empty())
        .unwrap_or_else(|| defaults.iter().map(|value| (*value).to_string()).collect())
}

fn optional_env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
}

fn bool_env(name: &str, default: bool) -> bool {
    std::env::var(name)
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(default)
}
