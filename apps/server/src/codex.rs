use super::*;

pub(super) async fn get_local_codex_models(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<LocalCodexModelsQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    authenticate_human_request(&state, &headers)?;
    let profile = query
        .codex_profile
        .as_deref()
        .map(str::trim)
        .filter(|profile| !profile.is_empty())
        .unwrap_or("default");
    if profile.len() > 64 || profile.chars().any(char::is_control) {
        return Err(AppError::Validation("invalid local Codex profile".into()).into());
    }
    let bundled = query.bundled.unwrap_or(false);
    let bytes = tokio::fs::read(&state.codex_model_catalog_path)
        .await
        .map_err(|error| {
            AppError::Internal(format!(
                "cannot read Trigger model catalog {}: {error}",
                state.codex_model_catalog_path.display()
            ))
        })?;
    let catalog: CodexModelCatalogFile = serde_json::from_slice(&bytes).map_err(|error| {
        AppError::Internal(format!("Trigger model catalog is invalid: {error}"))
    })?;
    let snapshot = catalog
        .catalogs
        .into_iter()
        .find(|snapshot| snapshot.codex_profile == profile && snapshot.bundled == bundled)
        .ok_or_else(|| {
            AppError::Validation(format!(
                "Trigger has not discovered models for Codex profile `{profile}` (bundled={bundled})"
            ))
        })?;
    Ok(Json(serde_json::json!({
        "models": snapshot.models,
        "source": snapshot.source,
        "discovered_at": snapshot.discovered_at,
    })))
}

pub(super) async fn list_company_codex_runner_profiles(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let profiles = state
        .platform
        .list_company_codex_runner_profiles_for_human(
            ListCompanyCodexRunnerProfilesForHumanInput {
                human_user_id: human.id,
                company_id,
            },
        )?;
    Ok(Json(serde_json::json!({ "profiles": profiles })))
}

pub(super) async fn get_company_codex_environments(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    state
        .platform
        .ensure_human_can_manage_company_codex(human.id, company_id)?;
    let environment = state
        .codex_control_store
        .environment_for_company(company_id)?;
    Ok(Json(serde_json::json!(environment)))
}

pub(super) async fn get_company_codex_cli_settings(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    state
        .platform
        .ensure_human_can_manage_company_codex(human.id, company_id)?;
    let settings = state.codex_control_store.company_cli_settings(company_id)?;
    Ok(Json(serde_json::json!({ "settings": settings })))
}

pub(super) async fn update_company_codex_cli_settings(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<Uuid>,
    Json(input): Json<UpdateCompanyCodexCliSettingsRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    state
        .platform
        .ensure_human_can_manage_company_codex(human.id, company_id)?;
    let settings =
        state
            .codex_control_store
            .save_company_cli_settings(CompanyCodexCliSettings {
                company_id,
                model: input.model,
                reasoning_effort: input.reasoning_effort,
                reasoning_summary: input.reasoning_summary,
                verbosity: input.verbosity,
                personality: input.personality,
                service_tier: input.service_tier,
                approval_policy: input.approval_policy,
                sandbox_mode: input.sandbox_mode,
                network_access: input.network_access,
                web_search: input.web_search,
                feature_multi_agent: input.feature_multi_agent,
                feature_remote_plugin: input.feature_remote_plugin,
                feature_hooks: input.feature_hooks,
                feature_goals: input.feature_goals,
                feature_shell_tool: input.feature_shell_tool,
                updated_at: now_utc(),
            })?;
    Ok(Json(serde_json::json!({ "settings": settings })))
}

pub(super) async fn request_codex_cli_install(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    state
        .platform
        .ensure_human_can_manage_company_codex(human.id, company_id)?;
    let runtime = state.codex_control_store.enqueue_cli_install()?;
    Ok(Json(serde_json::json!({ "runtime": runtime })))
}

pub(super) async fn request_codex_cli_update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    state
        .platform
        .ensure_human_can_manage_company_codex(human.id, company_id)?;
    let runtime = state.codex_control_store.enqueue_cli_update()?;
    Ok(Json(serde_json::json!({ "runtime": runtime })))
}

pub(super) async fn refresh_company_codex_mcp_servers(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<Uuid>,
    Json(input): Json<CodexMcpRefreshRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    state
        .platform
        .ensure_human_can_manage_company_codex(human.id, company_id)?;
    state
        .codex_control_store
        .enqueue_mcp_refresh(company_id, input.target_selector)?;
    Ok(Json(serde_json::json!({ "accepted": true })))
}

pub(super) async fn add_company_codex_mcp_server(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<Uuid>,
    Json(input): Json<CodexMcpServerInput>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    state
        .platform
        .ensure_human_can_manage_company_codex(human.id, company_id)?;
    state
        .codex_control_store
        .enqueue_mcp_add(company_id, input)?;
    Ok(Json(serde_json::json!({ "accepted": true })))
}

pub(super) async fn remove_company_codex_mcp_server(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, target_selector, server_name)): Path<(Uuid, String, String)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    state
        .platform
        .ensure_human_can_manage_company_codex(human.id, company_id)?;
    state
        .codex_control_store
        .enqueue_mcp_remove(company_id, target_selector, server_name)?;
    Ok(Json(serde_json::json!({ "accepted": true })))
}

pub(super) async fn create_company_codex_auth_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<Uuid>,
    Json(input): Json<CreateCodexAuthProfileRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    state
        .platform
        .ensure_human_can_manage_company_codex(human.id, company_id)?;
    let profile = state.codex_control_store.create_auth_profile(
        company_id,
        input.name,
        input.api_key,
        input.base_url,
    )?;
    Ok(Json(serde_json::json!({ "profile": profile })))
}

pub(super) async fn update_company_codex_auth_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, profile_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<UpdateCodexAuthProfileRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    state
        .platform
        .ensure_human_can_manage_company_codex(human.id, company_id)?;
    let profile = state.codex_control_store.update_auth_profile(
        company_id,
        profile_id,
        input.name,
        input.api_key,
        input.base_url,
    )?;
    Ok(Json(serde_json::json!({ "profile": profile })))
}

pub(super) async fn delete_company_codex_auth_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, profile_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    state
        .platform
        .ensure_human_can_manage_company_codex(human.id, company_id)?;
    let environment = state
        .codex_control_store
        .environment_for_company(company_id)?;
    let profile = environment
        .profiles
        .iter()
        .find(|profile| profile.id == profile_id)
        .ok_or_else(|| AppError::NotFound("Codex authentication profile not found".into()))?;
    let runner_profiles = state
        .platform
        .list_company_codex_runner_profiles_for_human(
            ListCompanyCodexRunnerProfilesForHumanInput {
                human_user_id: human.id,
                company_id,
            },
        )?;
    if runner_profiles
        .iter()
        .any(|runner| runner.profile.codex_profile == profile.selector)
    {
        return Err(AppError::Conflict(
            "Codex authentication profile is still used by a runner profile".into(),
        )
        .into());
    }
    let profile = state
        .codex_control_store
        .request_delete_auth_profile(company_id, profile_id)?;
    Ok(Json(serde_json::json!({ "profile": profile })))
}

pub(super) async fn list_company_codex_plugins(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<Uuid>,
    Query(query): Query<CodexPluginsQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let view = state.platform.list_company_codex_plugins_for_human(
        ListCompanyCodexPluginsForHumanInput {
            human_user_id: human.id,
            company_id,
            operation_limit: query.operation_limit.unwrap_or(50),
        },
    )?;
    Ok(Json(serde_json::json!(view)))
}

pub(super) async fn request_codex_plugin_operation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<Uuid>,
    Json(input): Json<CodexPluginOperationRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let operation = state.platform.request_codex_plugin_operation_for_human(
        RequestCodexPluginOperationForHumanInput {
            human_user_id: human.id,
            company_id,
            target_runner_id: input.target_runner_id,
            target_selector: input.target_selector,
            operation: input.operation,
            plugin_id: input.plugin_id,
        },
    )?;
    Ok(Json(serde_json::json!({ "operation": operation })))
}

pub(super) async fn create_company_codex_runner_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<Uuid>,
    Json(input): Json<UpsertCompanyCodexRunnerProfileRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    state
        .platform
        .ensure_human_can_manage_company_codex(human.id, company_id)?;
    ensure_company_managed_codex_profile(&state, company_id, &input.codex_profile)?;
    let profile = state
        .platform
        .upsert_company_codex_runner_profile_for_human(
            UpsertCompanyCodexRunnerProfileForHumanInput {
                human_user_id: human.id,
                company_id,
                profile_id: None,
                name: input.name,
                interval_seconds: input.interval_seconds,
                codex_profile: input.codex_profile,
                model: input.model,
                reasoning_effort: input.reasoning_effort,
                reasoning_summary: input.reasoning_summary,
                verbosity: input.verbosity,
                personality: input.personality,
                service_tier: input.service_tier,
                sandbox_mode: input.sandbox_mode,
                approval_policy: input.approval_policy,
                network_access: input.network_access,
                web_search: input.web_search,
                feature_multi_agent: input.feature_multi_agent,
                feature_remote_plugin: input.feature_remote_plugin,
                feature_hooks: input.feature_hooks,
                feature_goals: input.feature_goals,
                feature_shell_tool: input.feature_shell_tool,
                max_run_seconds: input.max_run_seconds,
                is_default: input.is_default,
            },
        )?;
    Ok(Json(serde_json::json!({ "profile": profile })))
}

pub(super) async fn update_company_codex_runner_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, profile_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<UpsertCompanyCodexRunnerProfileRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    state
        .platform
        .ensure_human_can_manage_company_codex(human.id, company_id)?;
    ensure_company_managed_codex_profile(&state, company_id, &input.codex_profile)?;
    let profile = state
        .platform
        .upsert_company_codex_runner_profile_for_human(
            UpsertCompanyCodexRunnerProfileForHumanInput {
                human_user_id: human.id,
                company_id,
                profile_id: Some(profile_id),
                name: input.name,
                interval_seconds: input.interval_seconds,
                codex_profile: input.codex_profile,
                model: input.model,
                reasoning_effort: input.reasoning_effort,
                reasoning_summary: input.reasoning_summary,
                verbosity: input.verbosity,
                personality: input.personality,
                service_tier: input.service_tier,
                sandbox_mode: input.sandbox_mode,
                approval_policy: input.approval_policy,
                network_access: input.network_access,
                web_search: input.web_search,
                feature_multi_agent: input.feature_multi_agent,
                feature_remote_plugin: input.feature_remote_plugin,
                feature_hooks: input.feature_hooks,
                feature_goals: input.feature_goals,
                feature_shell_tool: input.feature_shell_tool,
                max_run_seconds: input.max_run_seconds,
                is_default: input.is_default,
            },
        )?;
    Ok(Json(serde_json::json!({ "profile": profile })))
}

pub(super) async fn delete_company_codex_runner_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, profile_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    state
        .platform
        .delete_company_codex_runner_profile_for_human(
            DeleteCompanyCodexRunnerProfileForHumanInput {
                human_user_id: human.id,
                company_id,
                profile_id,
            },
        )?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) fn ensure_company_managed_codex_profile(
    state: &AppState,
    company_id: Uuid,
    selector: &str,
) -> AppResult<()> {
    if selector.starts_with("relay_")
        && state
            .codex_control_store
            .find_active_company_profile(company_id, selector)?
            .is_none()
    {
        return Err(AppError::Validation(
            "managed Codex authentication profile is unavailable or not active".into(),
        ));
    }
    Ok(())
}

pub(super) async fn list_company_approvals(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<Uuid>,
    Query(query): Query<ApprovalRequestsQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let approvals = state.platform.list_agent_tool_approvals_for_human(
        human.id,
        company_id,
        query.status.as_deref(),
        query.limit.unwrap_or(100),
    )?;
    Ok(Json(serde_json::json!({ "approvals": approvals })))
}

pub(super) async fn approve_company_approval(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, approval_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<ReviewApprovalRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let approval = state
        .platform
        .approve_agent_tool_approval(ReviewAgentToolApprovalInput {
            human_user_id: human.id,
            company_id,
            approval_request_id: approval_id,
            review_note: input.review_note,
        })?;
    Ok(Json(serde_json::json!({ "approval": approval })))
}

pub(super) async fn reject_company_approval(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, approval_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<ReviewApprovalRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let approval = state
        .platform
        .reject_agent_tool_approval(ReviewAgentToolApprovalInput {
            human_user_id: human.id,
            company_id,
            approval_request_id: approval_id,
            review_note: input.review_note,
        })?;
    Ok(Json(serde_json::json!({ "approval": approval })))
}

pub(super) async fn get_company_agent_codex_trigger(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, agent_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let trigger = state.platform.get_company_agent_codex_trigger_for_human(
        GetCompanyAgentCodexTriggerForHumanInput {
            human_user_id: human.id,
            company_id,
            agent_id,
        },
    )?;
    Ok(Json(serde_json::json!({ "trigger": trigger })))
}

pub(super) async fn upsert_company_agent_codex_trigger(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, agent_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<UpsertCompanyAgentCodexTriggerRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    state
        .platform
        .ensure_human_can_manage_company_codex(human.id, company_id)?;
    if let Some(runner_profile_id) = input.runner_profile_id {
        let runner_profiles = state
            .platform
            .list_company_codex_runner_profiles_for_human(
                ListCompanyCodexRunnerProfilesForHumanInput {
                    human_user_id: human.id,
                    company_id,
                },
            )?;
        let selector = runner_profiles
            .iter()
            .find(|view| view.profile.id == runner_profile_id)
            .map(|view| view.profile.codex_profile.as_str())
            .ok_or_else(|| AppError::NotFound("Codex runner profile not found".into()))?;
        ensure_company_managed_codex_profile(&state, company_id, selector)?;
    } else if let Some(selector) = input.codex_profile.as_deref() {
        ensure_company_managed_codex_profile(&state, company_id, selector)?;
    }
    let trigger = state
        .platform
        .upsert_company_agent_codex_trigger_for_human(
            UpsertCompanyAgentCodexTriggerForHumanInput {
                human_user_id: human.id,
                company_id,
                agent_id,
                interval_seconds: input.interval_seconds,
                codex_profile: input.codex_profile,
                model: input.model,
                reasoning_effort: input.reasoning_effort,
                reasoning_summary: input.reasoning_summary,
                verbosity: input.verbosity,
                personality: input.personality,
                service_tier: input.service_tier,
                sandbox_mode: input.sandbox_mode,
                approval_policy: input.approval_policy,
                network_access: input.network_access,
                web_search: input.web_search,
                feature_multi_agent: input.feature_multi_agent,
                feature_remote_plugin: input.feature_remote_plugin,
                feature_hooks: input.feature_hooks,
                feature_goals: input.feature_goals,
                feature_shell_tool: input.feature_shell_tool,
                max_run_seconds: input.max_run_seconds,
                runner_profile_id: input.runner_profile_id,
            },
        )?;
    Ok(Json(serde_json::json!({ "trigger": trigger })))
}

pub(super) async fn pause_company_agent_codex_trigger(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, agent_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let trigger = state.platform.pause_company_agent_codex_trigger_for_human(
        SetCompanyAgentCodexTriggerStatusForHumanInput {
            human_user_id: human.id,
            company_id,
            agent_id,
        },
    )?;
    Ok(Json(serde_json::json!({ "trigger": trigger })))
}

pub(super) async fn resume_company_agent_codex_trigger(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, agent_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let trigger = state
        .platform
        .resume_company_agent_codex_trigger_for_human(
            SetCompanyAgentCodexTriggerStatusForHumanInput {
                human_user_id: human.id,
                company_id,
                agent_id,
            },
        )?;
    Ok(Json(serde_json::json!({ "trigger": trigger })))
}

pub(super) async fn run_company_agent_codex_trigger_now(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, agent_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let trigger = state
        .platform
        .run_company_agent_codex_trigger_now_for_human(
            SetCompanyAgentCodexTriggerStatusForHumanInput {
                human_user_id: human.id,
                company_id,
                agent_id,
            },
        )?;
    Ok(Json(serde_json::json!({ "trigger": trigger })))
}

pub(super) async fn list_company_agent_codex_runs(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, agent_id)): Path<(Uuid, Uuid)>,
    Query(query): Query<CodexRunsQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let runs = state.platform.list_company_agent_codex_runs_for_human(
        ListCompanyAgentCodexRunsForHumanInput {
            human_user_id: human.id,
            company_id,
            agent_id,
            limit: query.limit.unwrap_or(20),
        },
    )?;
    Ok(Json(serde_json::json!({ "runs": runs })))
}

pub(super) async fn list_company_agent_codex_sessions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, agent_id)): Path<(Uuid, Uuid)>,
    Query(query): Query<CodexSessionsQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let sessions = state.platform.list_company_agent_codex_sessions_for_human(
        ListCompanyAgentCodexSessionsForHumanInput {
            human_user_id: human.id,
            company_id,
            agent_id,
            project_id: query.project_id,
            limit: query.limit.unwrap_or(50),
        },
    )?;
    Ok(Json(serde_json::json!({ "sessions": sessions })))
}
