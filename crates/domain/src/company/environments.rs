use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

pub const PROJECT_ENVIRONMENT_STATUS_UNKNOWN: &str = "unknown";
pub const PROJECT_ENVIRONMENT_STATUS_PROVISIONING: &str = "provisioning";
pub const PROJECT_ENVIRONMENT_STATUS_READY: &str = "ready";
pub const PROJECT_ENVIRONMENT_STATUS_DEGRADED: &str = "degraded";
pub const PROJECT_ENVIRONMENT_STATUS_OFFLINE: &str = "offline";

pub const PROJECT_ENVIRONMENT_SERVICE_HEALTH_UNKNOWN: &str = "unknown";
pub const PROJECT_ENVIRONMENT_SERVICE_HEALTH_HEALTHY: &str = "healthy";
pub const PROJECT_ENVIRONMENT_SERVICE_HEALTH_UNHEALTHY: &str = "unhealthy";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectEnvironment {
    pub id: Uuid,
    pub project_id: Uuid,
    pub environment_key: String,
    pub display_name: String,
    pub status: String,
    pub desired_revision: Option<String>,
    pub observed_revision: Option<String>,
    pub configuration_fingerprint: Option<String>,
    pub health_summary: Value,
    pub last_observed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectEnvironmentService {
    pub id: Uuid,
    pub environment_id: Uuid,
    pub service_key: String,
    pub desired_revision: Option<String>,
    pub observed_revision: Option<String>,
    pub image_digest: Option<String>,
    pub configuration_fingerprint: Option<String>,
    pub health_status: String,
    pub health_details: Value,
    pub observed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectTaskEnvironmentRequirement {
    pub task_id: Uuid,
    pub environment_id: Uuid,
    pub required_revision: Option<String>,
    pub required_services: Vec<String>,
    pub require_healthy: bool,
    pub created_at: DateTime<Utc>,
}

pub fn project_environment_requirement_satisfied(
    requirement: &ProjectTaskEnvironmentRequirement,
    environment: &ProjectEnvironment,
    services: &[ProjectEnvironmentService],
) -> bool {
    if requirement.require_healthy && environment.status != PROJECT_ENVIRONMENT_STATUS_READY {
        return false;
    }
    if requirement
        .required_revision
        .as_ref()
        .is_some_and(|required| environment.observed_revision.as_ref() != Some(required))
    {
        return false;
    }
    requirement.required_services.iter().all(|service_key| {
        services.iter().any(|service| {
            service.environment_id == environment.id
                && service.service_key == *service_key
                && (!requirement.require_healthy
                    || service.health_status == PROJECT_ENVIRONMENT_SERVICE_HEALTH_HEALTHY)
        })
    })
}
