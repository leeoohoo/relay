use super::*;

pub(super) async fn list_project_environments_for_human(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, project_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let environments = state.platform.list_project_environments_for_human(
        ListProjectEnvironmentsForHumanInput {
            human_user_id: human.id,
            company_id,
            project_id,
        },
    )?;
    let services = state
        .platform
        .list_environment_services_for_human(human.id, company_id, project_id)?;
    let requirements = state
        .platform
        .list_project_environment_requirements_for_human(human.id, company_id, project_id)?;
    Ok(Json(serde_json::json!({
        "environments": environments,
        "services": services,
        "requirements": requirements,
    })))
}

pub(super) async fn create_project_environment_for_human(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, project_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<CreateProjectEnvironmentRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let environment = state.platform.create_project_environment_for_human(
        CreateProjectEnvironmentForHumanInput {
            human_user_id: human.id,
            company_id,
            project_id,
            environment_key: input.environment_key,
            display_name: input.display_name,
            desired_revision: input.desired_revision,
        },
    )?;
    Ok(Json(serde_json::json!({ "environment": environment })))
}

pub(super) async fn observe_project_environment_for_human(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, project_id, environment_id)): Path<(Uuid, Uuid, Uuid)>,
    Json(input): Json<ObserveProjectEnvironmentRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let environment = state.platform.observe_project_environment_for_human(
        ObserveProjectEnvironmentForHumanInput {
            human_user_id: human.id,
            company_id,
            project_id,
            environment_id,
            status: input.status,
            desired_revision: input.desired_revision,
            observed_revision: input.observed_revision,
            configuration_fingerprint: input.configuration_fingerprint,
            health_summary: input.health_summary,
            observed_at: input.observed_at,
            services: input.services,
        },
    )?;
    Ok(Json(serde_json::json!({ "environment": environment })))
}

pub(super) async fn set_project_task_environment_requirement_for_human(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, project_id, task_id, environment_id)): Path<(Uuid, Uuid, Uuid, Uuid)>,
    Json(input): Json<SetProjectTaskEnvironmentRequirementRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let requirement = state
        .platform
        .set_project_task_environment_requirement_for_human(
            SetProjectTaskEnvironmentRequirementForHumanInput {
                human_user_id: human.id,
                company_id,
                project_id,
                task_id,
                environment_id,
                required_revision: input.required_revision,
                required_services: input.required_services,
                require_healthy: input.require_healthy,
            },
        )?;
    Ok(Json(serde_json::json!({ "requirement": requirement })))
}
