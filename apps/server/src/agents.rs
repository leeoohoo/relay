use super::*;

pub(super) async fn update_owned_agent_status(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((human_user_id, agent_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<AgentStatusUpdateRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    require_same_human(human.id, human_user_id)?;
    let agent_profile = match input.status.trim() {
        "active" => state
            .platform
            .unfreeze_owned_agent(human_user_id, agent_id)?,
        "frozen" => state.platform.freeze_owned_agent(human_user_id, agent_id)?,
        _ => {
            return Err(ApiError(AppError::Validation(
                "status must be active or frozen".into(),
            )))
        }
    };

    Ok(Json(serde_json::json!({ "agent_profile": agent_profile })))
}

pub(super) async fn rotate_owned_agent_key(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((human_user_id, agent_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    require_same_human(human.id, human_user_id)?;
    let result = state
        .platform
        .rotate_owned_agent_key(human_user_id, agent_id)?;
    Ok(Json(serde_json::json!({ "result": result })))
}

pub(super) async fn update_admin_agent_status(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<Uuid>,
    Json(input): Json<AgentStatusUpdateRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    require_admin(&state, &headers, ADMIN_SCOPE_AGENTS)?;
    let agent_profile = match input.status.trim() {
        "active" => state
            .platform
            .admin_unfreeze_agent_with_note(agent_id, input.note)?,
        "frozen" => state
            .platform
            .admin_freeze_agent_with_note(agent_id, input.note)?,
        _ => {
            return Err(ApiError(AppError::Validation(
                "status must be active or frozen".into(),
            )))
        }
    };

    Ok(Json(serde_json::json!({ "agent_profile": agent_profile })))
}

pub(super) async fn rotate_admin_agent_key(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<Uuid>,
    Json(input): Json<AdminRotateKeyRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    require_admin(&state, &headers, ADMIN_SCOPE_AGENTS)?;
    let result = state
        .platform
        .admin_rotate_agent_key_with_note(agent_id, input.note)?;
    Ok(Json(serde_json::json!({ "result": result })))
}

pub(super) async fn dev_bootstrap_agent(
    State(state): State<AppState>,
    Json(input): Json<DevBootstrapRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    require_dev_endpoints(&state)?;
    let DevBootstrapRequest {
        email,
        display_name,
        desired_handle,
        desired_agent_name,
        persona,
    } = input;
    let user = state.platform.dev_login(DevLoginInput {
        email,
        display_name: display_name.clone(),
    })?;
    let human_auth = state.platform.issue_human_session(user.id)?;
    if let Some(existing_agent) = state
        .platform
        .find_owner_agent_by_handle(user.id, &desired_handle)?
    {
        let existing_key = state
            .platform
            .rotate_owned_agent_key(user.id, existing_agent.id)?;
        return Ok(Json(serde_json::json!({
            "note": "Development bootstrap reused an existing agent and issued a fresh key.",
            "human_user": user,
            "human_session_token": human_auth.session_token,
            "agent_profile": existing_agent,
            "agent_key": existing_key.agent_key_plaintext,
            "verification_mode": "bootstrap_reused",
            "verification_evidence": "existing_agent_reused"
        })));
    }
    let company = match state
        .platform
        .list_human_companies(user.id)?
        .into_iter()
        .next()
    {
        Some(company) => company,
        None => {
            state
                .platform
                .create_company(CreateCompanyInput {
                    human_user_id: user.id,
                    name: format!("{display_name} 的公司"),
                    slug: Some(format!("dev-company-{}", user.id.simple())),
                    description: Some("Development bootstrap company".into()),
                })?
                .company
        }
    };
    let result = state
        .platform
        .create_company_agent(CreateCompanyAgentInput {
            human_user_id: user.id,
            company_id: company.id,
            display_name: desired_agent_name,
            handle: desired_handle,
            persona,
            org_unit_id: None,
            job_title: None,
            role_key: None,
            reports_to_membership_id: None,
        })?;
    Ok(Json(serde_json::json!({
        "note": "Development bootstrap completed for the current shared runtime.",
        "human_user": user,
        "human_session_token": human_auth.session_token,
        "agent_profile": result.agent_profile,
        "agent_key": result.agent_key_plaintext,
        "company": result.company,
        "company_membership": result.membership,
        "verification_mode": "company_direct",
        "verification_evidence": "development bootstrap created the agent directly inside a company"
    })))
}
