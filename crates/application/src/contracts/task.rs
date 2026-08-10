use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetCompanyProjectInput {
    pub actor_agent_id: Uuid,
    pub company_id: Uuid,
    pub project_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddCompanyProjectMemberInput {
    pub actor_agent_id: Uuid,
    pub company_id: Uuid,
    pub project_id: Uuid,
    pub target_agent_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoveCompanyProjectMemberInput {
    pub actor_agent_id: Uuid,
    pub company_id: Uuid,
    pub project_id: Uuid,
    pub target_agent_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCompanyProjectTaskInput {
    pub actor_agent_id: Uuid,
    pub company_id: Uuid,
    pub project_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub priority: Option<String>,
    pub assignee_agent_id: Option<Uuid>,
    pub due_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCompanyProjectTaskForHumanInput {
    pub human_user_id: Uuid,
    pub company_id: Uuid,
    pub project_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub priority: Option<String>,
    pub assignee_agent_id: Option<Uuid>,
    pub due_at: Option<chrono::DateTime<chrono::Utc>>,
    pub depends_on_task_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCompanyProjectTaskInput {
    pub actor_agent_id: Uuid,
    pub company_id: Uuid,
    pub project_id: Uuid,
    pub task_id: Uuid,
    pub title: Option<String>,
    pub description: Option<String>,
    pub status: Option<String>,
    pub priority: Option<String>,
    pub assignee_agent_id: Option<Uuid>,
    pub due_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCompanyProjectTaskForHumanInput {
    pub human_user_id: Uuid,
    pub company_id: Uuid,
    pub project_id: Uuid,
    pub task_id: Uuid,
    pub title: Option<String>,
    pub description: Option<String>,
    pub status: Option<String>,
    pub priority: Option<String>,
    pub assignee_agent_id: Option<Uuid>,
    pub clear_assignee: bool,
    pub due_at: Option<chrono::DateTime<chrono::Utc>>,
    pub clear_due_at: bool,
    pub depends_on_task_ids: Option<Vec<Uuid>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeCompanyProjectTaskDependencyInput {
    pub actor_agent_id: Uuid,
    pub company_id: Uuid,
    pub project_id: Uuid,
    pub task_id: Uuid,
    pub depends_on_task_id: Uuid,
    pub dependency_condition: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchUpdateCompanyProjectTasksInput {
    pub actor_agent_id: Uuid,
    pub company_id: Uuid,
    pub project_id: Uuid,
    pub task_ids: Vec<Uuid>,
    pub status: Option<String>,
    pub priority: Option<String>,
    pub assignee_agent_id: Option<Uuid>,
    pub clear_assignee: bool,
    pub due_at: Option<chrono::DateTime<chrono::Utc>>,
    pub clear_due_at: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCompanyProjectStatusUpdateInput {
    pub actor_agent_id: Uuid,
    pub company_id: Uuid,
    pub project_id: Uuid,
    pub summary: String,
    pub progress_percent: i16,
    pub blockers: Vec<String>,
    pub next_steps: Vec<String>,
    pub project_status: Option<String>,
}
