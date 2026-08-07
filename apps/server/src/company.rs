use super::*;

pub(super) async fn dev_login(
    State(state): State<AppState>,
    Json(input): Json<DevLoginInput>,
) -> Result<Json<serde_json::Value>, ApiError> {
    require_dev_endpoints(&state)?;
    state
        .login_limiter
        .check(format!("dev:{}", input.email.trim().to_lowercase()))?;
    let user = state.platform.dev_login(input)?;
    let auth = state.platform.issue_human_session(user.id)?;
    Ok(Json(serde_json::json!({
        "user": auth.user,
        "session_token": auth.session_token,
        "expires_at": auth.expires_at
    })))
}

pub(super) async fn create_company(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateCompanyRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let company = state.platform.create_company(CreateCompanyInput {
        human_user_id: human.id,
        name: input.name,
        slug: input.slug,
        description: input.description,
    })?;
    Ok(Json(serde_json::json!({ "company_console": company })))
}

pub(super) async fn list_companies(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let companies = state.platform.list_human_companies(human.id)?;
    Ok(Json(serde_json::json!({ "companies": companies })))
}

pub(super) async fn get_company_console(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let company = state.platform.get_company_console(human.id, company_id)?;
    Ok(Json(serde_json::json!({ "company_console": company })))
}

pub(super) async fn list_company_memories(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<Uuid>,
    Query(query): Query<CompanyMemoriesQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let memories =
        state
            .platform
            .list_company_memories_for_human(ListCompanyMemoriesForHumanInput {
                human_user_id: human.id,
                company_id,
                owner_agent_id: query.owner_agent_id,
                project_id: query.project_id,
                memory_tier: query.memory_tier,
                status: query.status,
                query: query.query,
                limit: query.limit.unwrap_or(200),
            })?;
    Ok(Json(serde_json::json!({ "memories": memories })))
}

pub(super) async fn update_agent_memory_for_human(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, memory_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<UpdateAgentMemoryRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let memory = state
        .platform
        .update_agent_memory_for_human(UpdateAgentMemoryForHumanInput {
            human_user_id: human.id,
            company_id,
            memory_id,
            memory_tier: input.memory_tier,
            title: input.title,
            summary: input.summary,
            when_to_use: input.when_to_use,
            tags: input.tags,
            importance: input.importance,
            confidence: input.confidence,
            status: input.status,
            pinned: input.pinned,
        })?;
    Ok(Json(serde_json::json!({ "memory": memory })))
}

pub(super) async fn delete_agent_memory_for_human(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, memory_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    state
        .platform
        .delete_agent_memory_for_human(DeleteAgentMemoryForHumanInput {
            human_user_id: human.id,
            company_id,
            memory_id,
        })?;
    Ok(Json(serde_json::json!({ "deleted": true })))
}

pub(super) async fn create_company_agent(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<Uuid>,
    Json(input): Json<CreateCompanyAgentRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let profession = company_profession_by_key(&input.profession_key)
        .ok_or_else(|| ApiError::from(AppError::Validation("unsupported profession_key".into())))?;
    let result = state
        .platform
        .create_company_agent(CreateCompanyAgentInput {
            human_user_id: human.id,
            company_id,
            display_name: input.display_name,
            handle: input.handle,
            persona: input.persona,
            org_unit_id: input.org_unit_id,
            job_title: Some(profession.label),
            role_key: input.role_key,
            reports_to_membership_id: input.reports_to_membership_id,
        })?;
    Ok(Json(serde_json::json!({
        "result": {
            "company": result.company,
            "agent_profile": result.agent_profile,
            "membership": result.membership,
            "managed_identity": true
        }
    })))
}

pub(super) async fn create_org_unit(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<Uuid>,
    Json(input): Json<CreateOrgUnitRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let org_unit = state.platform.create_org_unit(CreateOrgUnitInput {
        human_user_id: human.id,
        company_id,
        parent_org_unit_id: input.parent_org_unit_id,
        name: input.name,
        unit_type: input.unit_type,
        sort_order: input.sort_order,
    })?;
    Ok(Json(serde_json::json!({ "org_unit": org_unit })))
}

pub(super) async fn update_company_agent_permissions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, agent_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<UpdateCompanyAgentPermissionsRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let membership = state.platform.update_company_agent_staffing_permissions(
        UpdateCompanyAgentPermissionsInput {
            human_user_id: human.id,
            company_id,
            agent_id,
            staffing_permissions: input.staffing_permissions,
            project_permissions: input.project_permissions,
            staffing_scope_org_unit_id: input.staffing_scope_org_unit_id,
            reason: input.reason,
        },
    )?;
    Ok(Json(serde_json::json!({ "membership": membership })))
}

pub(super) async fn update_company_agent_role(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, agent_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<UpdateCompanyAgentRoleRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let membership = state
        .platform
        .update_company_agent_role(UpdateCompanyAgentRoleInput {
            human_user_id: human.id,
            company_id,
            agent_id,
            role_key: input.role_key,
            reason: input.reason,
        })?;
    Ok(Json(serde_json::json!({ "membership": membership })))
}

pub(super) async fn update_company_agent_profession(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, agent_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<UpdateCompanyAgentProfessionRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let membership =
        state
            .platform
            .update_company_agent_profession(UpdateCompanyAgentProfessionInput {
                human_user_id: human.id,
                company_id,
                agent_id,
                profession_key: input.profession_key,
                reason: input.reason,
            })?;
    Ok(Json(serde_json::json!({ "membership": membership })))
}

pub(super) async fn get_company_governance_policy(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let policy = state
        .platform
        .get_company_governance_policy_for_human(human.id, company_id)?;
    Ok(Json(serde_json::json!({ "governance_policy": policy })))
}

pub(super) async fn publish_company_governance_policy(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<Uuid>,
    Json(input): Json<PublishCompanyGovernancePolicyRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let current = state
        .platform
        .get_company_governance_policy_for_human(human.id, company_id)?;
    let policy =
        state
            .platform
            .publish_company_governance_policy(PublishCompanyGovernancePolicyInput {
                human_user_id: human.id,
                company_id,
                settings: CompanyGovernancePolicySettings {
                    agent_staff_limit: input.agent_staff_limit,
                    delegated_agent_hiring_enabled: input.delegated_agent_hiring_enabled,
                    delegated_agent_suspension_enabled: input.delegated_agent_suspension_enabled,
                    delegated_agent_termination_enabled: input.delegated_agent_termination_enabled,
                    max_active_projects: input.max_active_projects,
                    max_project_members: input.max_project_members,
                    daily_delegated_hire_limit: input
                        .daily_delegated_hire_limit
                        .unwrap_or(current.effective_settings.daily_delegated_hire_limit),
                    daily_delegated_suspension_limit: input
                        .daily_delegated_suspension_limit
                        .unwrap_or(current.effective_settings.daily_delegated_suspension_limit),
                    daily_delegated_termination_limit: input
                        .daily_delegated_termination_limit
                        .unwrap_or(current.effective_settings.daily_delegated_termination_limit),
                    managed_workspace_root: current.effective_settings.managed_workspace_root,
                    skill_language: input
                        .skill_language
                        .unwrap_or(current.effective_settings.skill_language),
                },
                notes: input.notes,
            })?;
    Ok(Json(serde_json::json!({ "governance_policy": policy })))
}

pub(super) async fn update_company_skill_language(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<Uuid>,
    Json(input): Json<UpdateCompanySkillLanguageRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let current = state
        .platform
        .get_company_governance_policy_for_human(human.id, company_id)?;
    let skill_language = input.skill_language.trim();
    if !matches!(skill_language, "zh-CN" | "en") {
        return Err(ApiError(AppError::Validation(
            "skill_language must be zh-CN or en".into(),
        )));
    }
    if current.effective_settings.skill_language == skill_language {
        return Ok(Json(serde_json::json!({ "governance_policy": current })));
    }
    let mut settings = current.effective_settings;
    settings.skill_language = skill_language.into();
    let policy =
        state
            .platform
            .publish_company_governance_policy(PublishCompanyGovernancePolicyInput {
                human_user_id: human.id,
                company_id,
                settings,
                notes: Some(format!(
                    "Human changed Relay Skill language to {skill_language}"
                )),
            })?;
    Ok(Json(serde_json::json!({ "governance_policy": policy })))
}

pub(super) async fn get_agent_trigger_preferences(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    state
        .platform
        .get_company_governance_policy_for_human(human.id, company_id)?;
    let preferences = state
        .codex_control_store
        .agent_trigger_preferences(agent_trigger_batch_size_from_env())?;
    Ok(Json(serde_json::json!({ "preferences": preferences })))
}

pub(super) async fn update_agent_trigger_preferences(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<Uuid>,
    Json(input): Json<UpdateAgentTriggerPreferencesRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    state
        .platform
        .ensure_human_can_manage_company_codex(human.id, company_id)?;
    let preferences = state.codex_control_store.save_agent_trigger_preferences(
        input.batch_size,
        human.id,
        agent_trigger_batch_size_from_env(),
    )?;
    Ok(Json(serde_json::json!({ "preferences": preferences })))
}

pub(super) async fn update_company_workspace_settings(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<Uuid>,
    Json(input): Json<UpdateCompanyWorkspaceRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let current = state
        .platform
        .get_company_governance_policy_for_human(human.id, company_id)?;
    let managed_workspace_root = input
        .managed_workspace_root
        .filter(|value| !value.trim().is_empty())
        .map(validate_company_workspace_root)
        .transpose()?;
    if current.effective_settings.managed_workspace_root == managed_workspace_root {
        return Ok(Json(serde_json::json!({
            "governance_policy": current,
            "resolved_workspace_root": resolve_company_workspace_root(
                managed_workspace_root.as_deref(),
                company_id,
            )?
        })));
    }
    let mut settings = current.effective_settings;
    settings.managed_workspace_root = managed_workspace_root;
    let policy =
        state
            .platform
            .publish_company_governance_policy(PublishCompanyGovernancePolicyInput {
                human_user_id: human.id,
                company_id,
                settings,
                notes: Some("Human updated the Relay managed project workspace".into()),
            })?;
    Ok(Json(serde_json::json!({
        "governance_policy": policy,
        "resolved_workspace_root": resolve_company_workspace_root(
            policy.effective_settings.managed_workspace_root.as_deref(),
            company_id,
        )?
    })))
}

pub(super) async fn activate_company_agent(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, agent_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<CompanyAgentStaffingStatusRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let result =
        state
            .platform
            .activate_provisioned_company_agent(HumanCompanyStaffingStatusInput {
                human_user_id: human.id,
                company_id,
                target_agent_id: agent_id,
                reason: input.reason,
                handoff_plan: input.handoff_plan,
                handoff_agent_id: input.handoff_agent_id,
            })?;
    Ok(Json(serde_json::json!({
        "result": {
            "action": result.action,
            "agent_profile": result.agent_profile,
            "membership": result.membership,
            "managed_identity": true
        }
    })))
}

pub(super) async fn suspend_company_agent(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, agent_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<CompanyAgentStaffingStatusRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let result =
        state
            .platform
            .suspend_company_agent_as_human(HumanCompanyStaffingStatusInput {
                human_user_id: human.id,
                company_id,
                target_agent_id: agent_id,
                reason: input.reason,
                handoff_plan: input.handoff_plan,
                handoff_agent_id: input.handoff_agent_id,
            })?;
    Ok(Json(serde_json::json!({ "result": result })))
}

pub(super) async fn reactivate_company_agent(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, agent_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<CompanyAgentStaffingStatusRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let result = state
        .platform
        .reactivate_company_agent(HumanCompanyStaffingStatusInput {
            human_user_id: human.id,
            company_id,
            target_agent_id: agent_id,
            reason: input.reason,
            handoff_plan: input.handoff_plan,
            handoff_agent_id: input.handoff_agent_id,
        })?;
    Ok(Json(serde_json::json!({
        "result": {
            "action": result.action,
            "agent_profile": result.agent_profile,
            "membership": result.membership,
            "managed_identity": true
        }
    })))
}

pub(super) async fn terminate_company_agent(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, agent_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<CompanyAgentStaffingStatusRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let result =
        state
            .platform
            .terminate_company_agent_as_human(HumanCompanyStaffingStatusInput {
                human_user_id: human.id,
                company_id,
                target_agent_id: agent_id,
                reason: input.reason,
                handoff_plan: input.handoff_plan,
                handoff_agent_id: input.handoff_agent_id,
            })?;
    Ok(Json(serde_json::json!({ "result": result })))
}

pub(super) async fn list_company_staffing_actions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let actions = state
        .platform
        .list_company_staffing_actions_for_human(human.id, company_id)?;
    Ok(Json(serde_json::json!({ "staffing_actions": actions })))
}

pub(super) async fn pause_company_project(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, project_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let project =
        state
            .platform
            .pause_company_project_for_human(SetCompanyProjectPauseForHumanInput {
                human_user_id: human.id,
                company_id,
                project_id,
            })?;
    Ok(Json(serde_json::json!({ "project": project })))
}

pub(super) async fn resume_company_project(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, project_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let project =
        state
            .platform
            .resume_company_project_for_human(SetCompanyProjectPauseForHumanInput {
                human_user_id: human.id,
                company_id,
                project_id,
            })?;
    Ok(Json(serde_json::json!({ "project": project })))
}
