use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use ai_chat_shared::now_utc;

mod mcp;
mod requests;
mod storage;
mod store;
mod validation;

#[cfg(test)]
use mcp::validate_mcp_server_input;
#[cfg(test)]
use validation::normalize_codex_base_url;

pub const CODEX_AUTH_PROFILE_STATUS_PENDING: &str = "pending";
pub const CODEX_AUTH_PROFILE_STATUS_ACTIVE: &str = "active";
pub const CODEX_AUTH_PROFILE_STATUS_FAILED: &str = "failed";
pub const CODEX_AUTH_PROFILE_STATUS_DELETING: &str = "deleting";

pub const CODEX_CLI_OPERATION_IDLE: &str = "idle";
pub const CODEX_CLI_OPERATION_INSTALL_PENDING: &str = "install_pending";
pub const CODEX_CLI_OPERATION_INSTALLING: &str = "installing";
pub const CODEX_CLI_OPERATION_UPDATE_PENDING: &str = "update_pending";
pub const CODEX_CLI_OPERATION_UPDATING: &str = "updating";
pub const CODEX_CLI_OPERATION_FAILED: &str = "failed";

pub const CODEX_DEFAULT_AUTH_STATUS_UNKNOWN: &str = "unknown";
pub const CODEX_DEFAULT_AUTH_STATUS_ACTIVE: &str = "active";
pub const CODEX_DEFAULT_AUTH_STATUS_LOGGED_OUT: &str = "logged_out";

pub const CODEX_INSTALLER_POSIX_SHELL: &str = "posix_shell";
pub const CODEX_INSTALLER_POWERSHELL: &str = "powershell";
pub const CODEX_INSTALLER_UNSUPPORTED: &str = "unsupported";

pub const CODEX_MCP_TRANSPORT_STDIO: &str = "stdio";
pub const CODEX_MCP_TRANSPORT_HTTP: &str = "streamable_http";
pub const CODEX_MCP_OPERATION_IDLE: &str = "idle";
pub const CODEX_MCP_OPERATION_REFRESH_PENDING: &str = "refresh_pending";
pub const CODEX_MCP_OPERATION_REFRESHING: &str = "refreshing";
pub const CODEX_MCP_OPERATION_ADD_PENDING: &str = "add_pending";
pub const CODEX_MCP_OPERATION_ADDING: &str = "adding";
pub const CODEX_MCP_OPERATION_REMOVE_PENDING: &str = "remove_pending";
pub const CODEX_MCP_OPERATION_REMOVING: &str = "removing";
pub const CODEX_MCP_OPERATION_FAILED: &str = "failed";
pub const CODEX_DEFAULT_OPENAI_BASE_URL: &str = "https://api.openai.com/v1";

pub const CODEX_REASONING_SUMMARY_AUTO: &str = "auto";
pub const CODEX_WEB_SEARCH_CACHED: &str = "cached";
pub const CODEX_PERSONALITY_PRAGMATIC: &str = "pragmatic";
pub const CODEX_APPROVAL_POLICY_NEVER: &str = "never";
pub const CODEX_SANDBOX_WORKSPACE_WRITE: &str = "workspace_write";
pub const AGENT_TRIGGER_BATCH_SIZE_MIN: usize = 1;
pub const AGENT_TRIGGER_BATCH_SIZE_MAX: usize = 100;
pub const AGENT_TRIGGER_BATCH_SIZE_DEFAULT: usize = 2;

pub fn agent_trigger_batch_size_from_env() -> usize {
    std::env::var("AGENT_TRIGGER_BATCH_SIZE")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .map(|value| value.clamp(AGENT_TRIGGER_BATCH_SIZE_MIN, AGENT_TRIGGER_BATCH_SIZE_MAX))
        .unwrap_or(AGENT_TRIGGER_BATCH_SIZE_DEFAULT)
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AgentTriggerPreferences {
    pub batch_size: usize,
    pub environment_default: usize,
    pub managed: bool,
    pub updated_by_human_user_id: Option<Uuid>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct AgentTriggerPreferencesFile {
    batch_size: Option<usize>,
    updated_by_human_user_id: Option<Uuid>,
    updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct CodexDefaultConfigSummary {
    pub codex_home: Option<String>,
    pub config_path: Option<String>,
    pub config_exists: bool,
    pub auth_path: Option<String>,
    pub auth_exists: bool,
    pub credential_hint: Option<String>,
    pub openai_base_url: Option<String>,
    pub model_provider: Option<String>,
    pub model: Option<String>,
    pub reasoning_effort: Option<String>,
    pub sandbox_mode: Option<String>,
    pub approval_policy: Option<String>,
    pub mcp_servers: Vec<String>,
    pub named_profiles: Vec<String>,
    pub trusted_project_count: usize,
    pub plugin_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CodexDefaultAuthEnvironment {
    pub selector: String,
    pub name: String,
    pub status: String,
    pub method: Option<String>,
    pub last_checked_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    #[serde(default)]
    pub config: CodexDefaultConfigSummary,
}

impl Default for CodexDefaultAuthEnvironment {
    fn default() -> Self {
        Self {
            selector: "default".into(),
            name: "宿主机默认登录".into(),
            status: CODEX_DEFAULT_AUTH_STATUS_UNKNOWN.into(),
            method: None,
            last_checked_at: None,
            last_error: None,
            config: CodexDefaultConfigSummary::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexCliRuntime {
    pub installed: bool,
    pub source: String,
    pub executable_path: Option<String>,
    pub installed_version: Option<String>,
    pub latest_version: Option<String>,
    pub update_available: bool,
    pub operation_status: String,
    pub last_checked_at: Option<DateTime<Utc>>,
    pub update_check_error: Option<String>,
    pub last_error: Option<String>,
    #[serde(default = "current_host_os")]
    pub host_os: String,
    #[serde(default = "current_host_arch")]
    pub host_arch: String,
    #[serde(default = "current_installer_kind")]
    pub installer_kind: String,
    #[serde(default = "current_installation_supported")]
    pub installation_supported: bool,
    #[serde(default)]
    pub default_auth: CodexDefaultAuthEnvironment,
    pub updated_at: DateTime<Utc>,
}

impl Default for CodexCliRuntime {
    fn default() -> Self {
        Self {
            installed: false,
            source: "unavailable".into(),
            executable_path: None,
            installed_version: None,
            latest_version: None,
            update_available: false,
            operation_status: CODEX_CLI_OPERATION_IDLE.into(),
            last_checked_at: None,
            update_check_error: None,
            last_error: None,
            host_os: current_host_os(),
            host_arch: current_host_arch(),
            installer_kind: current_installer_kind(),
            installation_supported: current_installation_supported(),
            default_auth: CodexDefaultAuthEnvironment::default(),
            updated_at: now_utc(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CodexAuthProfile {
    pub id: Uuid,
    pub company_id: Uuid,
    pub name: String,
    pub selector: String,
    #[serde(default)]
    pub base_url: Option<String>,
    pub status: String,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CompanyCodexCliSettings {
    pub company_id: Uuid,
    pub model: Option<String>,
    pub reasoning_effort: Option<String>,
    pub reasoning_summary: String,
    pub verbosity: Option<String>,
    pub personality: Option<String>,
    pub service_tier: Option<String>,
    pub approval_policy: String,
    pub sandbox_mode: String,
    pub network_access: bool,
    pub web_search: String,
    pub feature_multi_agent: bool,
    pub feature_remote_plugin: bool,
    pub feature_hooks: bool,
    pub feature_goals: bool,
    pub feature_shell_tool: bool,
    pub updated_at: DateTime<Utc>,
}

impl CompanyCodexCliSettings {
    pub fn new(company_id: Uuid) -> Self {
        Self {
            company_id,
            model: None,
            reasoning_effort: None,
            reasoning_summary: CODEX_REASONING_SUMMARY_AUTO.into(),
            verbosity: None,
            personality: Some(CODEX_PERSONALITY_PRAGMATIC.into()),
            service_tier: None,
            approval_policy: CODEX_APPROVAL_POLICY_NEVER.into(),
            sandbox_mode: CODEX_SANDBOX_WORKSPACE_WRITE.into(),
            network_access: true,
            web_search: CODEX_WEB_SEARCH_CACHED.into(),
            feature_multi_agent: true,
            feature_remote_plugin: true,
            feature_hooks: true,
            feature_goals: true,
            feature_shell_tool: true,
            updated_at: now_utc(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct CompanyCodexCliSettingsFile {
    settings: Vec<CompanyCodexCliSettings>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct CodexMcpServerView {
    pub name: String,
    pub transport: String,
    pub enabled: bool,
    pub auth_status: Option<String>,
    pub address: Option<String>,
    pub command: Option<String>,
    pub argument_count: usize,
    pub bearer_token_env_var: Option<String>,
    pub startup_timeout_sec: Option<u64>,
    pub tool_timeout_sec: Option<u64>,
    pub disabled_reason: Option<String>,
    pub configured_by_user: bool,
    #[serde(default)]
    pub managed_by_relay: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CodexMcpEnvironmentSnapshot {
    pub selector: String,
    pub status: String,
    pub operation_status: String,
    pub pending_server_name: Option<String>,
    pub servers: Vec<CodexMcpServerView>,
    pub last_checked_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
}

impl CodexMcpEnvironmentSnapshot {
    fn new(selector: String) -> Self {
        Self {
            selector,
            status: "unknown".into(),
            operation_status: CODEX_MCP_OPERATION_IDLE.into(),
            pending_server_name: None,
            servers: Vec::new(),
            last_checked_at: None,
            last_error: None,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct CodexMcpCatalogFile {
    environments: Vec<CodexMcpEnvironmentSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CodexMcpServerInput {
    pub target_selector: String,
    pub name: String,
    pub transport: String,
    pub url: Option<String>,
    pub command: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    pub bearer_token_env_var: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct CodexAuthProfileFile {
    profiles: Vec<CodexAuthProfile>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CodexEnvironmentView {
    pub runtime: CodexCliRuntime,
    pub profiles: Vec<CodexAuthProfile>,
    pub mcp_environments: Vec<CodexMcpEnvironmentSnapshot>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CodexControlRequestKind {
    InstallCli,
    UpdateCli,
    ProvisionAuth,
    DeleteAuth,
    RefreshMcp,
    AddMcp,
    RemoveMcp,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct CodexControlRequest {
    pub id: Uuid,
    pub kind: CodexControlRequestKind,
    pub company_id: Option<Uuid>,
    pub profile_id: Option<Uuid>,
    pub api_key: Option<String>,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub target_selector: Option<String>,
    #[serde(default)]
    pub mcp_server: Option<CodexMcpServerInput>,
    #[serde(default)]
    pub mcp_server_name: Option<String>,
    pub created_at: DateTime<Utc>,
}

pub struct ClaimedCodexControlRequest {
    pub request: CodexControlRequest,
    path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct CodexControlStore {
    control_root: PathBuf,
    state_root: PathBuf,
}

fn current_host_os() -> String {
    std::env::consts::OS.to_string()
}

fn current_host_arch() -> String {
    std::env::consts::ARCH.to_string()
}

fn current_installer_kind() -> String {
    match std::env::consts::OS {
        "macos" | "linux" => CODEX_INSTALLER_POSIX_SHELL,
        "windows" => CODEX_INSTALLER_POWERSHELL,
        _ => CODEX_INSTALLER_UNSUPPORTED,
    }
    .to_string()
}

fn current_installation_supported() -> bool {
    matches!(std::env::consts::OS, "macos" | "linux" | "windows")
}

pub fn managed_profile_selector(profile_id: Uuid) -> String {
    format!("relay_{profile_id}")
}

pub fn managed_profile_id(selector: &str) -> Option<Uuid> {
    selector
        .strip_prefix("relay_")
        .and_then(|value| Uuid::parse_str(value).ok())
}

#[cfg(test)]
mod tests;
