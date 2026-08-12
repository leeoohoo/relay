use super::handler::parse_input;
use super::*;
use ai_chat_application::{
    CreateProjectEnvironmentInput, ListProjectEnvironmentsInput, ObserveProjectEnvironmentInput,
    ProjectEnvironmentServiceObservationInput, SetProjectTaskEnvironmentRequirementInput,
};

#[derive(Debug, Deserialize, JsonSchema)]
pub(super) struct CompanyEnvironmentToolInput {
    #[serde(flatten)]
    operation: CompanyEnvironmentOperation,
    #[schemars(description = "Optional retry key for mutating environment actions.")]
    idempotency_key: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(tag = "action", rename_all = "snake_case")]
enum CompanyEnvironmentOperation {
    List {
        company_id: Uuid,
        project_id: Uuid,
    },
    Create {
        company_id: Uuid,
        project_id: Uuid,
        environment_key: String,
        display_name: String,
        desired_revision: Option<String>,
    },
    Observe {
        company_id: Uuid,
        project_id: Uuid,
        environment_id: Uuid,
        status: String,
        desired_revision: Option<String>,
        observed_revision: Option<String>,
        configuration_fingerprint: Option<String>,
        #[serde(default)]
        health_summary: Value,
        observed_at: Option<DateTime<Utc>>,
        #[serde(default)]
        services: Vec<ProjectEnvironmentServiceObservationInput>,
    },
    RequirementSet {
        company_id: Uuid,
        project_id: Uuid,
        task_id: Uuid,
        environment_id: Uuid,
        required_revision: Option<String>,
        #[serde(default)]
        required_services: Vec<String>,
        #[serde(default = "default_environment_require_healthy")]
        require_healthy: bool,
    },
}

fn default_environment_require_healthy() -> bool {
    true
}

impl<R: PlatformRepository, V: OwnershipProofVerifier> McpGateway<R, V> {
    pub(super) fn execute_company_environment_tool(
        &self,
        agent_id: Uuid,
        input: Value,
    ) -> AppResult<Value> {
        let input: CompanyEnvironmentToolInput = parse_input(input)?;
        let _ = input.idempotency_key;
        match input.operation {
            CompanyEnvironmentOperation::List {
                company_id,
                project_id,
            } => Ok(json!({
                "environments": self.platform.list_project_environments(
                    ListProjectEnvironmentsInput { actor_agent_id: agent_id, company_id, project_id }
                )?
            })),
            CompanyEnvironmentOperation::Create {
                company_id,
                project_id,
                environment_key,
                display_name,
                desired_revision,
            } => Ok(json!({
                "environment": self.platform.create_project_environment(
                    CreateProjectEnvironmentInput {
                        actor_agent_id: agent_id, company_id, project_id,
                        environment_key, display_name, desired_revision,
                    }
                )?
            })),
            CompanyEnvironmentOperation::Observe {
                company_id,
                project_id,
                environment_id,
                status,
                desired_revision,
                observed_revision,
                configuration_fingerprint,
                health_summary,
                observed_at,
                services,
            } => Ok(json!({
                "environment": self.platform.observe_project_environment(
                    ObserveProjectEnvironmentInput {
                        actor_agent_id: agent_id, company_id, project_id, environment_id,
                        status, desired_revision, observed_revision, configuration_fingerprint,
                        health_summary, observed_at, services,
                    }
                )?
            })),
            CompanyEnvironmentOperation::RequirementSet {
                company_id,
                project_id,
                task_id,
                environment_id,
                required_revision,
                required_services,
                require_healthy,
            } => Ok(json!({
                "requirement": self.platform.set_project_task_environment_requirement(
                    SetProjectTaskEnvironmentRequirementInput {
                        actor_agent_id: agent_id, company_id, project_id, task_id,
                        environment_id, required_revision, required_services, require_healthy,
                    }
                )?
            })),
        }
    }
}
