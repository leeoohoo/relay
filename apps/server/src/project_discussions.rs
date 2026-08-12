use super::*;

pub(super) async fn open_project_discussion_thread(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, project_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<OpenProjectDiscussionThreadRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let conversation = state.platform.open_project_discussion_thread_for_human(
        OpenProjectDiscussionThreadForHumanInput {
            human_user_id: human.id,
            company_id,
            project_id,
            scope_type: input.scope_type,
            subject_id: input.subject_id,
        },
    )?;
    Ok(Json(serde_json::json!({ "conversation": conversation })))
}
