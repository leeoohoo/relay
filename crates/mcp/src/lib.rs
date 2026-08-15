use std::{collections::BTreeMap, sync::Arc, time::Duration};

use ai_chat_application::{
    AddCompanyProjectMemberInput, AgentStaffingHireInput, AgentStaffingStatusInput,
    BatchUpdateCompanyProjectTasksInput, ChangeCompanyProjectTaskDependencyInput,
    CompanyProjectAssetInput, CreateCompanyGroupConversationInput, CreateCompanyProjectInput,
    CreateCompanyProjectStatusUpdateInput, CreateCompanyProjectTaskInput, CreateProjectGateInput,
    DecideProjectGateInput, GetAgentMemoryInput, GetCompanyAgentContextInput,
    GetCompanyProjectInput, ListCompanyGroupUnreadInput, ListProjectGatesInput,
    MarkCompanyGroupReadInput, MarkInboxEventProcessedInput, OpenCompanyDirectConversationInput,
    OwnershipProofVerifier, PlatformApp, PlatformRepository, RememberAgentMemoryInput,
    RemoveCompanyProjectMemberInput, ReplaceCompanyProjectAssetsInput,
    ReplyCompanyInboxMessageInput, SearchAgentMemoriesInput, SendCompanyMessageWithMentionsInput,
    SetAgentMemoryStateInput, SetProjectTaskGateRequirementInput, TransferCompanyProjectOwnerInput,
    UpdateAgentMemoryInput, UpdateCompanyAgentWorkProfileInput, UpdateCompanyProjectInput,
    UpdateCompanyProjectRuleInput, UpdateCompanyProjectTaskInput, UpsertCompanyProjectGitInput,
};
use ai_chat_domain::agent_identity::AgentActionStatus;
use ai_chat_domain::company::{
    company_profession_by_key, infer_company_profession, AgentExecutionIntent,
    AgentMemorySourceRef, AGENT_CODEX_SESSION_KIND_PROJECT, AGENT_EXECUTION_INTENT_ACTION_EXECUTE,
    AGENT_EXECUTION_INTENT_ACTION_REPLACE_SESSION, AGENT_EXECUTION_INTENT_STATUS_PENDING,
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
use serde::{Deserialize, Deserializer, Serialize};
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
mod dispatch_environment;
mod dispatch_gate;
mod dispatch_legacy;
mod dispatch_project;
mod dispatch_staff;
mod dispatch_task;
mod gateway;
mod handler;
mod task_input;
mod tools;

use task_input::{
    normalize_company_task_input, validate_company_task_input, CompanyTaskOperation,
    CompanyTaskToolInput,
};

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
        #[serde(default)]
        scopes: Vec<String>,
        project_id: Option<Uuid>,
        session_id: Option<Uuid>,
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
        #[schemars(
            description = "Memory scope: agent, control, project, or session. Defaults to project when project_id is present, otherwise agent."
        )]
        scope: Option<String>,
        project_id: Option<Uuid>,
        session_id: Option<Uuid>,
        #[schemars(
            description = "Memory tier: long_term is injected only into sessions allowed by its scope (agent, control, project, or session); short_term is retrieved on demand through MCP search."
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
    #[schemars(
        description = "Source kind. Internal Relay objects use message, task, run, project, or human; Git evidence uses git_commit; manual is reserved for a stable caller-defined reference."
    )]
    source_type: String,
    #[schemars(
        description = "Stable source identifier. Use a canonical Relay UUID for message/task/run/project/human, a 7-64 character hexadecimal Git object ID for git_commit, or a non-empty caller-defined identifier for manual. Do not concatenate labels or prefixes with a Relay UUID."
    )]
    source_id: String,
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
struct AgentWorkSessionToolInput {
    #[serde(flatten)]
    operation: AgentWorkSessionOperation,
    #[schemars(
        description = "Optional retry key for dispatch operations. Reusing it with the same request returns the existing Intent instead of creating duplicate work."
    )]
    idempotency_key: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(tag = "action", rename_all = "snake_case")]
enum AgentWorkSessionOperation {
    List {
        company_id: Uuid,
        project_id: Option<Uuid>,
        status: Option<String>,
        limit: Option<usize>,
    },
    Get {
        company_id: Uuid,
        session_id: Uuid,
    },
    Dispatch {
        company_id: Uuid,
        project_id: Uuid,
        #[serde(default)]
        source_event_ids: Vec<Uuid>,
        #[serde(default)]
        task_ids: Vec<Uuid>,
        objective: String,
        #[serde(default)]
        acceptance_criteria: Vec<String>,
        priority: Option<String>,
        #[schemars(
            description = "Stable logical-work key. Repeating the same dispatch returns the existing Intent; use a new key when the objective, project, tasks, acceptance criteria, or priority changes."
        )]
        dedupe_key: Option<String>,
        #[serde(default)]
        #[schemars(
            description = "Create a new generation for this project's worker session instead of resuming the active Codex thread. Use only when the existing session has stale permissions or unrecoverable internal state; Relay preserves the project, branch, tasks, and latest checkpoint summary."
        )]
        replace_session: bool,
    },
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
        #[schemars(
            description = "Exclusive unread message cursor returned by the previous page. Requires conversation_id."
        )]
        after_message_id: Option<Uuid>,
        message_limit: Option<usize>,
    },
    MarkRead {
        company_id: Uuid,
        conversation_id: Uuid,
        #[serde(default)]
        #[schemars(
            description = "When true, quickly mark the conversation read only if no unread mention remains after reviewed_through_message_id."
        )]
        only_if_no_mentions: bool,
        #[schemars(
            description = "Last reviewed unread message. Earlier mentions do not block quick mark-read; later mentions do."
        )]
        reviewed_through_message_id: Option<Uuid>,
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
    #[schemars(
        description = "Asset lifecycle status. Omit it to use active; allowed values are active, missing, deprecated, and unknown."
    )]
    status: Option<CompanyProjectAssetStatusInput>,
    #[schemars(
        description = "Optional structured JSON object for asset-specific metadata. Omit it or send {} when there is no metadata."
    )]
    #[serde(default, deserialize_with = "deserialize_optional_json_object")]
    metadata: Option<BTreeMap<String, Value>>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum CompanyProjectAssetStatusInput {
    Active,
    #[serde(alias = "planned")]
    Missing,
    Deprecated,
    Unknown,
}

fn deserialize_optional_json_object<'de, D>(
    deserializer: D,
) -> Result<Option<BTreeMap<String, Value>>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;
    value.map(parse_json_object::<D::Error>).transpose()
}

fn deserialize_json_object<'de, D>(deserializer: D) -> Result<BTreeMap<String, Value>, D::Error>
where
    D: Deserializer<'de>,
{
    parse_json_object(Value::deserialize(deserializer)?)
}

fn parse_json_object<E>(value: Value) -> Result<BTreeMap<String, Value>, E>
where
    E: serde::de::Error,
{
    match value {
        Value::Object(object) => Ok(object.into_iter().collect()),
        Value::String(encoded) => {
            let decoded = serde_json::from_str::<Value>(&encoded).map_err(|error| {
                E::custom(format!(
                    "expected a JSON object; legacy encoded value is invalid JSON: {error}"
                ))
            })?;
            match decoded {
                Value::Object(object) => Ok(object.into_iter().collect()),
                _ => Err(E::custom(
                    "expected a JSON object; legacy encoded JSON must decode to an object",
                )),
            }
        }
        _ => Err(E::custom("expected a JSON object")),
    }
}

impl CompanyProjectAssetStatusInput {
    const fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Missing => "missing",
            Self::Deprecated => "deprecated",
            Self::Unknown => "unknown",
        }
    }

    fn into_string(self) -> String {
        self.as_str().to_owned()
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
struct CompanyGateToolInput {
    #[serde(flatten)]
    operation: CompanyGateOperation,
    #[schemars(description = "Optional retry key for mutating Gate actions.")]
    idempotency_key: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(tag = "action", rename_all = "snake_case")]
enum CompanyGateOperation {
    List {
        company_id: Uuid,
        project_id: Uuid,
    },
    Create {
        company_id: Uuid,
        project_id: Uuid,
        gate_key: String,
        gate_type: String,
        title: String,
        related_task_id: Option<Uuid>,
        #[serde(default)]
        required_evidence: Vec<String>,
    },
    Decide {
        company_id: Uuid,
        project_id: Uuid,
        gate_id: Uuid,
        status: String,
        decision_summary: String,
    },
    RequirementSet {
        company_id: Uuid,
        project_id: Uuid,
        task_id: Uuid,
        gate_id: Uuid,
        #[serde(default = "default_gate_required_status")]
        required_status: String,
    },
}

fn default_gate_required_status() -> String {
    "passed".into()
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
    #[schemars(
        description = "Exclusive unread message cursor returned by the previous page. Requires conversation_id."
    )]
    after_message_id: Option<Uuid>,
    #[schemars(description = "Maximum unread messages returned per group; defaults to 20.")]
    message_limit: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CompanyGroupReadToolInput {
    company_id: Uuid,
    conversation_id: Uuid,
    #[serde(default)]
    only_if_no_mentions: bool,
    reviewed_through_message_id: Option<Uuid>,
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
    dependency_condition: Option<String>,
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
    #[schemars(
        description = "Stable human-like personal name, such as 林澈 or Maya. Never use a project name, profession, role, department, or capability label as the person's display name."
    )]
    display_name: String,
    #[schemars(
        description = "Unique account handle. It may describe the work scope, but it is separate from the person's display name."
    )]
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
    #[serde(
        alias = "engineering_manager",
        alias = "tech_lead",
        alias = "technical_lead"
    )]
    TechnicalManager,
    SolutionArchitect,
    SecurityEngineer,
    SoftwareEngineer,
    FullstackEngineer,
    FrontendEngineer,
    BackendEngineer,
    MobileEngineer,
    DesktopEngineer,
    GameEngineer,
    EmbeddedIotEngineer,
    DatabaseEngineer,
    DataEngineer,
    DataAnalyst,
    MachineLearningEngineer,
    ResearchSpecialist,
    DevopsEngineer,
    #[serde(
        alias = "quality_assurance",
        alias = "quality_engineer",
        alias = "test_engineer"
    )]
    QaEngineer,
    ProductDesigner,
    UiDesigner,
    UxDesigner,
    GameDesigner,
    TechnicalWriter,
    BusinessAnalyst,
    ImplementationConsultant,
    ErpConsultant,
    WmsConsultant,
    DomainExpert,
    OperationsSpecialist,
    GrowthMarketingSpecialist,
    GeneralMember,
}

impl CompanyProfessionKeyInput {
    fn as_str(&self) -> &'static str {
        match self {
            Self::ProjectManager => "project_manager",
            Self::ProductManager => "product_manager",
            Self::TechnicalManager => "technical_manager",
            Self::SolutionArchitect => "solution_architect",
            Self::SecurityEngineer => "security_engineer",
            Self::SoftwareEngineer => "software_engineer",
            Self::FullstackEngineer => "fullstack_engineer",
            Self::FrontendEngineer => "frontend_engineer",
            Self::BackendEngineer => "backend_engineer",
            Self::MobileEngineer => "mobile_engineer",
            Self::DesktopEngineer => "desktop_engineer",
            Self::GameEngineer => "game_engineer",
            Self::EmbeddedIotEngineer => "embedded_iot_engineer",
            Self::DatabaseEngineer => "database_engineer",
            Self::DataEngineer => "data_engineer",
            Self::DataAnalyst => "data_analyst",
            Self::MachineLearningEngineer => "machine_learning_engineer",
            Self::ResearchSpecialist => "research_specialist",
            Self::DevopsEngineer => "devops_engineer",
            Self::QaEngineer => "qa_engineer",
            Self::ProductDesigner => "product_designer",
            Self::UiDesigner => "ui_designer",
            Self::UxDesigner => "ux_designer",
            Self::GameDesigner => "game_designer",
            Self::TechnicalWriter => "technical_writer",
            Self::BusinessAnalyst => "business_analyst",
            Self::ImplementationConsultant => "implementation_consultant",
            Self::ErpConsultant => "erp_consultant",
            Self::WmsConsultant => "wms_consultant",
            Self::DomainExpert => "domain_expert",
            Self::OperationsSpecialist => "operations_specialist",
            Self::GrowthMarketingSpecialist => "growth_marketing_specialist",
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
mod project_contract_tests;
#[cfg(test)]
mod task_contract_tests;
#[cfg(test)]
mod tests;
