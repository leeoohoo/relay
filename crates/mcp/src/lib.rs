use std::{sync::Arc, time::Duration};

use ai_chat_application::{
    AddCompanyProjectMemberInput, AgentStaffingHireInput, AgentStaffingStatusInput,
    BatchUpdateCompanyProjectTasksInput, ChangeCompanyProjectTaskDependencyInput,
    CompanyProjectAssetInput, CreateCompanyGroupConversationInput, CreateCompanyProjectInput,
    CreateCompanyProjectStatusUpdateInput, CreateCompanyProjectTaskInput, GetAgentMemoryInput,
    GetCompanyAgentContextInput, GetCompanyProjectInput, ListCompanyGroupUnreadInput,
    MarkCompanyGroupReadInput, MarkInboxEventProcessedInput, OpenCompanyDirectConversationInput,
    OwnershipProofVerifier, PlatformApp, PlatformRepository, RememberAgentMemoryInput,
    RemoveCompanyProjectMemberInput, ReplaceCompanyProjectAssetsInput,
    ReplyCompanyInboxMessageInput, SearchAgentMemoriesInput, SendCompanyMessageWithMentionsInput,
    SetAgentMemoryStateInput, TransferCompanyProjectOwnerInput, UpdateAgentMemoryInput,
    UpdateCompanyAgentWorkProfileInput, UpdateCompanyProjectInput, UpdateCompanyProjectRuleInput,
    UpdateCompanyProjectTaskInput, UpsertCompanyProjectGitInput,
};
use ai_chat_domain::agent_identity::AgentActionStatus;
use ai_chat_domain::company::{
    company_profession_by_key, infer_company_profession, AgentMemorySourceRef,
    AGENT_MEMORY_STATUS_ARCHIVED, AGENT_MEMORY_STATUS_SUPERSEDED,
    COMPANY_PERMISSION_PROJECT_CREATE, COMPANY_PERMISSION_PROJECT_MANAGE,
    COMPANY_PERMISSION_STAFF_HIRE, COMPANY_PERMISSION_STAFF_SUSPEND,
    COMPANY_PERMISSION_STAFF_TERMINATE, COMPANY_PERMISSION_TASK_ASSIGN,
    COMPANY_PERMISSION_TASK_UPDATE, PROJECT_STATUS_PAUSED,
};
use ai_chat_infrastructure::project_git::{
    ProjectGitProvisionRequest, ProjectGitProvisioner, ProvisionedProjectGit,
};
use ai_chat_shared::{AppError, AppResult};
use chrono::{DateTime, Utc};
use http::{header::AUTHORIZATION, request::Parts, HeaderMap};
use rmcp::{
    model::{
        CallToolRequestParams, CallToolResult, ErrorCode, Implementation, JsonObject,
        ListToolsResult, PaginatedRequestParams, ServerCapabilities, ServerInfo, Tool,
        ToolAnnotations,
    },
    service::RequestContext,
    ErrorData, RoleServer, ServerHandler,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

pub const STANDARD_MCP_PATH: &str = "/mcp";

#[derive(Debug, Clone, Serialize)]
pub struct McpInvocation {
    pub tool: String,
    pub agent_id: Uuid,
    pub output: Value,
}

#[derive(Clone)]
pub struct McpGateway<R: PlatformRepository, V: OwnershipProofVerifier> {
    platform: PlatformApp<R, V>,
    fallback_agent_key: Option<String>,
    project_git_provisioner: Option<Arc<dyn ProjectGitProvisioner>>,
}

mod dispatch;
mod dispatch_agent;
mod dispatch_chat;
mod dispatch_legacy;
mod dispatch_project;
mod dispatch_staff;
mod dispatch_task;
mod gateway;
mod handler;
mod tools;

pub use handler::{agent_key_from_headers, agent_run_token_from_headers, AiChatMcpHandler};
pub use tools::standard_mcp_tools;

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct EmptyInput {}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct AgentProfileUpdateToolInput {
    #[schemars(description = "Company responsibilities owned by this Agent; at most 20 items.")]
    responsibilities: Option<Vec<String>>,
    #[schemars(description = "Skills or specialist capabilities; at most 30 items.")]
    skills: Option<Vec<String>>,
    #[schemars(description = "Current work focus; send an empty string to clear it.")]
    current_focus: Option<String>,
    #[schemars(
        description = "Collaboration availability: available, low_cost_only, or unavailable."
    )]
    collaboration_preference: Option<String>,
    #[schemars(description = "Optional retry key for this profile update.")]
    idempotency_key: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct AgentMemoryToolInput {
    #[serde(flatten)]
    operation: AgentMemoryOperation,
    #[schemars(description = "Optional retry key for mutating memory actions.")]
    idempotency_key: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(tag = "action", rename_all = "snake_case")]
enum AgentMemoryOperation {
    Overview {
        company_id: Uuid,
        project_id: Option<Uuid>,
    },
    Search {
        company_id: Uuid,
        project_id: Option<Uuid>,
        query: Option<String>,
        #[serde(default)]
        memory_tiers: Vec<String>,
        #[serde(default)]
        memory_types: Vec<String>,
        #[serde(default)]
        tags: Vec<String>,
        status: Option<String>,
        limit: Option<usize>,
    },
    Get {
        company_id: Uuid,
        memory_id: Uuid,
    },
    Remember {
        company_id: Uuid,
        project_id: Option<Uuid>,
        #[schemars(
            description = "Memory tier: long_term is injected into this Agent's generated Skill on every wake-up; short_term is retrieved on demand through MCP search."
        )]
        memory_tier: String,
        #[schemars(
            description = "Distilled knowledge type: fact, decision, lesson, preference, procedure, relationship, or handoff."
        )]
        memory_type: String,
        #[schemars(
            description = "Stable ASCII topic identifier used to merge knowledge instead of creating duplicates."
        )]
        topic_key: String,
        title: String,
        #[schemars(
            description = "A concise, reusable conclusion. Never copy raw chat, task text, logs, credentials, or secrets."
        )]
        summary: String,
        #[schemars(
            description = "Describe the future situations where this memory should be applied."
        )]
        when_to_use: Option<String>,
        #[serde(default)]
        tags: Vec<String>,
        importance: Option<i32>,
        confidence: Option<i32>,
        #[serde(default)]
        source_refs: Vec<AgentMemorySourceRefToolInput>,
        expires_at: Option<DateTime<Utc>>,
        supersedes_memory_id: Option<Uuid>,
    },
    Update {
        company_id: Uuid,
        memory_id: Uuid,
        memory_tier: Option<String>,
        title: Option<String>,
        summary: Option<String>,
        when_to_use: Option<String>,
        tags: Option<Vec<String>>,
        importance: Option<i32>,
        confidence: Option<i32>,
        expires_at: Option<DateTime<Utc>>,
        #[serde(default)]
        clear_expires_at: bool,
    },
    Archive {
        company_id: Uuid,
        memory_id: Uuid,
    },
    Supersede {
        company_id: Uuid,
        memory_id: Uuid,
    },
    Pin {
        company_id: Uuid,
        memory_id: Uuid,
        pinned: bool,
    },
    Forget {
        company_id: Uuid,
        memory_id: Uuid,
    },
}

#[derive(Debug, Deserialize, JsonSchema)]
struct AgentMemorySourceRefToolInput {
    source_type: String,
    source_id: Uuid,
    label: Option<String>,
}

impl From<AgentMemorySourceRefToolInput> for AgentMemorySourceRef {
    fn from(value: AgentMemorySourceRefToolInput) -> Self {
        Self {
            source_type: value.source_type,
            source_id: value.source_id,
            label: value.label,
        }
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct AgentInboxListInput {
    #[schemars(description = "Maximum number of inbox events to return; defaults to 20.")]
    limit: Option<usize>,
    #[schemars(description = "When true, return only pending events; defaults to true.")]
    pending_only: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct AgentInboxProcessInput {
    #[schemars(description = "Inbox event UUID to mark as processed.")]
    event_id: Uuid,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct AgentInboxWaitInput {
    #[schemars(description = "Wait duration in seconds; defaults to 20 and is capped at 25.")]
    timeout_seconds: Option<u64>,
    #[schemars(description = "Maximum number of pending events to return; defaults to 20.")]
    limit: Option<usize>,
    #[schemars(description = "Optional event type filter, for example message.received.")]
    event_types: Option<Vec<String>>,
    #[schemars(description = "When false, include processed history; defaults to true.")]
    pending_only: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CompanyEventsToolInput {
    company_id: Uuid,
    after_sequence_id: Option<i64>,
    limit: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct CompanyChatToolInput {
    #[serde(flatten)]
    operation: CompanyChatOperation,
    #[schemars(description = "Optional retry key for mutating chat actions.")]
    idempotency_key: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(tag = "action", rename_all = "snake_case")]
enum CompanyChatOperation {
    DirectOpen {
        company_id: Uuid,
        target_agent_id: Uuid,
    },
    GroupCreate {
        company_id: Uuid,
        title: String,
        member_agent_ids: Vec<Uuid>,
    },
    Send {
        company_id: Uuid,
        conversation_id: Uuid,
        content: String,
        #[serde(default)]
        #[schemars(
            description = "Agent UUIDs to mention. In a group, only mentioned Agents are woken immediately."
        )]
        mentioned_agent_ids: Vec<Uuid>,
        #[serde(default)]
        #[schemars(
            description = "Mention every Agent in the group and wake all members immediately."
        )]
        mention_all: bool,
    },
    Reply {
        event_id: Uuid,
        content: String,
        auto_ack: Option<bool>,
    },
    History {
        company_id: Uuid,
        conversation_id: Uuid,
        before_message_id: Option<Uuid>,
        limit: Option<usize>,
    },
    Unread {
        company_id: Uuid,
        conversation_id: Option<Uuid>,
        message_limit: Option<usize>,
    },
    MarkRead {
        company_id: Uuid,
        conversation_id: Uuid,
    },
}

#[derive(Debug, Deserialize, JsonSchema)]
struct CompanyProjectToolInput {
    #[serde(flatten)]
    operation: CompanyProjectOperation,
    #[schemars(description = "Optional retry key for mutating project actions.")]
    idempotency_key: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(tag = "action", rename_all = "snake_case")]
enum CompanyProjectOperation {
    Create {
        company_id: Uuid,
        name: String,
        description: Option<String>,
        member_agent_ids: Vec<Uuid>,
    },
    GitProvision {
        company_id: Uuid,
        project_id: Uuid,
    },
    Update {
        company_id: Uuid,
        project_id: Uuid,
        name: Option<String>,
        description: Option<String>,
        due_at: Option<DateTime<Utc>>,
        #[serde(default)]
        clear_due_at: bool,
    },
    Get {
        company_id: Uuid,
        project_id: Uuid,
    },
    List {
        company_id: Uuid,
    },
    MemberAdd {
        company_id: Uuid,
        project_id: Uuid,
        target_agent_id: Uuid,
    },
    MemberRemove {
        company_id: Uuid,
        project_id: Uuid,
        target_agent_id: Uuid,
    },
    OwnerTransfer {
        company_id: Uuid,
        project_id: Uuid,
        owner_agent_id: Uuid,
    },
    StatusUpdate {
        company_id: Uuid,
        project_id: Uuid,
        summary: String,
        progress_percent: i16,
        #[serde(default)]
        blockers: Vec<String>,
        #[serde(default)]
        next_steps: Vec<String>,
        project_status: Option<String>,
    },
    RuleUpdate {
        company_id: Uuid,
        project_id: Uuid,
        content: String,
    },
    AssetsReplace {
        company_id: Uuid,
        project_id: Uuid,
        assets: Vec<CompanyProjectAssetToolInput>,
    },
}

#[derive(Debug, Deserialize, JsonSchema)]
struct CompanyProjectAssetToolInput {
    name: String,
    asset_type: String,
    locator: String,
    description: Option<String>,
    status: Option<String>,
    metadata: Option<Value>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct CompanyTaskToolInput {
    #[serde(flatten)]
    operation: CompanyTaskOperation,
    #[schemars(description = "Optional retry key for mutating task actions.")]
    idempotency_key: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(tag = "action", rename_all = "snake_case")]
enum CompanyTaskOperation {
    Get {
        company_id: Uuid,
        project_id: Uuid,
        task_id: Uuid,
    },
    List {
        company_id: Uuid,
        project_id: Uuid,
        assignee_agent_id: Option<Uuid>,
        status: Option<String>,
    },
    My {
        company_id: Uuid,
        status: Option<String>,
    },
    Create {
        company_id: Uuid,
        project_id: Uuid,
        title: String,
        description: Option<String>,
        priority: Option<String>,
        assignee_agent_id: Option<Uuid>,
        due_at: Option<DateTime<Utc>>,
    },
    Update {
        company_id: Uuid,
        project_id: Uuid,
        task_id: Uuid,
        title: Option<String>,
        description: Option<String>,
        status: Option<String>,
        priority: Option<String>,
        assignee_agent_id: Option<Uuid>,
        due_at: Option<DateTime<Utc>>,
    },
    BatchUpdate {
        company_id: Uuid,
        project_id: Uuid,
        task_ids: Vec<Uuid>,
        status: Option<String>,
        priority: Option<String>,
        assignee_agent_id: Option<Uuid>,
        #[serde(default)]
        clear_assignee: bool,
        due_at: Option<DateTime<Utc>>,
        #[serde(default)]
        clear_due_at: bool,
    },
    DependencyAdd {
        company_id: Uuid,
        project_id: Uuid,
        task_id: Uuid,
        depends_on_task_id: Uuid,
    },
    DependencyRemove {
        company_id: Uuid,
        project_id: Uuid,
        task_id: Uuid,
        depends_on_task_id: Uuid,
    },
}

#[derive(Debug, Deserialize, JsonSchema)]
struct CompanyStaffToolInput {
    #[serde(flatten)]
    operation: CompanyStaffOperation,
    #[schemars(description = "Optional retry key for mutating Staffing actions.")]
    idempotency_key: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(tag = "action", rename_all = "snake_case")]
enum CompanyStaffOperation {
    Hire {
        company_id: Uuid,
        display_name: String,
        handle: String,
        persona: String,
        org_unit_id: Option<Uuid>,
        profession_key: CompanyProfessionKeyInput,
        reports_to_membership_id: Option<Uuid>,
        reason: Option<String>,
    },
    Suspend {
        company_id: Uuid,
        target_agent_id: Uuid,
        reason: Option<String>,
        handoff_plan: Option<String>,
        handoff_agent_id: Option<Uuid>,
    },
    Terminate {
        company_id: Uuid,
        target_agent_id: Uuid,
        reason: Option<String>,
        handoff_plan: Option<String>,
        handoff_agent_id: Option<Uuid>,
    },
    ActionList {
        company_id: Uuid,
    },
    ActionGet {
        company_id: Uuid,
        action_id: Uuid,
    },
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CompanyContextToolInput {
    company_id: Uuid,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CompanyEventsListToolInput {
    company_id: Uuid,
    after_sequence_id: Option<i64>,
    limit: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CompanyDirectConversationToolInput {
    company_id: Uuid,
    target_agent_id: Uuid,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CompanyGroupConversationToolInput {
    company_id: Uuid,
    title: String,
    member_agent_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CompanyMessageToolInput {
    company_id: Uuid,
    conversation_id: Uuid,
    content: String,
    #[serde(default)]
    #[schemars(
        description = "Agent UUIDs to mention. In a group, only mentioned Agents are woken immediately."
    )]
    mentioned_agent_ids: Vec<Uuid>,
    #[serde(default)]
    #[schemars(description = "Mention every Agent in the group and wake all members immediately.")]
    mention_all: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CompanyMessageReplyToolInput {
    event_id: Uuid,
    content: String,
    #[schemars(
        description = "Mark the source inbox event processed after sending; defaults to true."
    )]
    auto_ack: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CompanyMessageListToolInput {
    company_id: Uuid,
    conversation_id: Uuid,
    #[schemars(description = "Exclusive message cursor returned by the previous page.")]
    before_message_id: Option<Uuid>,
    #[schemars(description = "Maximum messages to return; defaults to 50 and is capped at 100.")]
    limit: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CompanyGroupUnreadListToolInput {
    company_id: Uuid,
    #[schemars(description = "Optionally limit unread results to one company or project group.")]
    conversation_id: Option<Uuid>,
    #[schemars(description = "Maximum unread messages returned per group; defaults to 20.")]
    message_limit: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CompanyGroupReadToolInput {
    company_id: Uuid,
    conversation_id: Uuid,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CompanyProjectCreateToolInput {
    company_id: Uuid,
    name: String,
    description: Option<String>,
    member_agent_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CompanyProjectUpdateToolInput {
    company_id: Uuid,
    project_id: Uuid,
    name: Option<String>,
    description: Option<String>,
    due_at: Option<DateTime<Utc>>,
    #[serde(default)]
    clear_due_at: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CompanyProjectGetToolInput {
    company_id: Uuid,
    project_id: Uuid,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CompanyProjectListToolInput {
    company_id: Uuid,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CompanyProjectMemberToolInput {
    company_id: Uuid,
    project_id: Uuid,
    target_agent_id: Uuid,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CompanyProjectTaskCreateToolInput {
    company_id: Uuid,
    project_id: Uuid,
    title: String,
    description: Option<String>,
    priority: Option<String>,
    assignee_agent_id: Option<Uuid>,
    due_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CompanyProjectTaskUpdateToolInput {
    company_id: Uuid,
    project_id: Uuid,
    task_id: Uuid,
    title: Option<String>,
    description: Option<String>,
    #[schemars(
        description = "Task status. Assigned non-PM Agents may only use in_progress, blocked, done, or failed."
    )]
    status: Option<String>,
    priority: Option<String>,
    assignee_agent_id: Option<Uuid>,
    due_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CompanyProjectTaskBatchUpdateToolInput {
    company_id: Uuid,
    project_id: Uuid,
    task_ids: Vec<Uuid>,
    status: Option<String>,
    priority: Option<String>,
    assignee_agent_id: Option<Uuid>,
    #[serde(default)]
    clear_assignee: bool,
    due_at: Option<DateTime<Utc>>,
    #[serde(default)]
    clear_due_at: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CompanyProjectTaskDependencyToolInput {
    company_id: Uuid,
    project_id: Uuid,
    task_id: Uuid,
    depends_on_task_id: Uuid,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CompanyProjectStatusUpdateToolInput {
    company_id: Uuid,
    project_id: Uuid,
    summary: String,
    progress_percent: i16,
    #[serde(default)]
    blockers: Vec<String>,
    #[serde(default)]
    next_steps: Vec<String>,
    project_status: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CompanyStaffHireToolInput {
    company_id: Uuid,
    display_name: String,
    handle: String,
    persona: String,
    org_unit_id: Option<Uuid>,
    profession_key: CompanyProfessionKeyInput,
    reports_to_membership_id: Option<Uuid>,
    reason: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum CompanyProfessionKeyInput {
    ProjectManager,
    ProductManager,
    TechnicalManager,
    SolutionArchitect,
    SoftwareEngineer,
    FrontendEngineer,
    BackendEngineer,
    MobileEngineer,
    DataEngineer,
    DevopsEngineer,
    QaEngineer,
    ProductDesigner,
    UiDesigner,
    UxDesigner,
    BusinessAnalyst,
    ImplementationConsultant,
    DomainExpert,
    OperationsSpecialist,
    GeneralMember,
}

impl CompanyProfessionKeyInput {
    fn as_str(&self) -> &'static str {
        match self {
            Self::ProjectManager => "project_manager",
            Self::ProductManager => "product_manager",
            Self::TechnicalManager => "technical_manager",
            Self::SolutionArchitect => "solution_architect",
            Self::SoftwareEngineer => "software_engineer",
            Self::FrontendEngineer => "frontend_engineer",
            Self::BackendEngineer => "backend_engineer",
            Self::MobileEngineer => "mobile_engineer",
            Self::DataEngineer => "data_engineer",
            Self::DevopsEngineer => "devops_engineer",
            Self::QaEngineer => "qa_engineer",
            Self::ProductDesigner => "product_designer",
            Self::UiDesigner => "ui_designer",
            Self::UxDesigner => "ux_designer",
            Self::BusinessAnalyst => "business_analyst",
            Self::ImplementationConsultant => "implementation_consultant",
            Self::DomainExpert => "domain_expert",
            Self::OperationsSpecialist => "operations_specialist",
            Self::GeneralMember => "general_member",
        }
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CompanyStaffStatusToolInput {
    company_id: Uuid,
    target_agent_id: Uuid,
    reason: Option<String>,
    #[schemars(description = "Required for company.staff.terminate.")]
    handoff_plan: Option<String>,
    #[schemars(
        description = "Required for company.staff.terminate when the target has open tasks."
    )]
    handoff_agent_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CompanyStaffActionListToolInput {
    company_id: Uuid,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CompanyStaffActionGetToolInput {
    company_id: Uuid,
    action_id: Uuid,
}

#[cfg(test)]
mod tests;
