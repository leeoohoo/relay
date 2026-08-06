use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OwnershipProofMode {
    Stub,
    Manual,
    Remote,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HarnessMode {
    Disabled,
    Official,
    SelfHosted,
}

impl HarnessMode {
    fn from_env_value(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().replace('-', "_").as_str() {
            "official" | "hosted" | "cloud" => Self::Official,
            "self_hosted" | "local" | "docker" => Self::SelfHosted,
            _ => Self::Disabled,
        }
    }

    pub fn provider_key(&self) -> Option<&'static str> {
        match self {
            Self::Disabled => None,
            Self::Official => Some("official"),
            Self::SelfHosted => Some("self_hosted"),
        }
    }
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
    pub harness_mode: HarnessMode,
    pub harness_base_url: Option<String>,
    pub harness_public_base_url: Option<String>,
    pub harness_space_prefix: String,
    pub harness_request_timeout_seconds: u64,
    pub harness_credentials_root: String,
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
        let harness_mode = std::env::var("HARNESS_MODE")
            .map(|value| HarnessMode::from_env_value(&value))
            .unwrap_or(HarnessMode::Disabled);
        let harness_base_url = std::env::var("HARNESS_BASE_URL")
            .ok()
            .map(|value| value.trim().trim_end_matches('/').to_string())
            .filter(|value| !value.is_empty());
        let harness_public_base_url = std::env::var("HARNESS_PUBLIC_BASE_URL")
            .ok()
            .map(|value| value.trim().trim_end_matches('/').to_string())
            .filter(|value| !value.is_empty())
            .or_else(|| harness_base_url.clone());
        let harness_space_prefix =
            std::env::var("HARNESS_SPACE_PREFIX").unwrap_or_else(|_| "u-".into());
        let harness_request_timeout_seconds = std::env::var("HARNESS_REQUEST_TIMEOUT_SECONDS")
            .ok()
            .and_then(|value| value.parse().ok())
            .filter(|value| *value > 0)
            .unwrap_or(15);
        let harness_credentials_root = std::env::var("HARNESS_CREDENTIALS_ROOT")
            .unwrap_or_else(|_| ".relay/harness-credentials".into());

        Self {
            host,
            port,
            database_url,
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
            harness_mode,
            harness_base_url,
            harness_public_base_url,
            harness_space_prefix,
            harness_request_timeout_seconds,
            harness_credentials_root,
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
