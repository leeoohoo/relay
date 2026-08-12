use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const EVENT_SUBSCRIPTION_IMMEDIATE: &str = "immediate";
pub const EVENT_SUBSCRIPTION_DIGEST: &str = "digest";
pub const EVENT_SUBSCRIPTION_ON_DEMAND: &str = "on_demand";
pub const EVENT_SUBSCRIPTION_MUTED: &str = "muted";

pub const EVENT_CATEGORY_MESSAGE: &str = "message";
pub const EVENT_CATEGORY_TASK: &str = "task";
pub const EVENT_CATEGORY_GATE: &str = "gate";
pub const EVENT_CATEGORY_BLOCKER: &str = "blocker";
pub const EVENT_CATEGORY_ENVIRONMENT: &str = "environment";
pub const EVENT_CATEGORY_REQUIREMENT: &str = "requirement";
pub const EVENT_CATEGORY_QA: &str = "qa";
pub const EVENT_CATEGORY_TECHNICAL: &str = "technical";
pub const EVENT_CATEGORY_GOVERNANCE: &str = "governance";

pub const RUNTIME_STATE_IDLE: &str = "idle";
pub const RUNTIME_STATE_TRIAGING: &str = "triaging";
pub const RUNTIME_STATE_EXECUTING: &str = "executing";
pub const RUNTIME_STATE_WAITING_DEPENDENCY: &str = "waiting_dependency";
pub const RUNTIME_STATE_WAITING_ENVIRONMENT: &str = "waiting_environment";
pub const RUNTIME_STATE_WAITING_APPROVAL: &str = "waiting_approval";
pub const RUNTIME_STATE_WAITING_HUMAN: &str = "waiting_human";
pub const RUNTIME_STATE_REPORTING: &str = "reporting";
pub const RUNTIME_STATE_RECOVERING: &str = "recovering";
pub const RUNTIME_STATE_FAILED: &str = "failed";
pub const RUNTIME_STATE_PAUSED: &str = "paused";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectMemberEventSubscription {
    pub project_id: Uuid,
    pub agent_profile_id: Uuid,
    pub event_category: String,
    pub subscription_mode: String,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectLoadWarning {
    pub code: String,
    pub severity: String,
    pub agent_profile_id: Option<Uuid>,
    pub title: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentRuntimeProjection {
    pub state: String,
    pub reason: String,
    pub session_kind: Option<String>,
    pub run_id: Option<Uuid>,
    pub intent_id: Option<Uuid>,
    pub task_id: Option<Uuid>,
    pub waiting_on_type: Option<String>,
    pub waiting_on_id: Option<Uuid>,
    pub heartbeat_at: Option<DateTime<Utc>>,
    pub stale: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectDiscussionThread {
    pub id: Uuid,
    pub project_id: Uuid,
    pub scope_type: String,
    pub subject_id: Uuid,
    pub conversation_id: Uuid,
    pub created_by_agent_id: Option<Uuid>,
    pub created_by_human_user_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}
