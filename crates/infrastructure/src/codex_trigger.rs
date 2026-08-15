use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{Arc, Mutex},
    time::Duration,
};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader},
    process::Command,
    time::{sleep, timeout, Instant},
};
use uuid::Uuid;

use ai_chat_domain::company::{
    AGENT_CODEX_APPROVAL_TOOL_COMMAND, AGENT_CODEX_APPROVAL_TOOL_FILE_CHANGE,
    AGENT_CODEX_APPROVAL_TOOL_PERMISSIONS, AGENT_CODEX_APPROVAL_TOOL_WEBSITE_ACCESS,
};
use ai_chat_shared::{AppError, AppResult};

use crate::codex_control::{
    managed_profile_id, CodexControlStore, CodexDefaultConfigSummary, CodexMcpServerInput,
    CodexMcpServerView, CODEX_MCP_TRANSPORT_HTTP, CODEX_MCP_TRANSPORT_STDIO,
};

const DEFAULT_RUN_TOKEN_ENV: &str = "RELAY_AGENT_RUN_TOKEN";
const SESSION_KIND_ENV: &str = "RELAY_AGENT_SESSION_KIND";
const DEFAULT_AUTO_COMPACT_TOKEN_LIMIT: u64 = 200_000;
const MAX_STDERR_BYTES: usize = 32 * 1024;
const MAX_CODEX_JSON_MESSAGE_BYTES: usize = 16 * 1024 * 1024;
const RELAY_UNSUPPORTED_CODEX_PLUGIN_IDS: [&str; 10] = [
    "browser@openai-bundled",
    "chrome@openai-bundled",
    "computer-use@openai-bundled",
    "visualize@openai-bundled",
    "record-and-replay@openai-bundled",
    "documents@openai-primary-runtime",
    "pdf@openai-primary-runtime",
    "spreadsheets@openai-primary-runtime",
    "presentations@openai-primary-runtime",
    "template-creator@openai-primary-runtime",
];

pub fn is_relay_supported_codex_plugin_id(plugin_id: &str) -> bool {
    !plugin_id.ends_with("@openai-primary-runtime")
        && !RELAY_UNSUPPORTED_CODEX_PLUGIN_IDS.contains(&plugin_id)
}

pub fn filter_relay_supported_codex_plugin_items(items: &Value) -> Value {
    Value::Array(
        items
            .as_array()
            .into_iter()
            .flatten()
            .filter(|plugin| {
                plugin
                    .get("pluginId")
                    .and_then(Value::as_str)
                    .is_some_and(is_relay_supported_codex_plugin_id)
            })
            .cloned()
            .collect(),
    )
}

pub fn filter_relay_supported_codex_marketplaces(
    marketplaces: &Value,
    installed: &Value,
    available: &Value,
) -> Value {
    let supported_marketplaces = installed
        .as_array()
        .into_iter()
        .flatten()
        .chain(available.as_array().into_iter().flatten())
        .filter_map(|plugin| plugin.get("marketplaceName").and_then(Value::as_str))
        .collect::<std::collections::HashSet<_>>();
    Value::Array(
        marketplaces
            .as_array()
            .into_iter()
            .flatten()
            .filter(|marketplace| {
                marketplace
                    .get("name")
                    .and_then(Value::as_str)
                    .is_some_and(|name| supported_marketplaces.contains(name))
            })
            .cloned()
            .collect(),
    )
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CodexModelReasoningEffort {
    pub effort: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CodexModelInfo {
    pub id: String,
    pub display_name: String,
    pub default_reasoning_effort: Option<String>,
    pub reasoning_efforts: Vec<CodexModelReasoningEffort>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexModelCatalogSnapshot {
    pub codex_profile: String,
    pub bundled: bool,
    pub models: Vec<CodexModelInfo>,
    pub source: String,
    pub discovered_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CodexModelCatalogFile {
    pub catalogs: Vec<CodexModelCatalogSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexPluginCatalogDiscovery {
    pub installed: Value,
    pub available: Value,
    pub marketplaces: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexDefaultAuthProbe {
    pub status: String,
    pub method: Option<String>,
    pub config: CodexDefaultConfigSummary,
}

#[derive(Debug, Clone)]
pub struct CodexTriggerRunner {
    executable: PathBuf,
    executable_source: String,
    prefix_args: Vec<String>,
    mcp_url: String,
    mcp_server_name: String,
    run_token_env_name: String,
    inherited_environment: HashMap<String, String>,
    shell_excluded_environment_names: Vec<String>,
    auto_compact_token_limit: u64,
    managed_profile_homes_root: PathBuf,
    managed_cli_home: PathBuf,
    runtime_temp_root: PathBuf,
    browser_mcp: BrowserMcpConfig,
}

struct ManagedRuntimeTempDirectory {
    path: PathBuf,
}

impl Drop for ManagedRuntimeTempDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedCodexMcpServer {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub url: Option<String>,
    pub env_http_headers: BTreeMap<String, String>,
    pub disabled_plugin_ids: Vec<String>,
    pub required: bool,
    pub startup_timeout_sec: Option<u64>,
    pub tool_timeout_sec: Option<u64>,
    pub default_tools_approval_mode: String,
    pub tool_approval_modes: BTreeMap<String, String>,
    pub prompt_hint: Option<String>,
}

impl ManagedCodexMcpServer {
    pub fn requires_human_approval(&self) -> bool {
        self.default_tools_approval_mode == "prompt"
            || self
                .tool_approval_modes
                .values()
                .any(|mode| mode == "prompt")
    }
}

#[derive(Clone)]
pub struct CodexRunRequest {
    pub cwd: PathBuf,
    pub codex_profile: String,
    pub model: Option<String>,
    pub reasoning_effort: Option<String>,
    pub reasoning_summary: Option<String>,
    pub verbosity: Option<String>,
    pub personality: Option<String>,
    pub service_tier: Option<String>,
    pub sandbox_mode: String,
    pub approval_policy: String,
    pub network_access: bool,
    pub web_search: String,
    pub feature_multi_agent: bool,
    pub feature_remote_plugin: bool,
    pub feature_hooks: bool,
    pub feature_goals: bool,
    pub feature_shell_tool: bool,
    pub max_run_seconds: u64,
    pub prompt: String,
    pub existing_thread_id: Option<String>,
    pub run_token: String,
    pub session_kind: String,
    pub environment: HashMap<String, String>,
    pub managed_mcp_servers: Vec<ManagedCodexMcpServer>,
    pub approval_handler: Option<Arc<dyn CodexApprovalHandler>>,
    pub progress_handler: Option<Arc<dyn CodexProgressHandler>>,
    pub cancellation_handler: Option<Arc<dyn CodexCancellationHandler>>,
}

#[derive(Debug, Clone)]
pub struct CodexProgressEvent {
    pub phase: String,
    pub summary: String,
    pub thread_id: Option<String>,
}

pub trait CodexProgressHandler: Send + Sync {
    fn report(&self, event: CodexProgressEvent);
}

#[async_trait]
pub trait CodexCancellationHandler: Send + Sync {
    fn should_cancel(&self) -> bool;

    async fn wait_for_cancellation(&self) {
        loop {
            sleep(Duration::from_millis(500)).await;
            if self.should_cancel() {
                return;
            }
        }
    }

    fn cancellation_reason(&self) -> String {
        "Codex run cancelled because the project was paused".into()
    }
}

async fn wait_for_cancellation(handler: Option<&Arc<dyn CodexCancellationHandler>>) {
    let Some(handler) = handler else {
        std::future::pending::<()>().await;
        return;
    };
    handler.wait_for_cancellation().await;
}

#[derive(Debug, Clone)]
pub struct CodexApprovalRequest {
    pub tool_name: String,
    pub risk_level: String,
    pub reason: String,
    pub arguments: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodexApprovalDecision {
    Accept,
    Decline,
}

#[async_trait]
pub trait CodexApprovalHandler: Send + Sync {
    async fn request_approval(
        &self,
        request: CodexApprovalRequest,
    ) -> AppResult<CodexApprovalDecision>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodexRunStatus {
    Succeeded,
    Failed,
    TimedOut,
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct CodexRunResult {
    pub status: CodexRunStatus,
    pub thread_id: Option<String>,
    pub exit_code: Option<i32>,
    pub final_message: Option<String>,
    pub error_message: Option<String>,
    pub resumed_existing_session: bool,
    pub replaced_failed_session: bool,
}

#[derive(Debug)]
struct ProcessOutcome {
    status: CodexRunStatus,
    thread_id: Option<String>,
    exit_code: Option<i32>,
    final_message: Option<String>,
    error_message: Option<String>,
    turn_started: bool,
}

#[derive(Debug, Default)]
struct JsonlEvents {
    thread_id: Option<String>,
    turn_started: bool,
    turn_completed: bool,
    turn_failed: bool,
    final_message: Option<String>,
    error_message: Option<String>,
}

mod app_server;
mod browser;
mod configuration;
mod jsonl;
mod managed_run_profile;
mod model_catalog;
mod runtime;
mod validation;

use app_server::*;
use browser::*;
use jsonl::*;
use validation::*;

pub use model_catalog::collect_codex_models;

#[cfg(test)]
mod app_server_tests;
#[cfg(test)]
mod browser_tests;
#[cfg(test)]
mod configuration_tests;
#[cfg(test)]
mod plugin_tests;
#[cfg(test)]
mod tests;
