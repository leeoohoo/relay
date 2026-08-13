use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use ai_chat_domain::agent_identity::AgentInboxEvent;
use ai_chat_domain::company::{
    AgentCodexRunToken, AgentCodexSession, AgentCodexTriggerConfig, AgentCodexTriggerRun,
    AgentExecutionIntent, AgentRuntimeProjection, CodexPluginCatalogSnapshot, CodexPluginOperation,
    CompanyCodexRunnerProfile, CompanyProject, CompanyProjectGitConfig, CompanyProjectTask,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyAgentCodexTriggerView {
    pub config: AgentCodexTriggerConfig,
    pub recent_runs: Vec<AgentCodexTriggerRun>,
    pub active_intents: Vec<AgentExecutionIntent>,
    pub recent_sessions: Vec<AgentCodexSession>,
    pub runner_profile_id: Option<Uuid>,
    pub runtime: AgentRuntimeProjection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyCodexRunnerProfileView {
    pub profile: CompanyCodexRunnerProfile,
    pub assigned_agent_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListCompanyCodexRunnerProfilesForHumanInput {
    pub human_user_id: Uuid,
    pub company_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpsertCompanyCodexRunnerProfileForHumanInput {
    pub human_user_id: Uuid,
    pub company_id: Uuid,
    pub profile_id: Option<Uuid>,
    pub name: String,
    pub interval_seconds: i32,
    pub codex_profile: String,
    pub model: Option<String>,
    pub reasoning_effort: Option<String>,
    pub reasoning_summary: Option<String>,
    pub verbosity: Option<String>,
    pub personality: Option<String>,
    pub service_tier: Option<String>,
    pub sandbox_mode: String,
    pub approval_policy: String,
    pub network_access: Option<bool>,
    pub web_search: Option<String>,
    pub feature_multi_agent: Option<bool>,
    pub feature_remote_plugin: Option<bool>,
    pub feature_hooks: Option<bool>,
    pub feature_goals: Option<bool>,
    pub feature_shell_tool: Option<bool>,
    pub max_run_seconds: i32,
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteCompanyCodexRunnerProfileForHumanInput {
    pub human_user_id: Uuid,
    pub company_id: Uuid,
    pub profile_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListCompanyCodexPluginsForHumanInput {
    pub human_user_id: Uuid,
    pub company_id: Uuid,
    pub operation_limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyCodexPluginsView {
    pub catalogs: Vec<CodexPluginCatalogSnapshot>,
    pub operations: Vec<CodexPluginOperation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestCodexPluginOperationForHumanInput {
    pub human_user_id: Uuid,
    pub company_id: Uuid,
    pub target_runner_id: String,
    pub target_selector: String,
    pub operation: String,
    pub plugin_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetCompanyAgentCodexTriggerForHumanInput {
    pub human_user_id: Uuid,
    pub company_id: Uuid,
    pub agent_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyAgentCodexRuntimeOverview {
    pub agent_id: Uuid,
    pub trigger: Option<CompanyAgentCodexTriggerView>,
    pub sessions: Vec<AgentCodexSession>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpsertCompanyAgentCodexTriggerForHumanInput {
    pub human_user_id: Uuid,
    pub company_id: Uuid,
    pub agent_id: Uuid,
    pub interval_seconds: Option<i32>,
    pub codex_profile: Option<String>,
    pub model: Option<String>,
    pub reasoning_effort: Option<String>,
    pub reasoning_summary: Option<String>,
    pub verbosity: Option<String>,
    pub personality: Option<String>,
    pub service_tier: Option<String>,
    pub sandbox_mode: Option<String>,
    pub approval_policy: Option<String>,
    pub network_access: Option<bool>,
    pub web_search: Option<String>,
    pub feature_multi_agent: Option<bool>,
    pub feature_remote_plugin: Option<bool>,
    pub feature_hooks: Option<bool>,
    pub feature_goals: Option<bool>,
    pub feature_shell_tool: Option<bool>,
    pub max_run_seconds: Option<i32>,
    pub runner_profile_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetCompanyAgentCodexTriggerStatusForHumanInput {
    pub human_user_id: Uuid,
    pub company_id: Uuid,
    pub agent_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListCompanyAgentCodexRunsForHumanInput {
    pub human_user_id: Uuid,
    pub company_id: Uuid,
    pub agent_id: Uuid,
    pub limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListCompanyAgentCodexSessionsForHumanInput {
    pub human_user_id: Uuid,
    pub company_id: Uuid,
    pub agent_id: Uuid,
    pub project_id: Option<Uuid>,
    pub limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCodexWorkDecision {
    pub should_run: bool,
    pub trigger_type: String,
    pub project: Option<CompanyProject>,
    pub git: Option<CompanyProjectGitConfig>,
    pub pending_inbox_count: usize,
    pub active_task_count: usize,
    pub waiting_task_count: usize,
    pub asset_refresh_due: bool,
    pub pending_execution_intent_count: usize,
    pub resume_existing_intents_directly: bool,
    pub control_snapshot: AgentControlSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentControlSnapshot {
    pub agent_profile_id: Uuid,
    pub company_id: Uuid,
    pub generated_at: DateTime<Utc>,
    pub snapshot_version: String,
    /// Pending message events visible to this Agent, including informational
    /// group messages that must not wake the Agent on their own.
    #[serde(default)]
    pub unread_messages: Vec<AgentInboxEvent>,
    pub actionable_events: Vec<AgentInboxEvent>,
    pub ready_tasks: Vec<CompanyProjectTask>,
    pub waiting_tasks: Vec<CompanyProjectTask>,
    pub active_intents: Vec<AgentExecutionIntent>,
    pub work_sessions: Vec<AgentCodexSession>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCodexRunTokenIssue {
    pub token: AgentCodexRunToken,
    pub plaintext_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompleteAgentCodexTriggerLeaseInput {
    pub trigger_config_id: Uuid,
    pub lease_owner: String,
    pub finished_at: DateTime<Utc>,
    pub next_run_at: DateTime<Utc>,
    pub succeeded: bool,
    pub error_message: Option<String>,
}
