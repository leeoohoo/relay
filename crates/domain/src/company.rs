use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

pub mod environments;
pub mod execution;
pub mod gates;
pub use environments::*;
pub use execution::*;
pub use gates::*;

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
pub const COMPANY_PROFESSION_FULLSTACK_ENGINEER: &str = "fullstack_engineer";
pub const COMPANY_PROFESSION_DESKTOP_ENGINEER: &str = "desktop_engineer";
pub const COMPANY_PROFESSION_GAME_ENGINEER: &str = "game_engineer";
pub const COMPANY_PROFESSION_EMBEDDED_IOT_ENGINEER: &str = "embedded_iot_engineer";
pub const COMPANY_PROFESSION_DATABASE_ENGINEER: &str = "database_engineer";
pub const COMPANY_PROFESSION_SECURITY_ENGINEER: &str = "security_engineer";
pub const COMPANY_PROFESSION_MACHINE_LEARNING_ENGINEER: &str = "machine_learning_engineer";
pub const COMPANY_PROFESSION_DATA_ANALYST: &str = "data_analyst";
pub const COMPANY_PROFESSION_GAME_DESIGNER: &str = "game_designer";
pub const COMPANY_PROFESSION_TECHNICAL_WRITER: &str = "technical_writer";
pub const COMPANY_PROFESSION_GROWTH_MARKETING_SPECIALIST: &str = "growth_marketing_specialist";
pub const COMPANY_PROFESSION_RESEARCH_SPECIALIST: &str = "research_specialist";
pub const COMPANY_PROFESSION_ERP_CONSULTANT: &str = "erp_consultant";
pub const COMPANY_PROFESSION_WMS_CONSULTANT: &str = "wms_consultant";
pub const COMPANY_PROFESSION_GENERAL_MEMBER: &str = "general_member";

pub const COMPANY_SKILL_LANGUAGE_ZH_CN: &str = "zh-CN";
pub const COMPANY_SKILL_LANGUAGE_EN: &str = "en";

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
pub const PROJECT_STATUS_PAUSED: &str = "paused";
pub const PROJECT_STATUS_BLOCKED: &str = "blocked";
pub const PROJECT_STATUS_COMPLETED: &str = "completed";
pub const PROJECT_STATUS_CANCELLED: &str = "cancelled";

pub const PROJECT_TYPE_SOFTWARE_DEVELOPMENT: &str = "software_development";
pub const PROJECT_TYPE_WEB_APPLICATION: &str = "web_application";
pub const PROJECT_TYPE_MOBILE_APPLICATION: &str = "mobile_application";
pub const PROJECT_TYPE_DESKTOP_APPLICATION: &str = "desktop_application";
pub const PROJECT_TYPE_BACKEND_SERVICE: &str = "backend_service";
pub const PROJECT_TYPE_LIBRARY_SDK: &str = "library_sdk";
pub const PROJECT_TYPE_GAME_DEVELOPMENT: &str = "game_development";
pub const PROJECT_TYPE_IOT_EMBEDDED_SYSTEM: &str = "iot_embedded_system";
pub const PROJECT_TYPE_ENTERPRISE_ERP: &str = "enterprise_erp";
pub const PROJECT_TYPE_WAREHOUSE_MANAGEMENT_SYSTEM: &str = "warehouse_management_system";
pub const PROJECT_TYPE_CUSTOMER_RELATIONSHIP_MANAGEMENT: &str = "customer_relationship_management";
pub const PROJECT_TYPE_MANUFACTURING_EXECUTION_SYSTEM: &str = "manufacturing_execution_system";
pub const PROJECT_TYPE_ECOMMERCE_PLATFORM: &str = "ecommerce_platform";
pub const PROJECT_TYPE_NOVEL_WRITING: &str = "novel_writing";
pub const PROJECT_TYPE_GENERAL_WRITING: &str = "general_writing";
pub const PROJECT_TYPE_RESEARCH: &str = "research";
pub const PROJECT_TYPE_DATA_ANALYSIS: &str = "data_analysis";
pub const PROJECT_TYPE_DATA_ENGINEERING_PLATFORM: &str = "data_engineering_platform";
pub const PROJECT_TYPE_MACHINE_LEARNING_SYSTEM: &str = "machine_learning_system";
pub const PROJECT_TYPE_PRODUCT_DESIGN: &str = "product_design";
pub const PROJECT_TYPE_DESIGN_SYSTEM_BRAND: &str = "design_system_brand";
pub const PROJECT_TYPE_MARKETING_CONTENT: &str = "marketing_content";
pub const PROJECT_TYPE_DOCUMENTATION: &str = "documentation";
pub const PROJECT_TYPE_AUTOMATION: &str = "automation";
pub const PROJECT_TYPE_OPERATIONS: &str = "operations";
pub const PROJECT_TYPE_IMPLEMENTATION_MIGRATION: &str = "implementation_migration";
pub const PROJECT_TYPE_GENERAL: &str = "general";

pub const PROJECT_TYPE_SOURCE_HUMAN: &str = "human";
pub const PROJECT_TYPE_SOURCE_DESCRIPTION: &str = "description_inference";
pub const PROJECT_TYPE_SOURCE_FOLDER: &str = "folder_inference";
pub const PROJECT_TYPE_SOURCE_SYSTEM: &str = "system_default";

pub const PROJECT_MEMBER_ROLE_OWNER: &str = "owner";
pub const PROJECT_MEMBER_ROLE_MEMBER: &str = "member";

pub const PROJECT_TASK_STATUS_TODO: &str = "todo";
pub const PROJECT_TASK_STATUS_IN_PROGRESS: &str = "in_progress";
pub const PROJECT_TASK_STATUS_BLOCKED: &str = "blocked";
pub const PROJECT_TASK_STATUS_DONE: &str = "done";
pub const PROJECT_TASK_STATUS_FAILED: &str = "failed";
pub const PROJECT_TASK_STATUS_CANCELLED: &str = "cancelled";
pub const PROJECT_TASK_DEPENDENCY_SUCCESS: &str = "success";
pub const PROJECT_TASK_DEPENDENCY_COMPLETION: &str = "completion";
pub const PROJECT_TASK_DEPENDENCY_FAILURE: &str = "failure";

pub fn project_task_dependency_satisfied(condition: &str, status: &str) -> bool {
    match condition {
        PROJECT_TASK_DEPENDENCY_SUCCESS => {
            matches!(
                status,
                PROJECT_TASK_STATUS_DONE | PROJECT_TASK_STATUS_CANCELLED
            )
        }
        PROJECT_TASK_DEPENDENCY_COMPLETION => matches!(
            status,
            PROJECT_TASK_STATUS_DONE | PROJECT_TASK_STATUS_FAILED | PROJECT_TASK_STATUS_CANCELLED
        ),
        PROJECT_TASK_DEPENDENCY_FAILURE => status == PROJECT_TASK_STATUS_FAILED,
        _ => false,
    }
}

pub const PROJECT_TASK_PRIORITY_LOW: &str = "low";
pub const PROJECT_TASK_PRIORITY_NORMAL: &str = "normal";
pub const PROJECT_TASK_PRIORITY_HIGH: &str = "high";
pub const PROJECT_TASK_PRIORITY_URGENT: &str = "urgent";

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
pub const AGENT_TOOL_APPROVAL_MODE_ONCE: &str = "once";
pub const AGENT_TOOL_APPROVAL_MODE_ALWAYS: &str = "always";
pub const AGENT_TOOL_APPROVAL_MODE_ALWAYS_LOCALHOST: &str = "always_localhost";
pub const COMPANY_GOVERNANCE_POLICY_STATUS_ACTIVE: &str = "active";
pub const COMPANY_GOVERNANCE_POLICY_STATUS_ARCHIVED: &str = "archived";

pub const AGENT_RUNTIME_MESSAGE_POLICY_DIRECT_ACK: &str = "direct_ack";
pub const AGENT_RUNTIME_MESSAGE_POLICY_MENTIONED_ACK: &str = "mentioned_ack";

pub const AGENT_CODEX_TRIGGER_STATUS_ACTIVE: &str = "active";
pub const AGENT_CODEX_TRIGGER_STATUS_PAUSED: &str = "paused";
pub const AGENT_CODEX_TRIGGER_STATUS_ERROR: &str = "error";
pub const AGENT_CODEX_SANDBOX_READ_ONLY: &str = "read_only";
pub const AGENT_CODEX_SANDBOX_WORKSPACE_WRITE: &str = "workspace_write";
pub const AGENT_CODEX_SETTING_INHERIT: &str = "inherit";
pub const AGENT_CODEX_APPROVAL_POLICY_NEVER: &str = "never";
pub const AGENT_CODEX_APPROVAL_POLICY_ON_REQUEST: &str = "on-request";
pub const AGENT_TOOL_APPROVAL_SOURCE_CODEX: &str = "codex";
pub const AGENT_CODEX_APPROVAL_TOOL_COMMAND: &str = "codex.command_execution";
pub const AGENT_CODEX_APPROVAL_TOOL_FILE_CHANGE: &str = "codex.file_change";
pub const AGENT_CODEX_APPROVAL_TOOL_PERMISSIONS: &str = "codex.permissions";
pub const AGENT_CODEX_APPROVAL_TOOL_WEBSITE_ACCESS: &str = "codex.website_access";
pub const AGENT_CODEX_APPROVAL_SCOPE_KEY: &str = "relay_approval_scope";
pub const AGENT_CODEX_APPROVAL_TARGET_KEY: &str = "relay_approval_target";
pub const AGENT_CODEX_APPROVAL_LOCAL_TARGET_KEY: &str = "relay_approval_local_target";
pub const AGENT_CODEX_TRIGGER_TYPE_SCHEDULED: &str = "scheduled";
pub const AGENT_CODEX_TRIGGER_TYPE_MANUAL: &str = "manual";
pub const AGENT_CODEX_TRIGGER_TYPE_MESSAGE: &str = "message";
pub const AGENT_CODEX_TRIGGER_TYPE_TASK: &str = "task";
pub const AGENT_CODEX_TRIGGER_TYPE_ASSET_REFRESH: &str = "asset_refresh";
pub const AGENT_CODEX_WAKE_REASON_MESSAGE: &str = "message";
pub const AGENT_CODEX_WAKE_REASON_TASK_READY: &str = "task_ready";
pub const AGENT_CODEX_WAKE_REASON_PROJECT_RESUMED: &str = "project_resumed";
pub const AGENT_CODEX_WAKE_REASON_INTENT_RECOVERY: &str = "intent_recovery";
pub const AGENT_CODEX_RUN_STATUS_RUNNING: &str = "running";
pub const AGENT_CODEX_RUN_STATUS_SUCCEEDED: &str = "succeeded";
pub const AGENT_CODEX_RUN_STATUS_FAILED: &str = "failed";
pub const AGENT_CODEX_RUN_STATUS_TIMED_OUT: &str = "timed_out";
pub const AGENT_CODEX_RUN_STATUS_CANCELLED: &str = "cancelled";
pub const AGENT_CODEX_RUN_STATUS_LEASE_LOST: &str = "lease_lost";
pub const AGENT_CODEX_SESSION_KIND_CONTROL: &str = "control";
pub const AGENT_CODEX_SESSION_KIND_PROJECT: &str = "project";
pub const AGENT_CODEX_SESSION_STATUS_ACTIVE: &str = "active";
pub const AGENT_CODEX_SESSION_STATUS_ARCHIVED: &str = "archived";
pub const AGENT_EXECUTION_INTENT_ACTION_EXECUTE: &str = "execute";
pub const AGENT_EXECUTION_INTENT_ACTION_REPLACE_SESSION: &str = "replace_session";
pub const AGENT_EXECUTION_INTENT_STATUS_PENDING: &str = "pending";
pub const AGENT_EXECUTION_INTENT_STATUS_RUNNING: &str = "running";
pub const AGENT_EXECUTION_INTENT_STATUS_COMPLETED: &str = "completed";
pub const AGENT_EXECUTION_INTENT_STATUS_FAILED: &str = "failed";
pub const AGENT_EXECUTION_INTENT_STATUS_CANCELLED: &str = "cancelled";

pub fn is_agent_codex_wake_reason(value: &str) -> bool {
    matches!(
        value,
        AGENT_CODEX_WAKE_REASON_MESSAGE
            | AGENT_CODEX_WAKE_REASON_TASK_READY
            | AGENT_CODEX_WAKE_REASON_PROJECT_RESUMED
            | AGENT_CODEX_WAKE_REASON_INTENT_RECOVERY
    )
}

pub const AGENT_MEMORY_SCOPE_AGENT: &str = "agent";
pub const AGENT_MEMORY_SCOPE_CONTROL: &str = "control";
pub const AGENT_MEMORY_SCOPE_PROJECT: &str = "project";
pub const AGENT_MEMORY_SCOPE_SESSION: &str = "session";
pub const AGENT_MEMORY_TIER_SHORT_TERM: &str = "short_term";
pub const AGENT_MEMORY_TIER_LONG_TERM: &str = "long_term";
pub const AGENT_MEMORY_INJECTION_ALWAYS: &str = "always";
pub const AGENT_MEMORY_INJECTION_ON_DEMAND: &str = "on_demand";
pub const AGENT_MEMORY_INJECTION_NEVER: &str = "never";
pub const AGENT_MEMORY_VISIBILITY_CONTROL: &str = "control";
pub const AGENT_MEMORY_VISIBILITY_WORKER: &str = "worker";
pub const AGENT_MEMORY_VISIBILITY_BOTH: &str = "both";
pub const AGENT_MEMORY_STATUS_DRAFT: &str = "draft";
pub const AGENT_MEMORY_STATUS_ACTIVE: &str = "active";
pub const AGENT_MEMORY_STATUS_ARCHIVED: &str = "archived";
pub const AGENT_MEMORY_STATUS_SUPERSEDED: &str = "superseded";

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
    pub label_en: String,
    pub description: String,
    pub description_en: String,
    pub category_key: String,
    pub category_label: String,
    pub category_label_en: String,
    pub skill_name: String,
    pub skill_markdown: String,
    pub skill_markdown_en: String,
    pub can_create_tasks: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CompanyProjectTypeDefinition {
    pub key: String,
    pub label: String,
    pub label_en: String,
    pub description: String,
    pub description_en: String,
    pub category_key: String,
    pub category_label: String,
    pub category_label_en: String,
    pub rule_markdown: String,
    pub rule_markdown_en: String,
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
    pub project_type: String,
    pub project_type_source: String,
    pub project_type_confidence: i32,
    pub project_type_evidence: Vec<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentMemorySourceRef {
    pub source_type: String,
    pub source_id: String,
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMemory {
    pub id: Uuid,
    pub company_id: Uuid,
    pub owner_agent_id: Uuid,
    #[serde(skip_serializing)]
    pub scope: String,
    pub project_id: Option<Uuid>,
    pub session_id: Option<Uuid>,
    pub memory_tier: String,
    pub injection_mode: String,
    pub visibility: String,
    pub memory_type: String,
    pub topic_key: String,
    pub title: String,
    pub summary: String,
    pub when_to_use: String,
    pub tags: Vec<String>,
    pub importance: i32,
    pub confidence: i32,
    pub pinned: bool,
    pub status: String,
    pub source_refs: Vec<AgentMemorySourceRef>,
    pub supersedes_memory_id: Option<Uuid>,
    pub expires_at: Option<DateTime<Utc>>,
    pub verified_by_agent_id: Option<Uuid>,
    pub verified_by_human_user_id: Option<Uuid>,
    pub verified_at: Option<DateTime<Utc>>,
    pub created_by_agent_id: Option<Uuid>,
    pub created_by_human_user_id: Option<Uuid>,
    pub updated_by_agent_id: Option<Uuid>,
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
    pub id: Uuid,
    pub agent_profile_id: Uuid,
    pub session_kind: String,
    pub scope_key: String,
    pub project_id: Option<Uuid>,
    pub generation: i32,
    pub codex_thread_id: String,
    pub workspace_key: String,
    pub status: String,
    pub summary_short: String,
    pub checkpoint_json: serde_json::Value,
    pub skill_bundle_version: String,
    pub memory_snapshot_version: String,
    pub policy_version: String,
    pub created_at: DateTime<Utc>,
    pub last_used_at: DateTime<Utc>,
    pub archived_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentExecutionIntent {
    pub id: Uuid,
    pub company_id: Uuid,
    pub agent_profile_id: Uuid,
    pub project_id: Uuid,
    pub worker_session_id: Option<Uuid>,
    pub source_event_ids: Vec<Uuid>,
    pub task_ids: Vec<Uuid>,
    pub action_type: String,
    pub objective: String,
    pub acceptance_criteria: Vec<String>,
    pub priority: String,
    pub dedupe_key: String,
    pub status: String,
    pub result_summary: String,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub claimed_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
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

pub const CODEX_PLUGIN_OPERATION_INSTALL: &str = "install";
pub const CODEX_PLUGIN_OPERATION_REMOVE: &str = "remove";
pub const CODEX_PLUGIN_OPERATION_REFRESH: &str = "refresh";
pub const CODEX_PLUGIN_OPERATION_STATUS_QUEUED: &str = "queued";
pub const CODEX_PLUGIN_OPERATION_STATUS_RUNNING: &str = "running";
pub const CODEX_PLUGIN_OPERATION_STATUS_SUCCEEDED: &str = "succeeded";
pub const CODEX_PLUGIN_OPERATION_STATUS_FAILED: &str = "failed";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexPluginCatalogSnapshot {
    pub runner_id: String,
    pub target_selector: String,
    pub hostname: String,
    pub codex_version: Option<String>,
    pub fingerprint: String,
    pub discovery_status: String,
    pub diagnostic_message: Option<String>,
    pub installed: Value,
    pub available: Value,
    pub marketplaces: Value,
    pub discovered_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexPluginOperation {
    pub id: Uuid,
    pub company_id: Uuid,
    pub target_runner_id: String,
    pub target_selector: String,
    pub operation: String,
    pub plugin_id: Option<String>,
    pub status: String,
    pub requested_by_human_user_id: Uuid,
    pub lease_owner: Option<String>,
    pub lease_expires_at: Option<DateTime<Utc>>,
    pub attempt_count: i32,
    pub error_message: Option<String>,
    pub result: Value,
    pub requested_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
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
    pub dependency_condition: String,
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
    #[serde(default)]
    pub managed_workspace_root: Option<String>,
    #[serde(default = "default_company_skill_language")]
    pub skill_language: String,
}

pub fn default_company_skill_language() -> String {
    COMPANY_SKILL_LANGUAGE_ZH_CN.into()
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

mod professions;
mod project_types;

pub use professions::{
    company_profession_by_key, company_profession_catalog, infer_company_profession,
};
pub use project_types::{
    company_project_type_by_key, company_project_type_catalog, infer_company_project_type,
};

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
mod tests;
