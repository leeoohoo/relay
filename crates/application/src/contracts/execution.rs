use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use ai_chat_domain::company::{
    CompanyProjectTask, CompanyProjectTaskDependency, ProjectEnvironment,
    ProjectEnvironmentService, ProjectEvidence, ProjectGate, ProjectTaskAttempt,
    ProjectTaskBlocker, ProjectTaskEnvironmentRequirement, ProjectTaskGateRequirement,
    ProjectTaskRelation,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectTaskWaitingReason {
    pub kind: String,
    pub code: String,
    pub summary: String,
    pub related_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectTaskDependencyReadiness {
    pub dependency: CompanyProjectTaskDependency,
    pub dependency_task: Option<CompanyProjectTask>,
    pub satisfied: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectTaskGateReadiness {
    pub requirement: ProjectTaskGateRequirement,
    pub gate: Option<ProjectGate>,
    pub satisfied: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectTaskEnvironmentReadiness {
    pub requirement: ProjectTaskEnvironmentRequirement,
    pub environment: Option<ProjectEnvironment>,
    pub services: Vec<ProjectEnvironmentService>,
    pub satisfied: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectTaskReadinessView {
    pub task_id: Uuid,
    pub readiness: String,
    pub can_start: bool,
    pub waiting_reasons: Vec<ProjectTaskWaitingReason>,
    pub suggested_actions: Vec<String>,
    pub dependencies: Vec<ProjectTaskDependencyReadiness>,
    pub gate_requirements: Vec<ProjectTaskGateReadiness>,
    pub environment_requirements: Vec<ProjectTaskEnvironmentReadiness>,
    pub open_blockers: Vec<ProjectTaskBlocker>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectTaskExecutionView {
    pub readiness: ProjectTaskReadinessView,
    pub attempts: Vec<ProjectTaskAttempt>,
    pub blockers: Vec<ProjectTaskBlocker>,
    pub relations: Vec<ProjectTaskRelation>,
    pub evidence: Vec<ProjectEvidence>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartProjectTaskAttemptInput {
    pub actor_agent_id: Uuid,
    pub company_id: Uuid,
    pub project_id: Uuid,
    pub task_id: Uuid,
    pub intent_id: Option<Uuid>,
    pub attempt_type: String,
    pub objective: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinishProjectTaskAttemptInput {
    pub actor_agent_id: Uuid,
    pub company_id: Uuid,
    pub project_id: Uuid,
    pub task_id: Uuid,
    pub attempt_id: Uuid,
    pub status: String,
    pub result_summary: String,
    pub failure_category: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenProjectTaskBlockerInput {
    pub actor_agent_id: Uuid,
    pub company_id: Uuid,
    pub project_id: Uuid,
    pub task_id: Uuid,
    pub attempt_id: Option<Uuid>,
    pub blocker_type: String,
    pub summary: String,
    pub owner_agent_id: Option<Uuid>,
    pub resolution_condition: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolveProjectTaskBlockerInput {
    pub actor_agent_id: Uuid,
    pub company_id: Uuid,
    pub project_id: Uuid,
    pub task_id: Uuid,
    pub blocker_id: Uuid,
    pub status: String,
    pub resolution_summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddProjectTaskRelationInput {
    pub actor_agent_id: Uuid,
    pub company_id: Uuid,
    pub project_id: Uuid,
    pub source_task_id: Uuid,
    pub target_task_id: Uuid,
    pub relation_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoveProjectTaskRelationInput {
    pub actor_agent_id: Uuid,
    pub company_id: Uuid,
    pub project_id: Uuid,
    pub relation_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateProjectEvidenceInput {
    pub actor_agent_id: Uuid,
    pub company_id: Uuid,
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
    pub dedupe_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectTaskExecutionForHumanInput {
    pub human_user_id: Uuid,
    pub company_id: Uuid,
    pub project_id: Uuid,
    pub task_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenProjectTaskBlockerForHumanInput {
    pub human_user_id: Uuid,
    pub company_id: Uuid,
    pub project_id: Uuid,
    pub task_id: Uuid,
    pub attempt_id: Option<Uuid>,
    pub blocker_type: String,
    pub summary: String,
    pub owner_agent_id: Option<Uuid>,
    pub resolution_condition: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolveProjectTaskBlockerForHumanInput {
    pub human_user_id: Uuid,
    pub company_id: Uuid,
    pub project_id: Uuid,
    pub task_id: Uuid,
    pub blocker_id: Uuid,
    pub status: String,
    pub resolution_summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddProjectTaskRelationForHumanInput {
    pub human_user_id: Uuid,
    pub company_id: Uuid,
    pub project_id: Uuid,
    pub source_task_id: Uuid,
    pub target_task_id: Uuid,
    pub relation_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoveProjectTaskRelationForHumanInput {
    pub human_user_id: Uuid,
    pub company_id: Uuid,
    pub project_id: Uuid,
    pub relation_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateProjectEvidenceForHumanInput {
    pub human_user_id: Uuid,
    pub company_id: Uuid,
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
    pub dedupe_key: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
}
