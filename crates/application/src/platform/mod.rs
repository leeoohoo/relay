use std::collections::{HashMap, HashSet};

use argon2::{
    password_hash::{PasswordHash, PasswordVerifier},
    Argon2,
};
use chrono::{DateTime, Duration, Utc};
use serde_json::{json, Value};
use uuid::Uuid;

use ai_chat_domain::agent_identity::{
    AgentActionLog, AgentActionStatus, AgentIdempotencyRecord, AgentInboxEvent,
    AgentInboxEventStatus, AgentKeyIssueLog, AgentKeyIssueType, AgentKeyRecord, AgentOwnerBinding,
    AgentProfile, AgentRegistrationRequest, AgentStatus, ChallengeStatus, HumanAccountToken,
    HumanCredential, HumanSession, HumanUser, OwnershipProofChallenge, OwnershipProofProvider,
    RegistrationStatus, SocialProofSubmission, AGENT_COLLABORATION_PREFERENCE_AVAILABLE,
    AGENT_COLLABORATION_PREFERENCE_UNAVAILABLE,
};
use ai_chat_domain::company::{
    company_profession_catalog, company_project_type_by_key, company_project_type_catalog,
    default_company_agent_permissions_for_profession, infer_company_profession,
    infer_company_project_type, AgentCodexRunActivity, AgentCodexRunToken, AgentCodexSession,
    AgentCodexTriggerConfig, AgentCodexTriggerRun, AgentExecutionIntent, AgentMemory,
    AgentRuntimeProjection, AgentStaffingAction, AgentToolApprovalRequest,
    CodexPluginCatalogSnapshot, CodexPluginOperation, Company, CompanyAgentMembership,
    CompanyCodexRunnerProfile, CompanyGovernancePolicySettings, CompanyGovernancePolicyVersion,
    CompanyHumanMember, CompanyProject, CompanyProjectAsset, CompanyProjectAssetRefreshConfig,
    CompanyProjectGitConfig, CompanyProjectMember, CompanyProjectRule, CompanyProjectStatusUpdate,
    CompanyProjectTask, CompanyProjectTaskDependency, CompanyRealtimeEvent, OrgUnit,
    ProjectDiscussionThread, ProjectEnvironment, ProjectEnvironmentService, ProjectLoadWarning,
    ProjectMemberEventSubscription, ProjectTaskEnvironmentRequirement,
    AGENT_CODEX_APPROVAL_LOCAL_TARGET_KEY, AGENT_CODEX_APPROVAL_POLICY_NEVER,
    AGENT_CODEX_APPROVAL_POLICY_ON_REQUEST, AGENT_CODEX_APPROVAL_SCOPE_KEY,
    AGENT_CODEX_APPROVAL_TARGET_KEY, AGENT_CODEX_APPROVAL_TOOL_COMMAND,
    AGENT_CODEX_APPROVAL_TOOL_FILE_CHANGE, AGENT_CODEX_APPROVAL_TOOL_PERMISSIONS,
    AGENT_CODEX_APPROVAL_TOOL_WEBSITE_ACCESS, AGENT_CODEX_RUN_STATUS_RUNNING,
    AGENT_CODEX_SANDBOX_READ_ONLY, AGENT_CODEX_SANDBOX_WORKSPACE_WRITE,
    AGENT_CODEX_SESSION_KIND_CONTROL, AGENT_CODEX_SESSION_KIND_PROJECT,
    AGENT_CODEX_SETTING_INHERIT, AGENT_CODEX_TRIGGER_STATUS_ACTIVE,
    AGENT_CODEX_TRIGGER_STATUS_ERROR, AGENT_CODEX_TRIGGER_STATUS_PAUSED,
    AGENT_CODEX_TRIGGER_TYPE_ASSET_REFRESH, AGENT_CODEX_TRIGGER_TYPE_MANUAL,
    AGENT_CODEX_TRIGGER_TYPE_MESSAGE, AGENT_CODEX_TRIGGER_TYPE_SCHEDULED,
    AGENT_CODEX_TRIGGER_TYPE_TASK, AGENT_CODEX_WAKE_REASON_TASK_STATUS_CHANGED,
    AGENT_EXECUTION_CAPABILITY_BROWSER, AGENT_EXECUTION_INTENT_STATUS_FAILED,
    AGENT_EXECUTION_INTENT_STATUS_PENDING, AGENT_EXECUTION_INTENT_STATUS_RUNNING,
    AGENT_MEMORY_INJECTION_ALWAYS, AGENT_MEMORY_INJECTION_ON_DEMAND, AGENT_MEMORY_SCOPE_AGENT,
    AGENT_MEMORY_SCOPE_CONTROL, AGENT_MEMORY_SCOPE_PROJECT, AGENT_MEMORY_SCOPE_SESSION,
    AGENT_MEMORY_STATUS_ACTIVE, AGENT_MEMORY_STATUS_ARCHIVED, AGENT_MEMORY_STATUS_DRAFT,
    AGENT_MEMORY_STATUS_SUPERSEDED, AGENT_MEMORY_TIER_LONG_TERM, AGENT_MEMORY_TIER_SHORT_TERM,
    AGENT_MEMORY_VISIBILITY_BOTH, AGENT_MEMORY_VISIBILITY_CONTROL, AGENT_MEMORY_VISIBILITY_WORKER,
    AGENT_RUNTIME_APPROVAL_ACTION_STAFF_HIRE, AGENT_RUNTIME_APPROVAL_ACTION_STAFF_SUSPEND,
    AGENT_RUNTIME_APPROVAL_ACTION_STAFF_TERMINATE, AGENT_RUNTIME_APPROVAL_ACTION_TASK_REASSIGN,
    AGENT_TOOL_APPROVAL_MODE_ALWAYS, AGENT_TOOL_APPROVAL_MODE_ALWAYS_LOCALHOST,
    AGENT_TOOL_APPROVAL_MODE_ONCE, AGENT_TOOL_APPROVAL_SOURCE_CODEX,
    AGENT_TOOL_APPROVAL_STATUS_APPROVED, AGENT_TOOL_APPROVAL_STATUS_EXECUTED,
    AGENT_TOOL_APPROVAL_STATUS_EXECUTING, AGENT_TOOL_APPROVAL_STATUS_EXPIRED,
    AGENT_TOOL_APPROVAL_STATUS_FAILED, AGENT_TOOL_APPROVAL_STATUS_PENDING,
    AGENT_TOOL_APPROVAL_STATUS_REJECTED, CODEX_PLUGIN_OPERATION_INSTALL,
    CODEX_PLUGIN_OPERATION_REFRESH, CODEX_PLUGIN_OPERATION_REMOVE,
    CODEX_PLUGIN_OPERATION_STATUS_QUEUED, COMPANY_AGENT_ROLE_MANAGER, COMPANY_AGENT_ROLE_MEMBER,
    COMPANY_GOVERNANCE_POLICY_STATUS_ACTIVE, COMPANY_PERMISSION_AGENT_COMMUNICATE,
    COMPANY_PERMISSION_PROJECT_ASSETS_MANAGE, COMPANY_PERMISSION_PROJECT_CREATE,
    COMPANY_PERMISSION_PROJECT_MANAGE, COMPANY_PERMISSION_PROJECT_RULES_MANAGE,
    COMPANY_PERMISSION_STAFF_HIRE, COMPANY_PERMISSION_STAFF_SUSPEND,
    COMPANY_PERMISSION_STAFF_TERMINATE, COMPANY_PERMISSION_TASK_ASSIGN,
    COMPANY_PERMISSION_TASK_UPDATE, COMPANY_PROFESSION_BUSINESS_ANALYST,
    COMPANY_PROFESSION_PRODUCT_MANAGER, COMPANY_PROFESSION_PROJECT_MANAGER,
    COMPANY_PROFESSION_QA_ENGINEER, COMPANY_PROFESSION_SOLUTION_ARCHITECT,
    COMPANY_PROFESSION_TECHNICAL_MANAGER, COMPANY_ROLE_ADMIN, COMPANY_ROLE_OWNER,
    EVENT_CATEGORY_BLOCKER, EVENT_CATEGORY_ENVIRONMENT, EVENT_CATEGORY_GATE,
    EVENT_CATEGORY_GOVERNANCE, EVENT_CATEGORY_MESSAGE, EVENT_CATEGORY_QA,
    EVENT_CATEGORY_REQUIREMENT, EVENT_CATEGORY_TASK, EVENT_CATEGORY_TECHNICAL,
    EVENT_SUBSCRIPTION_DIGEST, EVENT_SUBSCRIPTION_IMMEDIATE, EVENT_SUBSCRIPTION_MUTED,
    EVENT_SUBSCRIPTION_ON_DEMAND, PROJECT_MEMBER_ROLE_MEMBER, PROJECT_MEMBER_ROLE_OWNER,
    PROJECT_STATUS_ACTIVE, PROJECT_STATUS_CANCELLED, PROJECT_STATUS_COMPLETED,
    PROJECT_STATUS_PAUSED, PROJECT_TASK_STATUS_BLOCKED, PROJECT_TASK_STATUS_CANCELLED,
    PROJECT_TASK_STATUS_DONE, PROJECT_TASK_STATUS_FAILED, PROJECT_TASK_STATUS_IN_PROGRESS,
    PROJECT_TASK_STATUS_TODO, PROJECT_TYPE_SOURCE_DESCRIPTION, PROJECT_TYPE_SOURCE_FOLDER,
    PROJECT_TYPE_SOURCE_HUMAN, PROJECT_TYPE_SOURCE_SYSTEM, RUNTIME_STATE_EXECUTING,
    RUNTIME_STATE_FAILED, RUNTIME_STATE_IDLE, RUNTIME_STATE_PAUSED, RUNTIME_STATE_RECOVERING,
    RUNTIME_STATE_REPORTING, RUNTIME_STATE_TRIAGING, RUNTIME_STATE_WAITING_APPROVAL,
    STAFFING_ACTION_ACTIVATE, STAFFING_ACTION_HIRE, STAFFING_ACTION_PERMISSION_UPDATE,
    STAFFING_ACTION_PROFESSION_UPDATE, STAFFING_ACTION_REACTIVATE, STAFFING_ACTION_ROLE_UPDATE,
    STAFFING_ACTION_SUSPEND, STAFFING_ACTION_TERMINATE, STAFFING_ACTOR_AGENT, STAFFING_ACTOR_HUMAN,
    STAFFING_STATUS_COMPLETED,
};
use ai_chat_domain::social::{
    ConversationContext, ConversationPreview, ConversationType, MessageView,
    CONVERSATION_CONTEXT_BLOCKER_THREAD, CONVERSATION_CONTEXT_COMPANY_ALL,
    CONVERSATION_CONTEXT_COMPANY_DIRECT, CONVERSATION_CONTEXT_COMPANY_GROUP,
    CONVERSATION_CONTEXT_GATE_THREAD, CONVERSATION_CONTEXT_PROJECT_GROUP,
    CONVERSATION_CONTEXT_TASK_THREAD,
};
use ai_chat_shared::{hash_secret, now_utc, AppError, AppResult};

use crate::ownership_proof::{
    OwnershipProofVerifier, OwnershipVerificationInput, StubOwnershipProofVerifier,
};
use crate::validation::*;

use crate::contracts::*;
use crate::service::PlatformRepository;

mod auth;
mod chat;
mod chat_internal;
mod chat_unread;
mod codex_profiles;
mod codex_runtime;
mod company;
mod company_paging;
mod control_snapshot;
mod discussion_threads;
mod environments;
mod event_routing;
mod execution;
mod gates;
mod memory;
mod ownership;
mod project;
mod project_managed;
mod registration;
mod role_subscriptions;
mod runtime_projection;
mod security;
mod staffing_create;
mod staffing_status;
mod task_access;
mod tasks;

#[derive(Clone)]
pub struct PlatformApp<
    R: PlatformRepository,
    V: OwnershipProofVerifier = StubOwnershipProofVerifier,
> {
    pub(crate) repo: R,
    verifier: V,
}

struct MessageDeliveryPolicy<'a> {
    project_id: Option<Uuid>,
    mentioned_agent_ids: &'a [Uuid],
    mention_all: bool,
    wake_recipient_agent_ids: &'a [Uuid],
    project_owner_followup_agent_id: Option<Uuid>,
}

impl<R: PlatformRepository> PlatformApp<R, StubOwnershipProofVerifier> {
    pub fn new(repo: R) -> Self {
        Self {
            repo,
            verifier: StubOwnershipProofVerifier,
        }
    }
}

impl<R: PlatformRepository, V: OwnershipProofVerifier> PlatformApp<R, V> {
    pub fn with_verifier(repo: R, verifier: V) -> Self {
        Self { repo, verifier }
    }

    pub fn health_check(&self) -> AppResult<()> {
        self.repo.health_check()
    }
}
