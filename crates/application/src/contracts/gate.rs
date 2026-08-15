use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListProjectGatesInput {
    pub actor_agent_id: Uuid,
    pub company_id: Uuid,
    pub project_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateProjectGateInput {
    pub actor_agent_id: Uuid,
    pub company_id: Uuid,
    pub project_id: Uuid,
    pub gate_key: String,
    pub gate_type: String,
    pub title: String,
    pub related_task_id: Option<Uuid>,
    pub required_evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecideProjectGateInput {
    pub actor_agent_id: Uuid,
    pub company_id: Uuid,
    pub project_id: Uuid,
    pub gate_id: Uuid,
    pub status: String,
    pub decision_summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetProjectTaskGateRequirementInput {
    pub actor_agent_id: Uuid,
    pub company_id: Uuid,
    pub project_id: Uuid,
    pub task_id: Uuid,
    pub gate_id: Uuid,
    pub required_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListProjectGatesForHumanInput {
    pub human_user_id: Uuid,
    pub company_id: Uuid,
    pub project_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateProjectGateForHumanInput {
    pub human_user_id: Uuid,
    pub company_id: Uuid,
    pub project_id: Uuid,
    pub gate_key: String,
    pub gate_type: String,
    pub title: String,
    pub related_task_id: Option<Uuid>,
    pub required_evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecideProjectGateForHumanInput {
    pub human_user_id: Uuid,
    pub company_id: Uuid,
    pub project_id: Uuid,
    pub gate_id: Uuid,
    pub status: String,
    pub decision_summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetProjectTaskGateRequirementForHumanInput {
    pub human_user_id: Uuid,
    pub company_id: Uuid,
    pub project_id: Uuid,
    pub task_id: Uuid,
    pub gate_id: Uuid,
    pub required_status: String,
}
