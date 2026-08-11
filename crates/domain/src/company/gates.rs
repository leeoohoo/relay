use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const PROJECT_GATE_STATUS_PENDING: &str = "pending";
pub const PROJECT_GATE_STATUS_EVALUATING: &str = "evaluating";
pub const PROJECT_GATE_STATUS_PASSED: &str = "passed";
pub const PROJECT_GATE_STATUS_FAILED: &str = "failed";
pub const PROJECT_GATE_STATUS_WAIVED: &str = "waived";
pub const PROJECT_GATE_STATUS_CANCELLED: &str = "cancelled";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectGate {
    pub id: Uuid,
    pub company_id: Uuid,
    pub project_id: Uuid,
    pub gate_key: String,
    pub gate_type: String,
    pub title: String,
    pub status: String,
    pub related_task_id: Option<Uuid>,
    pub required_evidence: Vec<String>,
    pub decision_summary: String,
    pub decided_by_agent_id: Option<Uuid>,
    pub decided_by_human_user_id: Option<Uuid>,
    pub decided_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectTaskGateRequirement {
    pub task_id: Uuid,
    pub gate_id: Uuid,
    pub required_status: String,
    pub created_at: DateTime<Utc>,
}

pub fn project_gate_requirement_satisfied(
    requirement: &ProjectTaskGateRequirement,
    gate: &ProjectGate,
) -> bool {
    match requirement.required_status.as_str() {
        PROJECT_GATE_STATUS_WAIVED => gate.status == PROJECT_GATE_STATUS_WAIVED,
        _ => matches!(
            gate.status.as_str(),
            PROJECT_GATE_STATUS_PASSED | PROJECT_GATE_STATUS_WAIVED
        ),
    }
}
