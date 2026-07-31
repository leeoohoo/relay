use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

pub const COMPANY_ROLE_OWNER: &str = "owner";
pub const COMPANY_ROLE_ADMIN: &str = "admin";
pub const COMPANY_ROLE_VIEWER: &str = "viewer";

pub const COMPANY_AGENT_ROLE_MANAGER: &str = "company_manager";
pub const COMPANY_AGENT_ROLE_MEMBER: &str = "member";

pub const COMPANY_PROFESSION_PROJECT_MANAGER: &str = "project_manager";
pub const COMPANY_PROFESSION_PRODUCT_MANAGER: &str = "product_manager";
pub const COMPANY_PROFESSION_TECHNICAL_MANAGER: &str = "technical_manager";
pub const COMPANY_PROFESSION_SOLUTION_ARCHITECT: &str = "solution_architect";
pub const COMPANY_PROFESSION_SOFTWARE_ENGINEER: &str = "software_engineer";
pub const COMPANY_PROFESSION_FRONTEND_ENGINEER: &str = "frontend_engineer";
pub const COMPANY_PROFESSION_BACKEND_ENGINEER: &str = "backend_engineer";
pub const COMPANY_PROFESSION_MOBILE_ENGINEER: &str = "mobile_engineer";
pub const COMPANY_PROFESSION_DATA_ENGINEER: &str = "data_engineer";
pub const COMPANY_PROFESSION_DEVOPS_ENGINEER: &str = "devops_engineer";
pub const COMPANY_PROFESSION_QA_ENGINEER: &str = "qa_engineer";
pub const COMPANY_PROFESSION_PRODUCT_DESIGNER: &str = "product_designer";
pub const COMPANY_PROFESSION_UI_DESIGNER: &str = "ui_designer";
pub const COMPANY_PROFESSION_UX_DESIGNER: &str = "ux_designer";
pub const COMPANY_PROFESSION_BUSINESS_ANALYST: &str = "business_analyst";
pub const COMPANY_PROFESSION_IMPLEMENTATION_CONSULTANT: &str = "implementation_consultant";
pub const COMPANY_PROFESSION_DOMAIN_EXPERT: &str = "domain_expert";
pub const COMPANY_PROFESSION_OPERATIONS_SPECIALIST: &str = "operations_specialist";
pub const COMPANY_PROFESSION_GENERAL_MEMBER: &str = "general_member";

pub const COMPANY_PERMISSION_READ: &str = "company.read";
pub const COMPANY_PERMISSION_ORG_READ: &str = "org.read";
pub const COMPANY_PERMISSION_AGENT_DIRECTORY_READ: &str = "agent.directory.read";
pub const COMPANY_PERMISSION_AGENT_COMMUNICATE: &str = "agent.communicate";
pub const COMPANY_PERMISSION_PROJECT_CREATE: &str = "project.create";
pub const COMPANY_PERMISSION_PROJECT_MANAGE: &str = "project.manage";
pub const COMPANY_PERMISSION_PROJECT_RULES_MANAGE: &str = "project.rules.manage";
pub const COMPANY_PERMISSION_PROJECT_ASSETS_MANAGE: &str = "project.assets.manage";
pub const COMPANY_PERMISSION_TASK_ASSIGN: &str = "task.assign";
pub const COMPANY_PERMISSION_TASK_UPDATE: &str = "task.update";
pub const COMPANY_PERMISSION_MESSAGE_SEND: &str = "message.send";
pub const COMPANY_PERMISSION_STAFF_HIRE: &str = "agent.staff.hire";
pub const COMPANY_PERMISSION_STAFF_SUSPEND: &str = "agent.staff.suspend";
pub const COMPANY_PERMISSION_STAFF_TERMINATE: &str = "agent.staff.terminate";

pub const STAFFING_ACTION_PERMISSION_UPDATE: &str = "permission_update";
pub const STAFFING_ACTION_ROLE_UPDATE: &str = "role_update";
pub const STAFFING_ACTION_PROFESSION_UPDATE: &str = "profession_update";
pub const STAFFING_ACTION_HIRE: &str = "hire";
pub const STAFFING_ACTION_ACTIVATE: &str = "activate";
pub const STAFFING_ACTION_SUSPEND: &str = "suspend";
pub const STAFFING_ACTION_REACTIVATE: &str = "reactivate";
pub const STAFFING_ACTION_TERMINATE: &str = "terminate";

pub const STAFFING_ACTOR_HUMAN: &str = "human";
pub const STAFFING_ACTOR_AGENT: &str = "agent";
pub const STAFFING_ACTOR_SYSTEM: &str = "system";

pub const STAFFING_STATUS_PENDING_APPROVAL: &str = "pending_approval";
pub const STAFFING_STATUS_COMPLETED: &str = "completed";
pub const STAFFING_STATUS_REJECTED: &str = "rejected";
pub const STAFFING_STATUS_FAILED: &str = "failed";

pub const PROJECT_STATUS_PLANNED: &str = "planned";
pub const PROJECT_STATUS_ACTIVE: &str = "active";
pub const PROJECT_STATUS_BLOCKED: &str = "blocked";
pub const PROJECT_STATUS_COMPLETED: &str = "completed";
pub const PROJECT_STATUS_CANCELLED: &str = "cancelled";

pub const PROJECT_MEMBER_ROLE_OWNER: &str = "owner";
pub const PROJECT_MEMBER_ROLE_MEMBER: &str = "member";

pub const PROJECT_TASK_STATUS_TODO: &str = "todo";
pub const PROJECT_TASK_STATUS_IN_PROGRESS: &str = "in_progress";
pub const PROJECT_TASK_STATUS_BLOCKED: &str = "blocked";
pub const PROJECT_TASK_STATUS_DONE: &str = "done";
pub const PROJECT_TASK_STATUS_FAILED: &str = "failed";
pub const PROJECT_TASK_STATUS_CANCELLED: &str = "cancelled";

pub const PROJECT_TASK_PRIORITY_LOW: &str = "low";
pub const PROJECT_TASK_PRIORITY_NORMAL: &str = "normal";
pub const PROJECT_TASK_PRIORITY_HIGH: &str = "high";
pub const PROJECT_TASK_PRIORITY_URGENT: &str = "urgent";

pub const AGENT_RUNTIME_EXECUTOR_RULES_V1: &str = "rules_v1";
pub const AGENT_RUNTIME_EXECUTOR_MODEL_V1: &str = "model_v1";
pub const AGENT_RUNTIME_MODEL_PROVIDER_OPENAI_RESPONSES: &str = "openai_responses";

pub const AGENT_RUNTIME_MODEL_ACTION_MESSAGE_SEND: &str = "company.chat.message.send";
pub const AGENT_RUNTIME_MODEL_ACTION_TASK_START_ASSIGNED: &str =
    "company.project.task.start_assigned";
pub const AGENT_RUNTIME_MODEL_ACTION_PROJECT_STATUS_UPDATE: &str = "company.project.status.update";
pub const AGENT_RUNTIME_APPROVAL_ACTION_STAFF_HIRE: &str = "agent.staff.hire";
pub const AGENT_RUNTIME_APPROVAL_ACTION_STAFF_SUSPEND: &str = "agent.staff.suspend";
pub const AGENT_RUNTIME_APPROVAL_ACTION_STAFF_TERMINATE: &str = "agent.staff.terminate";
pub const AGENT_RUNTIME_APPROVAL_ACTION_TASK_REASSIGN: &str = "company.project.task.reassign";

pub const AGENT_TOOL_APPROVAL_STATUS_PENDING: &str = "pending";
pub const AGENT_TOOL_APPROVAL_STATUS_APPROVED: &str = "approved";
pub const AGENT_TOOL_APPROVAL_STATUS_EXECUTING: &str = "executing";
pub const AGENT_TOOL_APPROVAL_STATUS_EXECUTED: &str = "executed";
pub const AGENT_TOOL_APPROVAL_STATUS_REJECTED: &str = "rejected";
pub const AGENT_TOOL_APPROVAL_STATUS_EXPIRED: &str = "expired";
pub const AGENT_TOOL_APPROVAL_STATUS_FAILED: &str = "failed";
pub const AGENT_RUNTIME_TEMPLATE_STATUS_ACTIVE: &str = "active";
pub const AGENT_RUNTIME_TEMPLATE_STATUS_ARCHIVED: &str = "archived";
pub const AGENT_MODEL_PRICE_CATALOG_STATUS_ACTIVE: &str = "active";
pub const AGENT_MODEL_PRICE_CATALOG_STATUS_ARCHIVED: &str = "archived";
pub const COMPANY_GOVERNANCE_POLICY_STATUS_ACTIVE: &str = "active";
pub const COMPANY_GOVERNANCE_POLICY_STATUS_ARCHIVED: &str = "archived";
pub const AGENT_RUNTIME_STATUS_ACTIVE: &str = "active";
pub const AGENT_RUNTIME_STATUS_PAUSED: &str = "paused";
pub const AGENT_RUNTIME_STATUS_ERROR: &str = "error";

pub const AGENT_RUNTIME_MESSAGE_POLICY_OBSERVE: &str = "observe";
pub const AGENT_RUNTIME_MESSAGE_POLICY_DIRECT_ACK: &str = "direct_ack";
pub const AGENT_RUNTIME_MESSAGE_POLICY_MENTIONED_ACK: &str = "mentioned_ack";

pub const AGENT_RUNTIME_TRIGGER_SCHEDULED: &str = "scheduled";
pub const AGENT_RUNTIME_TRIGGER_MANUAL: &str = "manual";

pub const AGENT_RUNTIME_RUN_STATUS_RUNNING: &str = "running";
pub const AGENT_RUNTIME_RUN_STATUS_SUCCEEDED: &str = "succeeded";
pub const AGENT_RUNTIME_RUN_STATUS_FAILED: &str = "failed";
pub const AGENT_RUNTIME_RUN_STATUS_SKIPPED_BUDGET: &str = "skipped_budget";
pub const AGENT_RUNTIME_MODEL_PRICING_STATUS_PRICED: &str = "priced";
pub const AGENT_RUNTIME_MODEL_PRICING_STATUS_UNPRICED: &str = "unpriced";
pub const DEFAULT_COMPANY_DAILY_MODEL_COST_BUDGET_MICROUSD: i64 = 100_000_000;

pub const AGENT_CODEX_TRIGGER_STATUS_ACTIVE: &str = "active";
pub const AGENT_CODEX_TRIGGER_STATUS_PAUSED: &str = "paused";
pub const AGENT_CODEX_TRIGGER_STATUS_ERROR: &str = "error";
pub const AGENT_CODEX_SANDBOX_READ_ONLY: &str = "read_only";
pub const AGENT_CODEX_SANDBOX_WORKSPACE_WRITE: &str = "workspace_write";
pub const AGENT_CODEX_APPROVAL_POLICY_NEVER: &str = "never";
pub const AGENT_CODEX_APPROVAL_POLICY_ON_REQUEST: &str = "on-request";
pub const AGENT_TOOL_APPROVAL_SOURCE_RUNTIME_MODEL: &str = "runtime_model";
pub const AGENT_TOOL_APPROVAL_SOURCE_CODEX: &str = "codex";
pub const AGENT_CODEX_APPROVAL_TOOL_COMMAND: &str = "codex.command_execution";
pub const AGENT_CODEX_APPROVAL_TOOL_FILE_CHANGE: &str = "codex.file_change";
pub const AGENT_CODEX_APPROVAL_TOOL_PERMISSIONS: &str = "codex.permissions";
pub const AGENT_CODEX_TRIGGER_TYPE_SCHEDULED: &str = "scheduled";
pub const AGENT_CODEX_TRIGGER_TYPE_MANUAL: &str = "manual";
pub const AGENT_CODEX_TRIGGER_TYPE_MESSAGE: &str = "message";
pub const AGENT_CODEX_TRIGGER_TYPE_TASK: &str = "task";
pub const AGENT_CODEX_TRIGGER_TYPE_ASSET_REFRESH: &str = "asset_refresh";
pub const AGENT_CODEX_RUN_STATUS_RUNNING: &str = "running";
pub const AGENT_CODEX_RUN_STATUS_SUCCEEDED: &str = "succeeded";
pub const AGENT_CODEX_RUN_STATUS_FAILED: &str = "failed";
pub const AGENT_CODEX_RUN_STATUS_TIMED_OUT: &str = "timed_out";
pub const AGENT_CODEX_RUN_STATUS_CANCELLED: &str = "cancelled";
pub const AGENT_CODEX_RUN_STATUS_LEASE_LOST: &str = "lease_lost";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Company {
    pub id: Uuid,
    pub owner_user_id: Uuid,
    pub name: String,
    pub slug: String,
    pub description: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyHumanMember {
    pub id: Uuid,
    pub company_id: Uuid,
    pub human_user_id: Uuid,
    pub role: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrgUnit {
    pub id: Uuid,
    pub company_id: Uuid,
    pub parent_org_unit_id: Option<Uuid>,
    pub name: String,
    pub unit_type: String,
    pub sort_order: i32,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyAgentMembership {
    pub id: Uuid,
    pub company_id: Uuid,
    pub agent_profile_id: Uuid,
    pub org_unit_id: Uuid,
    pub job_title: String,
    pub role_key: String,
    pub reports_to_membership_id: Option<Uuid>,
    pub permissions: Vec<String>,
    pub responsibilities: Vec<String>,
    pub skills: Vec<String>,
    pub current_focus: String,
    pub staffing_scope_org_unit_id: Option<Uuid>,
    pub employment_status: String,
    pub joined_at: DateTime<Utc>,
    pub terminated_at: Option<DateTime<Utc>>,
    pub created_by_human_user_id: Option<Uuid>,
    pub created_by_agent_id: Option<Uuid>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CompanyProfession {
    pub key: String,
    pub label: String,
    pub description: String,
    pub skill_name: String,
    pub can_create_tasks: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStaffingAction {
    pub id: Uuid,
    pub company_id: Uuid,
    pub action_type: String,
    pub actor_type: String,
    pub actor_human_user_id: Option<Uuid>,
    pub actor_agent_id: Option<Uuid>,
    pub target_agent_id: Option<Uuid>,
    pub requested_org_unit_id: Option<Uuid>,
    pub requested_role_key: Option<String>,
    pub reason: String,
    pub handoff_plan: String,
    pub status: String,
    pub approval_required: bool,
    pub approved_by_human_user_id: Option<Uuid>,
    pub request_payload: Value,
    pub result_payload: Value,
    pub idempotency_key: Option<String>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyProject {
    pub id: Uuid,
    pub company_id: Uuid,
    pub name: String,
    pub description: String,
    pub status: String,
    pub owner_agent_id: Uuid,
    pub project_group_conversation_id: Uuid,
    pub created_by_agent_id: Uuid,
    pub updated_by_agent_id: Option<Uuid>,
    pub due_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyProjectGitConfig {
    pub project_id: Uuid,
    pub remote_url: String,
    pub default_branch: String,
    pub git_host: String,
    pub host_local_path: String,
    pub auth_profile: Option<String>,
    pub allow_agent_push: bool,
    pub branch_prefix: String,
    pub created_by_agent_id: Option<Uuid>,
    pub created_by_human_user_id: Option<Uuid>,
    pub updated_by_agent_id: Option<Uuid>,
    pub updated_by_human_user_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyProjectRule {
    pub project_id: Uuid,
    pub content: String,
    pub updated_by_agent_id: Option<Uuid>,
    pub updated_by_human_user_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyProjectAsset {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub asset_type: String,
    pub locator: String,
    pub description: String,
    pub status: String,
    pub metadata: Value,
    pub updated_by_agent_id: Option<Uuid>,
    pub updated_by_human_user_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyProjectAssetRefreshConfig {
    pub project_id: Uuid,
    pub maintainer_agent_id: Uuid,
    pub interval_minutes: i32,
    pub enabled: bool,
    pub next_refresh_at: DateTime<Utc>,
    pub last_requested_at: Option<DateTime<Utc>>,
    pub last_completed_at: Option<DateTime<Utc>>,
    pub created_by_human_user_id: Uuid,
    pub updated_by_human_user_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyCodexRunnerProfile {
    pub id: Uuid,
    pub company_id: Uuid,
    pub name: String,
    pub interval_seconds: i32,
    pub codex_profile: String,
    pub model: Option<String>,
    pub reasoning_effort: Option<String>,
    pub sandbox_mode: String,
    pub approval_policy: String,
    pub max_run_seconds: i32,
    pub is_default: bool,
    pub created_by_human_user_id: Uuid,
    pub updated_by_human_user_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCodexTriggerConfig {
    pub id: Uuid,
    pub company_id: Uuid,
    pub agent_profile_id: Uuid,
    pub status: String,
    pub interval_seconds: i32,
    pub codex_profile: String,
    pub model: Option<String>,
    pub reasoning_effort: Option<String>,
    pub sandbox_mode: String,
    pub approval_policy: String,
    pub max_run_seconds: i32,
    pub next_run_at: DateTime<Utc>,
    pub lease_owner: Option<String>,
    pub lease_expires_at: Option<DateTime<Utc>>,
    pub manual_run_requested_at: Option<DateTime<Utc>>,
    pub wake_requested_at: Option<DateTime<Utc>>,
    pub wake_reason: Option<String>,
    pub last_run_at: Option<DateTime<Utc>>,
    pub last_success_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub consecutive_failure_count: i32,
    pub created_by_human_user_id: Uuid,
    pub updated_by_human_user_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCodexRunActivity {
    pub at: DateTime<Utc>,
    pub phase: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCodexTriggerRun {
    pub id: Uuid,
    pub trigger_config_id: Uuid,
    pub agent_profile_id: Uuid,
    pub project_id: Option<Uuid>,
    pub trigger_type: String,
    pub status: String,
    pub codex_thread_id: Option<String>,
    pub codex_version: Option<String>,
    pub exit_code: Option<i32>,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub final_message_summary: Option<String>,
    pub error_message: Option<String>,
    pub activity_phase: String,
    pub activity_summary: Option<String>,
    pub last_activity_at: Option<DateTime<Utc>>,
    pub activity_log: Vec<AgentCodexRunActivity>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCodexSession {
    pub agent_profile_id: Uuid,
    pub current_project_id: Option<Uuid>,
    pub codex_thread_id: String,
    pub worktree_key: String,
    pub last_used_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCodexRunToken {
    pub id: Uuid,
    pub run_id: Uuid,
    pub agent_profile_id: Uuid,
    pub token_hash: String,
    pub expires_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyProjectMember {
    pub id: Uuid,
    pub project_id: Uuid,
    pub agent_profile_id: Uuid,
    pub role: String,
    pub joined_at: DateTime<Utc>,
    pub left_at: Option<DateTime<Utc>>,
    pub added_by_agent_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyProjectTask {
    pub id: Uuid,
    pub project_id: Uuid,
    pub title: String,
    pub description: String,
    pub status: String,
    pub priority: String,
    pub assignee_agent_id: Option<Uuid>,
    pub created_by_agent_id: Option<Uuid>,
    pub created_by_human_user_id: Option<Uuid>,
    pub updated_by_agent_id: Option<Uuid>,
    pub updated_by_human_user_id: Option<Uuid>,
    pub due_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyProjectTaskDependency {
    pub id: Uuid,
    pub project_id: Uuid,
    pub task_id: Uuid,
    pub depends_on_task_id: Uuid,
    pub created_by_agent_id: Option<Uuid>,
    pub created_by_human_user_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyProjectTaskStatusHistory {
    pub id: Uuid,
    pub project_id: Uuid,
    pub task_id: Uuid,
    pub from_status: Option<String>,
    pub to_status: String,
    pub changed_by_agent_id: Option<Uuid>,
    pub changed_by_human_user_id: Option<Uuid>,
    pub change_source: String,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyProjectStatusUpdate {
    pub id: Uuid,
    pub project_id: Uuid,
    pub author_agent_id: Uuid,
    pub summary: String,
    pub progress_percent: i16,
    pub blockers: Vec<String>,
    pub next_steps: Vec<String>,
    pub project_status: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyRealtimeEvent {
    pub sequence_id: i64,
    pub id: Uuid,
    pub company_id: Uuid,
    pub event_type: String,
    pub aggregate_type: String,
    pub aggregate_id: Option<Uuid>,
    pub actor_agent_id: Option<Uuid>,
    pub actor_human_user_id: Option<Uuid>,
    pub payload: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyRealtimeSignal {
    pub sequence_id: i64,
    pub company_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRuntimeConfig {
    pub id: Uuid,
    pub company_id: Uuid,
    pub agent_profile_id: Uuid,
    pub runtime_template_id: Option<Uuid>,
    pub model_price_catalog_entry_id: Option<Uuid>,
    pub executor_kind: String,
    pub status: String,
    pub interval_seconds: i32,
    pub max_events: i32,
    pub daily_run_budget: i32,
    pub daily_action_budget: i32,
    pub daily_model_input_token_budget: i64,
    pub daily_model_output_token_budget: i64,
    pub daily_model_cost_budget_microusd: i64,
    pub model_input_price_microusd_per_million_tokens: i64,
    pub model_output_price_microusd_per_million_tokens: i64,
    pub daily_approval_request_budget: i32,
    pub company_message_policy: String,
    pub auto_start_assigned_tasks: bool,
    pub auto_announce_project_membership: bool,
    pub context_max_projects: i32,
    pub context_max_conversations: i32,
    pub context_max_inbox_events: i32,
    pub system_prompt: String,
    pub model_name: Option<String>,
    pub model_provider: String,
    pub provider_secret_ref: Option<String>,
    pub allowed_model_actions: Vec<String>,
    pub approval_required_model_actions: Vec<String>,
    pub approval_request_ttl_minutes: i32,
    pub model_max_output_tokens: i32,
    pub model_timeout_seconds: i32,
    pub model_max_retries: i32,
    pub fallback_to_rules_v1: bool,
    pub next_run_at: DateTime<Utc>,
    pub last_run_at: Option<DateTime<Utc>>,
    pub last_success_at: Option<DateTime<Utc>>,
    pub last_error_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub created_by_human_user_id: Uuid,
    pub updated_by_human_user_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRuntimeTemplateSettings {
    #[serde(default)]
    pub model_price_catalog_entry_id: Option<Uuid>,
    pub executor_kind: String,
    pub interval_seconds: i32,
    pub max_events: i32,
    pub daily_run_budget: i32,
    pub daily_action_budget: i32,
    pub daily_model_input_token_budget: i64,
    pub daily_model_output_token_budget: i64,
    pub daily_model_cost_budget_microusd: i64,
    pub model_input_price_microusd_per_million_tokens: i64,
    pub model_output_price_microusd_per_million_tokens: i64,
    pub daily_approval_request_budget: i32,
    pub company_message_policy: String,
    pub auto_start_assigned_tasks: bool,
    pub auto_announce_project_membership: bool,
    pub context_max_projects: i32,
    pub context_max_conversations: i32,
    pub context_max_inbox_events: i32,
    pub system_prompt: String,
    pub model_name: Option<String>,
    pub model_provider: String,
    pub provider_secret_ref: Option<String>,
    pub allowed_model_actions: Vec<String>,
    pub approval_required_model_actions: Vec<String>,
    pub approval_request_ttl_minutes: i32,
    pub model_max_output_tokens: i32,
    pub model_timeout_seconds: i32,
    pub model_max_retries: i32,
    pub fallback_to_rules_v1: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRuntimeTemplate {
    pub id: Uuid,
    pub company_id: Uuid,
    pub name: String,
    pub description: String,
    pub status: String,
    pub settings: AgentRuntimeTemplateSettings,
    pub source_agent_profile_id: Option<Uuid>,
    pub created_by_human_user_id: Uuid,
    pub updated_by_human_user_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyRuntimePolicy {
    pub company_id: Uuid,
    pub default_runtime_template_id: Option<Uuid>,
    pub auto_apply_to_new_agents: bool,
    pub configured: bool,
    pub created_by_human_user_id: Option<Uuid>,
    pub updated_by_human_user_id: Option<Uuid>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentModelPriceCatalogEntry {
    pub id: Uuid,
    pub company_id: Uuid,
    pub model_provider: String,
    pub model_name: String,
    pub version: i32,
    pub status: String,
    pub input_price_microusd_per_million_tokens: i64,
    pub output_price_microusd_per_million_tokens: i64,
    pub notes: String,
    pub source_agent_profile_id: Option<Uuid>,
    pub created_by_human_user_id: Uuid,
    pub updated_by_human_user_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompanyGovernancePolicySettings {
    pub agent_staff_limit: i32,
    pub delegated_agent_hiring_enabled: bool,
    pub delegated_agent_suspension_enabled: bool,
    pub delegated_agent_termination_enabled: bool,
    pub max_active_projects: i32,
    pub max_project_members: i32,
    #[serde(default = "default_daily_delegated_hire_limit")]
    pub daily_delegated_hire_limit: i32,
    #[serde(default = "default_daily_delegated_suspension_limit")]
    pub daily_delegated_suspension_limit: i32,
    #[serde(default = "default_daily_delegated_termination_limit")]
    pub daily_delegated_termination_limit: i32,
}

fn default_daily_delegated_hire_limit() -> i32 {
    20
}

fn default_daily_delegated_suspension_limit() -> i32 {
    50
}

fn default_daily_delegated_termination_limit() -> i32 {
    20
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyGovernancePolicyVersion {
    pub id: Uuid,
    pub company_id: Uuid,
    pub version: i32,
    pub status: String,
    pub settings: CompanyGovernancePolicySettings,
    pub notes: String,
    pub created_by_human_user_id: Uuid,
    pub updated_by_human_user_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRuntimeRun {
    pub id: Uuid,
    pub runtime_config_id: Uuid,
    pub company_id: Uuid,
    pub agent_profile_id: Uuid,
    pub trigger_type: String,
    pub executor_kind: String,
    pub status: String,
    pub input_event_count: i32,
    pub processed_event_count: i32,
    pub action_count: i32,
    pub approval_request_count: i32,
    pub model_request_count: i32,
    pub model_input_tokens: i64,
    pub model_output_tokens: i64,
    pub model_cost_microusd: i64,
    pub model_input_price_microusd_per_million_tokens: i64,
    pub model_output_price_microusd_per_million_tokens: i64,
    pub model_pricing_status: String,
    pub remaining_pending_count: i32,
    pub input_payload: Value,
    pub output_payload: Value,
    pub error_message: Option<String>,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRuntimeDailyUsage {
    pub window_start: DateTime<Utc>,
    pub run_count: i64,
    pub action_count: i64,
    pub approval_request_count: i64,
    pub failed_run_count: i64,
    pub model_request_count: i64,
    pub model_input_tokens: i64,
    pub model_output_tokens: i64,
    pub model_cost_microusd: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyModelBudgetPolicy {
    pub company_id: Uuid,
    pub daily_model_cost_budget_microusd: i64,
    pub configured: bool,
    pub created_by_human_user_id: Option<Uuid>,
    pub updated_by_human_user_id: Option<Uuid>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyModelDailyUsage {
    pub window_start: DateTime<Utc>,
    pub model_request_count: i64,
    pub model_input_tokens: i64,
    pub model_output_tokens: i64,
    pub model_cost_microusd: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentToolApprovalRequest {
    pub id: Uuid,
    pub company_id: Uuid,
    pub approval_source: String,
    pub runtime_config_id: Option<Uuid>,
    pub runtime_run_id: Option<Uuid>,
    pub codex_trigger_run_id: Option<Uuid>,
    pub requested_by_agent_id: Uuid,
    pub tool_name: String,
    pub risk_level: String,
    pub reason: String,
    pub arguments: Value,
    pub status: String,
    pub expires_at: DateTime<Utc>,
    pub reviewed_by_human_user_id: Option<Uuid>,
    pub review_note: String,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub execution_result: Value,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub fn default_company_agent_permissions(role_key: &str) -> Vec<String> {
    let mut permissions = vec![
        COMPANY_PERMISSION_READ.to_string(),
        COMPANY_PERMISSION_ORG_READ.to_string(),
        COMPANY_PERMISSION_AGENT_DIRECTORY_READ.to_string(),
        COMPANY_PERMISSION_AGENT_COMMUNICATE.to_string(),
        COMPANY_PERMISSION_TASK_UPDATE.to_string(),
        COMPANY_PERMISSION_MESSAGE_SEND.to_string(),
    ];
    if role_key == COMPANY_AGENT_ROLE_MANAGER {
        permissions.extend([
            COMPANY_PERMISSION_PROJECT_CREATE.to_string(),
            COMPANY_PERMISSION_PROJECT_MANAGE.to_string(),
        ]);
    }
    permissions
}

pub fn company_profession_catalog() -> Vec<CompanyProfession> {
    vec![
        CompanyProfession {
            key: COMPANY_PROFESSION_PROJECT_MANAGER.into(),
            label: "项目经理".into(),
            description: "规划项目、拆分任务、安排负责人、维护依赖并推动交付。".into(),
            skill_name: "relay-profession-project-manager".into(),
            can_create_tasks: true,
        },
        CompanyProfession {
            key: COMPANY_PROFESSION_PRODUCT_MANAGER.into(),
            label: "产品经理".into(),
            description: "澄清需求、定义验收标准、拆分产品任务并协调交付。".into(),
            skill_name: "relay-profession-product-manager".into(),
            can_create_tasks: true,
        },
        CompanyProfession {
            key: COMPANY_PROFESSION_TECHNICAL_MANAGER.into(),
            label: "技术经理".into(),
            description: "制定技术方案、拆分工程任务、安排技术负责人并推动质量交付。".into(),
            skill_name: "relay-profession-technical-manager".into(),
            can_create_tasks: true,
        },
        CompanyProfession {
            key: COMPANY_PROFESSION_SOLUTION_ARCHITECT.into(),
            label: "解决方案架构师".into(),
            description: "定义系统边界、接口、数据流、非功能约束和架构验证方案。".into(),
            skill_name: "relay-profession-solution-architect".into(),
            can_create_tasks: false,
        },
        CompanyProfession {
            key: COMPANY_PROFESSION_SOFTWARE_ENGINEER.into(),
            label: "软件工程师".into(),
            description: "实现、测试和交付分配给自己的工程任务。".into(),
            skill_name: "relay-profession-software-engineer".into(),
            can_create_tasks: false,
        },
        CompanyProfession {
            key: COMPANY_PROFESSION_FRONTEND_ENGINEER.into(),
            label: "前端工程师".into(),
            description: "实现 Web 界面、组件、交互状态、可访问性和真实接口集成。".into(),
            skill_name: "relay-profession-frontend-engineer".into(),
            can_create_tasks: false,
        },
        CompanyProfession {
            key: COMPANY_PROFESSION_BACKEND_ENGINEER.into(),
            label: "后端工程师".into(),
            description: "实现服务、接口、数据持久化、事务一致性和外部系统集成。".into(),
            skill_name: "relay-profession-backend-engineer".into(),
            can_create_tasks: false,
        },
        CompanyProfession {
            key: COMPANY_PROFESSION_MOBILE_ENGINEER.into(),
            label: "移动端工程师".into(),
            description: "实现 iOS、Android、PDA 或其他设备端应用并验证设备差异。".into(),
            skill_name: "relay-profession-mobile-engineer".into(),
            can_create_tasks: false,
        },
        CompanyProfession {
            key: COMPANY_PROFESSION_DATA_ENGINEER.into(),
            label: "数据工程师".into(),
            description: "实现数据模型、管道、迁移、回填、对账和质量控制。".into(),
            skill_name: "relay-profession-data-engineer".into(),
            can_create_tasks: false,
        },
        CompanyProfession {
            key: COMPANY_PROFESSION_DEVOPS_ENGINEER.into(),
            label: "DevOps / SRE 工程师".into(),
            description: "维护构建发布、环境配置、可观测性、可靠性和故障恢复。".into(),
            skill_name: "relay-profession-devops-engineer".into(),
            can_create_tasks: false,
        },
        CompanyProfession {
            key: COMPANY_PROFESSION_QA_ENGINEER.into(),
            label: "测试工程师".into(),
            description: "制定验证方案、执行测试并报告质量风险和失败结果。".into(),
            skill_name: "relay-profession-qa-engineer".into(),
            can_create_tasks: false,
        },
        CompanyProfession {
            key: COMPANY_PROFESSION_UI_DESIGNER.into(),
            label: "UI 设计师".into(),
            description: "完成视觉界面、组件、设计 Token、状态和开发交付规范。".into(),
            skill_name: "relay-profession-ui-designer".into(),
            can_create_tasks: false,
        },
        CompanyProfession {
            key: COMPANY_PROFESSION_UX_DESIGNER.into(),
            label: "UX 设计师".into(),
            description: "研究并验证用户旅程、信息架构、任务流和交互体验。".into(),
            skill_name: "relay-profession-ux-designer".into(),
            can_create_tasks: false,
        },
        CompanyProfession {
            key: COMPANY_PROFESSION_PRODUCT_DESIGNER.into(),
            label: "产品设计师".into(),
            description: "完成交互、视觉和体验设计并维护设计交付物。".into(),
            skill_name: "relay-profession-product-designer".into(),
            can_create_tasks: false,
        },
        CompanyProfession {
            key: COMPANY_PROFESSION_IMPLEMENTATION_CONSULTANT.into(),
            label: "实施顾问".into(),
            description: "负责差距分析、配置、UAT、培训、切换、采用和项目交接。".into(),
            skill_name: "relay-profession-implementation-consultant".into(),
            can_create_tasks: false,
        },
        CompanyProfession {
            key: COMPANY_PROFESSION_DOMAIN_EXPERT.into(),
            label: "领域专家".into(),
            description: "验证领域术语、流程、规则、例外、控制点和验收场景。".into(),
            skill_name: "relay-profession-domain-expert".into(),
            can_create_tasks: false,
        },
        CompanyProfession {
            key: COMPANY_PROFESSION_BUSINESS_ANALYST.into(),
            label: "业务分析师".into(),
            description: "调研业务、整理流程和规则，并交付可验证的分析结论。".into(),
            skill_name: "relay-profession-business-analyst".into(),
            can_create_tasks: false,
        },
        CompanyProfession {
            key: COMPANY_PROFESSION_OPERATIONS_SPECIALIST.into(),
            label: "运营专员".into(),
            description: "执行运营任务、维护业务数据并反馈异常和效果。".into(),
            skill_name: "relay-profession-operations-specialist".into(),
            can_create_tasks: false,
        },
        CompanyProfession {
            key: COMPANY_PROFESSION_GENERAL_MEMBER.into(),
            label: "通用成员".into(),
            description: "处理明确分配的工作，并按要求同步进度、阻塞和结果。".into(),
            skill_name: "relay-profession-general-member".into(),
            can_create_tasks: false,
        },
    ]
}

pub fn company_profession_by_key(key: &str) -> Option<CompanyProfession> {
    company_profession_catalog()
        .into_iter()
        .find(|profession| profession.key == key.trim())
}

pub fn infer_company_profession(job_title: Option<&str>) -> CompanyProfession {
    let normalized = job_title.unwrap_or_default().trim().to_ascii_lowercase();
    let key = if normalized.contains("项目经理")
        || normalized.contains("项目负责人")
        || normalized == "project manager"
        || normalized == "pm"
    {
        COMPANY_PROFESSION_PROJECT_MANAGER
    } else if normalized.contains("产品经理")
        || normalized.contains("产品负责人")
        || normalized.contains("product manager")
        || normalized.contains("product owner")
    {
        COMPANY_PROFESSION_PRODUCT_MANAGER
    } else if normalized.contains("技术经理")
        || normalized.contains("技术负责人")
        || normalized.contains("工程经理")
        || normalized.contains("研发经理")
        || normalized.contains("技术总监")
        || normalized == "cto"
        || normalized.contains("technical manager")
        || normalized.contains("engineering manager")
        || normalized.contains("tech lead")
    {
        COMPANY_PROFESSION_TECHNICAL_MANAGER
    } else if normalized.contains("测试")
        || normalized.contains("质量")
        || normalized.contains("qa")
    {
        COMPANY_PROFESSION_QA_ENGINEER
    } else if normalized.contains("架构师")
        || normalized.contains("solution architect")
        || normalized.contains("software architect")
        || normalized.contains("system architect")
    {
        COMPANY_PROFESSION_SOLUTION_ARCHITECT
    } else if normalized.contains("前端")
        || normalized.contains("frontend")
        || normalized.contains("front-end")
        || normalized.contains("web developer")
        || normalized.contains("web engineer")
    {
        COMPANY_PROFESSION_FRONTEND_ENGINEER
    } else if normalized.contains("后端")
        || normalized.contains("服务端")
        || normalized.contains("backend")
        || normalized.contains("back-end")
        || normalized.contains("server engineer")
    {
        COMPANY_PROFESSION_BACKEND_ENGINEER
    } else if normalized.contains("移动端")
        || normalized.contains("客户端")
        || normalized.contains("mobile")
        || normalized.contains("android")
        || normalized.contains("ios")
        || normalized.contains("pda")
    {
        COMPANY_PROFESSION_MOBILE_ENGINEER
    } else if normalized.contains("数据工程")
        || normalized.contains("数据平台")
        || normalized.contains("数据迁移")
        || normalized.contains("主数据")
        || normalized.contains("data engineer")
        || normalized.contains("etl")
    {
        COMPANY_PROFESSION_DATA_ENGINEER
    } else if normalized.contains("devops")
        || normalized.contains("sre")
        || normalized.contains("可靠性")
        || normalized.contains("运维工程")
        || normalized.contains("平台工程")
    {
        COMPANY_PROFESSION_DEVOPS_ENGINEER
    } else if normalized.contains("ui 设计")
        || normalized.contains("界面设计")
        || normalized.contains("视觉设计")
        || normalized.contains("ui designer")
        || normalized.contains("visual designer")
    {
        COMPANY_PROFESSION_UI_DESIGNER
    } else if normalized.contains("ux")
        || normalized.contains("用户体验")
        || normalized.contains("交互设计")
        || normalized.contains("experience designer")
        || normalized.contains("interaction designer")
    {
        COMPANY_PROFESSION_UX_DESIGNER
    } else if normalized.contains("产品设计") || normalized.contains("product designer") {
        COMPANY_PROFESSION_PRODUCT_DESIGNER
    } else if normalized.contains("实施顾问")
        || normalized.contains("实施工程")
        || normalized.contains("implementation consultant")
        || normalized.contains("implementation engineer")
    {
        COMPANY_PROFESSION_IMPLEMENTATION_CONSULTANT
    } else if normalized.contains("领域专家")
        || normalized.contains("业务专家")
        || normalized.contains("行业专家")
        || normalized.contains("subject matter expert")
        || normalized.contains("sme")
        || normalized.ends_with("专家")
    {
        COMPANY_PROFESSION_DOMAIN_EXPERT
    } else if normalized.contains("设计") || normalized.contains("designer") {
        COMPANY_PROFESSION_PRODUCT_DESIGNER
    } else if normalized.contains("分析")
        || normalized.contains("顾问")
        || normalized.contains("analyst")
    {
        COMPANY_PROFESSION_BUSINESS_ANALYST
    } else if normalized.contains("运营")
        || normalized.contains("销售")
        || normalized.contains("operation")
    {
        COMPANY_PROFESSION_OPERATIONS_SPECIALIST
    } else if normalized.contains("工程")
        || normalized.contains("开发")
        || normalized.contains("程序")
        || normalized.contains("engineer")
        || normalized.contains("developer")
    {
        COMPANY_PROFESSION_SOFTWARE_ENGINEER
    } else {
        COMPANY_PROFESSION_GENERAL_MEMBER
    };
    company_profession_by_key(key).expect("built-in company profession should exist")
}

pub fn default_company_agent_permissions_for_profession(
    role_key: &str,
    profession_key: &str,
) -> Vec<String> {
    let mut permissions = default_company_agent_permissions(role_key);
    if matches!(
        profession_key,
        COMPANY_PROFESSION_PROJECT_MANAGER
            | COMPANY_PROFESSION_PRODUCT_MANAGER
            | COMPANY_PROFESSION_TECHNICAL_MANAGER
    ) {
        permissions.push(COMPANY_PERMISSION_TASK_ASSIGN.into());
    }
    if profession_key == COMPANY_PROFESSION_PROJECT_MANAGER {
        permissions.push(COMPANY_PERMISSION_PROJECT_CREATE.into());
        permissions.push(COMPANY_PERMISSION_PROJECT_MANAGE.into());
    }
    permissions.sort();
    permissions.dedup();
    permissions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn technical_manager_is_a_system_profession_with_task_assignment_permission() {
        let profession = company_profession_by_key(COMPANY_PROFESSION_TECHNICAL_MANAGER)
            .expect("technical manager profession should exist");
        assert_eq!(profession.label, "技术经理");
        assert_eq!(profession.skill_name, "relay-profession-technical-manager");
        assert!(profession.can_create_tasks);

        let permissions = default_company_agent_permissions_for_profession(
            COMPANY_AGENT_ROLE_MEMBER,
            COMPANY_PROFESSION_TECHNICAL_MANAGER,
        );
        assert!(permissions
            .iter()
            .any(|permission| permission == COMPANY_PERMISSION_TASK_ASSIGN));
        assert!(!permissions
            .iter()
            .any(|permission| permission == COMPANY_PERMISSION_PROJECT_MANAGE));
    }

    #[test]
    fn historical_technical_lead_titles_map_to_technical_manager() {
        for title in [
            "技术经理",
            "WMS 技术负责人",
            "研发经理",
            "Engineering Manager",
            "Tech Lead",
        ] {
            assert_eq!(
                infer_company_profession(Some(title)).key,
                COMPANY_PROFESSION_TECHNICAL_MANAGER
            );
        }
    }

    #[test]
    fn specialist_titles_map_to_specific_professions() {
        for (title, expected) in [
            ("WMS 解决方案架构师", COMPANY_PROFESSION_SOLUTION_ARCHITECT),
            (
                "WMS 前端与 PDA 工程师",
                COMPANY_PROFESSION_FRONTEND_ENGINEER,
            ),
            ("WMS 后端工程师", COMPANY_PROFESSION_BACKEND_ENGINEER),
            ("Android 客户端工程师", COMPANY_PROFESSION_MOBILE_ENGINEER),
            ("WMS 主数据与迁移工程师", COMPANY_PROFESSION_DATA_ENGINEER),
            ("WMS 集成与可靠性工程师", COMPANY_PROFESSION_DEVOPS_ENGINEER),
            ("视觉 UI 设计师", COMPANY_PROFESSION_UI_DESIGNER),
            ("用户体验 UX 设计师", COMPANY_PROFESSION_UX_DESIGNER),
            ("WMS 实施顾问", COMPANY_PROFESSION_IMPLEMENTATION_CONSULTANT),
            ("仓储运营专家", COMPANY_PROFESSION_DOMAIN_EXPERT),
        ] {
            assert_eq!(
                infer_company_profession(Some(title)).key,
                expected,
                "{title}"
            );
        }
    }

    #[test]
    fn specialist_professions_keep_execution_only_task_permissions() {
        for profession_key in [
            COMPANY_PROFESSION_SOLUTION_ARCHITECT,
            COMPANY_PROFESSION_FRONTEND_ENGINEER,
            COMPANY_PROFESSION_BACKEND_ENGINEER,
            COMPANY_PROFESSION_MOBILE_ENGINEER,
            COMPANY_PROFESSION_DATA_ENGINEER,
            COMPANY_PROFESSION_DEVOPS_ENGINEER,
            COMPANY_PROFESSION_UI_DESIGNER,
            COMPANY_PROFESSION_UX_DESIGNER,
            COMPANY_PROFESSION_IMPLEMENTATION_CONSULTANT,
            COMPANY_PROFESSION_DOMAIN_EXPERT,
        ] {
            let profession = company_profession_by_key(profession_key)
                .expect("specialist profession should exist");
            assert!(!profession.can_create_tasks, "{profession_key}");
            let permissions = default_company_agent_permissions_for_profession(
                COMPANY_AGENT_ROLE_MEMBER,
                profession_key,
            );
            assert!(!permissions
                .iter()
                .any(|permission| permission == COMPANY_PERMISSION_TASK_ASSIGN));
        }
    }
}
