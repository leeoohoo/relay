use super::*;
use ai_chat_application::{
    AddProjectTaskRelationForHumanInput, CreateProjectEvidenceForHumanInput,
    OpenProjectTaskBlockerForHumanInput, ProjectTaskExecutionForHumanInput,
    RemoveProjectTaskRelationForHumanInput, ResolveProjectTaskBlockerForHumanInput,
};

pub(super) async fn get_project_task_execution_for_human(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, project_id, task_id)): Path<(Uuid, Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let execution =
        state
            .platform
            .get_project_task_execution_for_human(ProjectTaskExecutionForHumanInput {
                human_user_id: human.id,
                company_id,
                project_id,
                task_id,
            })?;
    Ok(Json(serde_json::json!({ "execution": execution })))
}

pub(super) async fn open_project_task_blocker_for_human(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, project_id, task_id)): Path<(Uuid, Uuid, Uuid)>,
    Json(input): Json<OpenProjectTaskBlockerRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let blocker = state.platform.open_project_task_blocker_for_human(
        OpenProjectTaskBlockerForHumanInput {
            human_user_id: human.id,
            company_id,
            project_id,
            task_id,
            attempt_id: input.attempt_id,
            blocker_type: input.blocker_type,
            summary: input.summary,
            owner_agent_id: input.owner_agent_id,
            resolution_condition: input.resolution_condition,
        },
    )?;
    Ok(Json(serde_json::json!({ "blocker": blocker })))
}

pub(super) async fn resolve_project_task_blocker_for_human(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, project_id, task_id, blocker_id)): Path<(Uuid, Uuid, Uuid, Uuid)>,
    Json(input): Json<ResolveProjectTaskBlockerRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let blocker = state.platform.resolve_project_task_blocker_for_human(
        ResolveProjectTaskBlockerForHumanInput {
            human_user_id: human.id,
            company_id,
            project_id,
            task_id,
            blocker_id,
            status: input.status,
            resolution_summary: input.resolution_summary,
        },
    )?;
    Ok(Json(serde_json::json!({ "blocker": blocker })))
}

pub(super) async fn add_project_task_relation_for_human(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, project_id, task_id)): Path<(Uuid, Uuid, Uuid)>,
    Json(input): Json<AddProjectTaskRelationRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let relation = state.platform.add_project_task_relation_for_human(
        AddProjectTaskRelationForHumanInput {
            human_user_id: human.id,
            company_id,
            project_id,
            source_task_id: task_id,
            target_task_id: input.target_task_id,
            relation_type: input.relation_type,
        },
    )?;
    Ok(Json(serde_json::json!({ "relation": relation })))
}

pub(super) async fn remove_project_task_relation_for_human(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, project_id, relation_id)): Path<(Uuid, Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    state.platform.remove_project_task_relation_for_human(
        RemoveProjectTaskRelationForHumanInput {
            human_user_id: human.id,
            company_id,
            project_id,
            relation_id,
        },
    )?;
    Ok(Json(
        serde_json::json!({ "relation_id": relation_id, "removed": true }),
    ))
}

pub(super) async fn create_project_evidence_for_human(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, project_id, task_id)): Path<(Uuid, Uuid, Uuid)>,
    Json(input): Json<CreateProjectEvidenceRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let evidence =
        state
            .platform
            .create_project_evidence_for_human(CreateProjectEvidenceForHumanInput {
                human_user_id: human.id,
                company_id,
                project_id,
                task_id: Some(task_id),
                attempt_id: input.attempt_id,
                gate_id: input.gate_id,
                environment_id: input.environment_id,
                evidence_type: input.evidence_type,
                title: input.title,
                summary: input.summary,
                result: input.result,
                artifact_refs: input.artifact_refs,
                metrics: input.metrics,
                dedupe_key: input.dedupe_key,
                created_at: None,
            })?;
    Ok(Json(serde_json::json!({ "evidence": evidence })))
}
