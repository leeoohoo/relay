use std::{
    collections::{BTreeMap, HashMap},
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
    time::Duration,
};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader},
    process::Command,
    time::{sleep, timeout},
};
use uuid::Uuid;

use ai_chat_domain::company::{
    AGENT_CODEX_APPROVAL_TOOL_COMMAND, AGENT_CODEX_APPROVAL_TOOL_FILE_CHANGE,
    AGENT_CODEX_APPROVAL_TOOL_PERMISSIONS,
};
use ai_chat_shared::{AppError, AppResult};

use crate::codex_control::{
    managed_profile_id, CodexControlStore, CodexDefaultConfigSummary, CodexMcpServerInput,
    CodexMcpServerView, CODEX_MCP_TRANSPORT_HTTP, CODEX_MCP_TRANSPORT_STDIO,
};

const DEFAULT_RUN_TOKEN_ENV: &str = "RELAY_AGENT_RUN_TOKEN";
const DEFAULT_AUTO_COMPACT_TOKEN_LIMIT: u64 = 200_000;
const MAX_STDERR_BYTES: usize = 32 * 1024;
const MAX_CODEX_JSON_MESSAGE_BYTES: usize = 16 * 1024 * 1024;

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
    pub environment: HashMap<String, String>,
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

pub trait CodexCancellationHandler: Send + Sync {
    fn should_cancel(&self) -> bool;
}

async fn wait_for_cancellation(handler: Option<&Arc<dyn CodexCancellationHandler>>) {
    let Some(handler) = handler else {
        std::future::pending::<()>().await;
        return;
    };
    loop {
        sleep(Duration::from_millis(500)).await;
        if handler.should_cancel() {
            return;
        }
    }
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
mod configuration;
mod jsonl;
mod model_catalog;
mod runtime;
mod validation;

use app_server::*;
use jsonl::*;
use validation::*;

pub use model_catalog::collect_codex_models;

#[cfg(test)]
mod tests;
