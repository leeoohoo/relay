use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

pub const TASK_ATTEMPT_STATUS_QUEUED: &str = "queued";
pub const TASK_ATTEMPT_STATUS_RUNNING: &str = "running";
pub const TASK_ATTEMPT_STATUS_SUCCEEDED: &str = "succeeded";
pub const TASK_ATTEMPT_STATUS_FAILED: &str = "failed";
pub const TASK_ATTEMPT_STATUS_CANCELLED: &str = "cancelled";
pub const TASK_ATTEMPT_STATUS_INTERRUPTED: &str = "interrupted";

pub const TASK_BLOCKER_STATUS_OPEN: &str = "open";
pub const TASK_BLOCKER_STATUS_RESOLVED: &str = "resolved";
pub const TASK_BLOCKER_STATUS_WAIVED: &str = "waived";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectTaskAttempt {
    pub id: Uuid,
    pub project_id: Uuid,
    pub task_id: Uuid,
    pub agent_id: Uuid,
    pub intent_id: Option<Uuid>,
    pub attempt_number: i32,
    pub attempt_type: String,
    pub status: String,
    pub objective: String,
    pub result_summary: Option<String>,
    pub failure_category: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectTaskBlocker {
    pub id: Uuid,
    pub project_id: Uuid,
    pub task_id: Uuid,
    pub attempt_id: Option<Uuid>,
    pub blocker_type: String,
    pub status: String,
    pub summary: String,
    pub owner_agent_id: Option<Uuid>,
    pub owner_human_user_id: Option<Uuid>,
    pub resolution_condition: String,
    pub resolution_summary: Option<String>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectTaskRelation {
    pub id: Uuid,
    pub project_id: Uuid,
    pub source_task_id: Uuid,
    pub target_task_id: Uuid,
    pub relation_type: String,
    pub created_by_agent_id: Option<Uuid>,
    pub created_by_human_user_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectEvidence {
    pub id: Uuid,
    pub project_id: Uuid,
    pub task_id: Option<Uuid>,
    pub attempt_id: Option<Uuid>,
    pub gate_id: Option<Uuid>,
    pub environment_id: Option<Uuid>,
    pub evidence_type: String,
    pub title: String,
    pub summary: String,
    pub result: String,
    pub artifact_refs: Vec<Value>,
    pub metrics: Value,
    pub producer_agent_id: Option<Uuid>,
    pub producer_human_user_id: Option<Uuid>,
    pub dedupe_key: Option<String>,
    pub created_at: DateTime<Utc>,
}
