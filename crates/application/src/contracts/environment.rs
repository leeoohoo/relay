use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProjectEnvironmentServiceObservationInput {
    pub service_key: String,
    pub desired_revision: Option<String>,
    pub observed_revision: Option<String>,
    pub image_digest: Option<String>,
    pub configuration_fingerprint: Option<String>,
    pub health_status: String,
    pub health_details: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListProjectEnvironmentsInput {
    pub actor_agent_id: Uuid,
    pub company_id: Uuid,
    pub project_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateProjectEnvironmentInput {
    pub actor_agent_id: Uuid,
    pub company_id: Uuid,
    pub project_id: Uuid,
    pub environment_key: String,
    pub display_name: String,
    pub desired_revision: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObserveProjectEnvironmentInput {
    pub actor_agent_id: Uuid,
    pub company_id: Uuid,
    pub project_id: Uuid,
    pub environment_id: Uuid,
    pub status: String,
    pub desired_revision: Option<String>,
    pub observed_revision: Option<String>,
    pub configuration_fingerprint: Option<String>,
    pub health_summary: Value,
    pub observed_at: Option<DateTime<Utc>>,
    pub services: Vec<ProjectEnvironmentServiceObservationInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetProjectTaskEnvironmentRequirementInput {
    pub actor_agent_id: Uuid,
    pub company_id: Uuid,
    pub project_id: Uuid,
    pub task_id: Uuid,
    pub environment_id: Uuid,
    pub required_revision: Option<String>,
    pub required_services: Vec<String>,
    pub require_healthy: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListProjectEnvironmentsForHumanInput {
    pub human_user_id: Uuid,
    pub company_id: Uuid,
    pub project_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateProjectEnvironmentForHumanInput {
    pub human_user_id: Uuid,
    pub company_id: Uuid,
    pub project_id: Uuid,
    pub environment_key: String,
    pub display_name: String,
    pub desired_revision: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObserveProjectEnvironmentForHumanInput {
    pub human_user_id: Uuid,
    pub company_id: Uuid,
    pub project_id: Uuid,
    pub environment_id: Uuid,
    pub status: String,
    pub desired_revision: Option<String>,
    pub observed_revision: Option<String>,
    pub configuration_fingerprint: Option<String>,
    pub health_summary: Value,
    pub observed_at: Option<DateTime<Utc>>,
    pub services: Vec<ProjectEnvironmentServiceObservationInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetProjectTaskEnvironmentRequirementForHumanInput {
    pub human_user_id: Uuid,
    pub company_id: Uuid,
    pub project_id: Uuid,
    pub task_id: Uuid,
    pub environment_id: Uuid,
    pub required_revision: Option<String>,
    pub required_services: Vec<String>,
    pub require_healthy: bool,
}
