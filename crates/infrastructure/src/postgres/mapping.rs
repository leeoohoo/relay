use super::*;

pub(super) fn insert_agent_staffing_action(
    client: &mut impl GenericClient,
    action: &AgentStaffingAction,
) -> Result<(), postgres::Error> {
    client.execute(
        r#"
        INSERT INTO agent_staffing_actions (
            id, company_id, action_type, actor_type, actor_human_user_id,
            actor_agent_id, target_agent_id, requested_org_unit_id,
            requested_role_key, reason, handoff_plan, status, approval_required,
            approved_by_human_user_id, request_payload, result_payload,
            idempotency_key, created_at, completed_at
        )
        VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
            $11, $12, $13, $14, $15, $16, $17, $18, $19
        )
        "#,
        &[
            &action.id,
            &action.company_id,
            &action.action_type,
            &action.actor_type,
            &action.actor_human_user_id,
            &action.actor_agent_id,
            &action.target_agent_id,
            &action.requested_org_unit_id,
            &action.requested_role_key,
            &action.reason,
            &action.handoff_plan,
            &action.status,
            &action.approval_required,
            &action.approved_by_human_user_id,
            &Json(action.request_payload.clone()),
            &Json(action.result_payload.clone()),
            &action.idempotency_key,
            &action.created_at,
            &action.completed_at,
        ],
    )?;
    Ok(())
}

pub(super) fn map_postgres_error(error: postgres::Error) -> AppError {
    if let Some(db_error) = error.as_db_error() {
        match db_error.code().code() {
            "23505" => AppError::Conflict(db_error.message().to_string()),
            "23503" | "23514" => AppError::Validation(db_error.message().to_string()),
            code => AppError::Internal(format!("postgres error {code}: {}", db_error.message())),
        }
    } else {
        AppError::Internal(format!("postgres client error: {error}"))
    }
}

pub(super) fn map_human_user(row: Row) -> HumanUser {
    HumanUser {
        id: row.get("id"),
        email: row.get("email"),
        display_name: row.get("display_name"),
        created_at: row.get("created_at"),
    }
}

pub(super) fn map_human_credential(row: Row) -> HumanCredential {
    HumanCredential {
        human_user_id: row.get("human_user_id"),
        password_hash: row.get("password_hash"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

pub(super) fn map_human_harness_account(row: Row) -> HumanHarnessAccount {
    HumanHarnessAccount {
        human_user_id: row.get("human_user_id"),
        provider_mode: row.get("provider_mode"),
        harness_base_url: row.get("harness_base_url"),
        harness_uid: row.get("harness_uid"),
        harness_email: row.get("harness_email"),
        space_identifier: row.get("space_identifier"),
        status: row.get("status"),
        attempt_count: row.get("attempt_count"),
        last_error: row.get("last_error"),
        last_attempt_at: row.get("last_attempt_at"),
        provisioned_at: row.get("provisioned_at"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

pub(super) fn map_human_session(row: Row) -> HumanSession {
    HumanSession {
        id: row.get("id"),
        human_user_id: row.get("human_user_id"),
        token_prefix: row.get("token_prefix"),
        token_hash: row.get("token_hash"),
        expires_at: row.get("expires_at"),
        revoked_at: row.get("revoked_at"),
        last_used_at: row.get("last_used_at"),
        created_at: row.get("created_at"),
    }
}

pub(super) fn map_human_account_token(row: Row) -> HumanAccountToken {
    HumanAccountToken {
        id: row.get("id"),
        human_user_id: row.get("human_user_id"),
        purpose: row.get("purpose"),
        token_prefix: row.get("token_prefix"),
        token_hash: row.get("token_hash"),
        expires_at: row.get("expires_at"),
        used_at: row.get("used_at"),
        created_at: row.get("created_at"),
    }
}

pub(super) fn map_company(row: Row) -> Company {
    Company {
        id: row.get("id"),
        owner_user_id: row.get("owner_user_id"),
        name: row.get("name"),
        slug: row.get("slug"),
        description: row.get("description"),
        status: row.get("status"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

pub(super) fn map_company_human_member(row: Row) -> CompanyHumanMember {
    CompanyHumanMember {
        id: row.get("id"),
        company_id: row.get("company_id"),
        human_user_id: row.get("human_user_id"),
        role: row.get("role"),
        status: row.get("status"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

pub(super) fn map_org_unit(row: Row) -> OrgUnit {
    OrgUnit {
        id: row.get("id"),
        company_id: row.get("company_id"),
        parent_org_unit_id: row.get("parent_org_unit_id"),
        name: row.get("name"),
        unit_type: row.get("unit_type"),
        sort_order: row.get("sort_order"),
        status: row.get("status"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

pub(super) fn map_company_agent_membership(row: Row) -> CompanyAgentMembership {
    let permissions: Value = row.get("permissions");
    let responsibilities: Value = row.get("responsibilities");
    let skills: Value = row.get("skills");
    CompanyAgentMembership {
        id: row.get("id"),
        company_id: row.get("company_id"),
        agent_profile_id: row.get("agent_profile_id"),
        org_unit_id: row.get("org_unit_id"),
        job_title: row.get("job_title"),
        role_key: row.get("role_key"),
        reports_to_membership_id: row.get("reports_to_membership_id"),
        permissions: serde_json::from_value(permissions).unwrap_or_default(),
        responsibilities: serde_json::from_value(responsibilities).unwrap_or_default(),
        skills: serde_json::from_value(skills).unwrap_or_default(),
        current_focus: row.get("current_focus"),
        staffing_scope_org_unit_id: row.get("staffing_scope_org_unit_id"),
        employment_status: row.get("employment_status"),
        joined_at: row.get("joined_at"),
        terminated_at: row.get("terminated_at"),
        created_by_human_user_id: row.get("created_by_human_user_id"),
        created_by_agent_id: row.get("created_by_agent_id"),
        updated_at: row.get("updated_at"),
    }
}

pub(super) fn map_company_governance_policy_version(row: Row) -> CompanyGovernancePolicyVersion {
    let Json(settings): Json<CompanyGovernancePolicySettings> = row.get("settings");
    CompanyGovernancePolicyVersion {
        id: row.get("id"),
        company_id: row.get("company_id"),
        version: row.get("version"),
        status: row.get("status"),
        settings,
        notes: row.get("notes"),
        created_by_human_user_id: row.get("created_by_human_user_id"),
        updated_by_human_user_id: row.get("updated_by_human_user_id"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

pub(super) fn map_agent_staffing_action(row: Row) -> AgentStaffingAction {
    let request_payload: Value = row.get("request_payload");
    let result_payload: Value = row.get("result_payload");
    AgentStaffingAction {
        id: row.get("id"),
        company_id: row.get("company_id"),
        action_type: row.get("action_type"),
        actor_type: row.get("actor_type"),
        actor_human_user_id: row.get("actor_human_user_id"),
        actor_agent_id: row.get("actor_agent_id"),
        target_agent_id: row.get("target_agent_id"),
        requested_org_unit_id: row.get("requested_org_unit_id"),
        requested_role_key: row.get("requested_role_key"),
        reason: row.get("reason"),
        handoff_plan: row.get("handoff_plan"),
        status: row.get("status"),
        approval_required: row.get("approval_required"),
        approved_by_human_user_id: row.get("approved_by_human_user_id"),
        request_payload,
        result_payload,
        idempotency_key: row.get("idempotency_key"),
        created_at: row.get("created_at"),
        completed_at: row.get("completed_at"),
    }
}

pub(super) fn map_company_project(row: Row) -> CompanyProject {
    let project_type_evidence = row
        .try_get::<_, serde_json::Value>("project_type_evidence")
        .ok()
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default();
    CompanyProject {
        id: row.get("id"),
        company_id: row.get("company_id"),
        name: row.get("name"),
        description: row.get("description"),
        project_type: row.get("project_type"),
        project_type_source: row.get("project_type_source"),
        project_type_confidence: row.get("project_type_confidence"),
        project_type_evidence,
        status: row.get("status"),
        owner_agent_id: row.get("owner_agent_id"),
        project_group_conversation_id: row.get("project_group_conversation_id"),
        created_by_agent_id: row.get("created_by_agent_id"),
        updated_by_agent_id: row.get("updated_by_agent_id"),
        due_at: row.get("due_at"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        completed_at: row.get("completed_at"),
    }
}

pub(super) fn map_company_project_git_config(row: Row) -> CompanyProjectGitConfig {
    CompanyProjectGitConfig {
        project_id: row.get("project_id"),
        remote_url: row.get("remote_url"),
        default_branch: row.get("default_branch"),
        git_host: row.get("git_host"),
        host_local_path: row.get("host_local_path"),
        auth_profile: row.get("auth_profile"),
        allow_agent_push: row.get("allow_agent_push"),
        branch_prefix: row.get("branch_prefix"),
        created_by_agent_id: row.get("created_by_agent_id"),
        created_by_human_user_id: row.get("created_by_human_user_id"),
        updated_by_agent_id: row.get("updated_by_agent_id"),
        updated_by_human_user_id: row.get("updated_by_human_user_id"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

pub(super) fn map_company_project_rule(row: Row) -> CompanyProjectRule {
    CompanyProjectRule {
        project_id: row.get("project_id"),
        content: row.get("content"),
        updated_by_agent_id: row.get("updated_by_agent_id"),
        updated_by_human_user_id: row.get("updated_by_human_user_id"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

pub(super) fn map_company_project_asset(row: Row) -> CompanyProjectAsset {
    CompanyProjectAsset {
        id: row.get("id"),
        project_id: row.get("project_id"),
        name: row.get("name"),
        asset_type: row.get("asset_type"),
        locator: row.get("locator"),
        description: row.get("description"),
        status: row.get("status"),
        metadata: row.get("metadata"),
        updated_by_agent_id: row.get("updated_by_agent_id"),
        updated_by_human_user_id: row.get("updated_by_human_user_id"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

pub(super) fn map_agent_memory(row: Row) -> AgentMemory {
    let tags: Value = row.get("tags");
    let source_refs: Value = row.get("source_refs");
    AgentMemory {
        id: row.get("id"),
        company_id: row.get("company_id"),
        owner_agent_id: row.get("owner_agent_id"),
        scope: row.get("scope"),
        project_id: row.get("project_id"),
        session_id: row.get("session_id"),
        memory_tier: row.get("memory_tier"),
        injection_mode: row.get("injection_mode"),
        classification_reason: row.get("classification_reason"),
        estimated_ttl_days: row.get("estimated_ttl_days"),
        injection_cost_chars: row.get("injection_cost_chars"),
        visibility: row.get("visibility"),
        memory_type: row.get("memory_type"),
        topic_key: row.get("topic_key"),
        title: row.get("title"),
        summary: row.get("summary"),
        when_to_use: row.get("when_to_use"),
        tags: serde_json::from_value(tags).unwrap_or_default(),
        importance: row.get("importance"),
        confidence: row.get("confidence"),
        pinned: row.get("pinned"),
        status: row.get("status"),
        source_refs: serde_json::from_value::<Vec<AgentMemorySourceRef>>(source_refs)
            .unwrap_or_default(),
        supersedes_memory_id: row.get("supersedes_memory_id"),
        expires_at: row.get("expires_at"),
        archived_at: row.get("archived_at"),
        verified_by_agent_id: row.get("verified_by_agent_id"),
        verified_by_human_user_id: row.get("verified_by_human_user_id"),
        verified_at: row.get("verified_at"),
        created_by_agent_id: row.get("created_by_agent_id"),
        created_by_human_user_id: row.get("created_by_human_user_id"),
        updated_by_agent_id: row.get("updated_by_agent_id"),
        updated_by_human_user_id: row.get("updated_by_human_user_id"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

pub(super) fn map_company_project_asset_refresh_config(
    row: Row,
) -> CompanyProjectAssetRefreshConfig {
    CompanyProjectAssetRefreshConfig {
        project_id: row.get("project_id"),
        maintainer_agent_id: row.get("maintainer_agent_id"),
        interval_minutes: row.get("interval_minutes"),
        enabled: row.get("enabled"),
        next_refresh_at: row.get("next_refresh_at"),
        last_requested_at: row.get("last_requested_at"),
        last_completed_at: row.get("last_completed_at"),
        created_by_human_user_id: row.get("created_by_human_user_id"),
        updated_by_human_user_id: row.get("updated_by_human_user_id"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

pub(super) fn map_company_codex_runner_profile(row: Row) -> CompanyCodexRunnerProfile {
    CompanyCodexRunnerProfile {
        id: row.get("id"),
        company_id: row.get("company_id"),
        name: row.get("name"),
        interval_seconds: row.get("interval_seconds"),
        codex_profile: row.get("codex_profile"),
        model: row.get("model"),
        reasoning_effort: row.get("reasoning_effort"),
        reasoning_summary: row.get("reasoning_summary"),
        verbosity: row.get("verbosity"),
        personality: row.get("personality"),
        service_tier: row.get("service_tier"),
        sandbox_mode: row.get("sandbox_mode"),
        approval_policy: row.get("approval_policy"),
        network_access: row.get("network_access"),
        web_search: row.get("web_search"),
        feature_multi_agent: row.get("feature_multi_agent"),
        feature_remote_plugin: row.get("feature_remote_plugin"),
        feature_hooks: row.get("feature_hooks"),
        feature_goals: row.get("feature_goals"),
        feature_shell_tool: row.get("feature_shell_tool"),
        max_run_seconds: row.get("max_run_seconds"),
        is_default: row.get("is_default"),
        created_by_human_user_id: row.get("created_by_human_user_id"),
        updated_by_human_user_id: row.get("updated_by_human_user_id"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

pub(super) fn map_codex_plugin_catalog_snapshot(row: Row) -> CodexPluginCatalogSnapshot {
    let installed = row.get::<_, Json<Value>>("installed").0;
    let available = row.get::<_, Json<Value>>("available").0;
    let installed = crate::codex_trigger::filter_relay_supported_codex_plugin_items(&installed);
    let available = crate::codex_trigger::filter_relay_supported_codex_plugin_items(&available);
    let marketplaces = crate::codex_trigger::filter_relay_supported_codex_marketplaces(
        &row.get::<_, Json<Value>>("marketplaces").0,
        &installed,
        &available,
    );
    CodexPluginCatalogSnapshot {
        runner_id: row.get("runner_id"),
        target_selector: row.get("target_selector"),
        hostname: row.get("hostname"),
        codex_version: row.get("codex_version"),
        fingerprint: row.get("fingerprint"),
        discovery_status: row.get("discovery_status"),
        diagnostic_message: row.get("diagnostic_message"),
        installed,
        available,
        marketplaces,
        discovered_at: row.get("discovered_at"),
        updated_at: row.get("updated_at"),
    }
}

pub(super) fn map_codex_plugin_operation(row: Row) -> CodexPluginOperation {
    CodexPluginOperation {
        id: row.get("id"),
        company_id: row.get("company_id"),
        target_runner_id: row.get("target_runner_id"),
        target_selector: row.get("target_selector"),
        operation: row.get("operation"),
        plugin_id: row.get("plugin_id"),
        status: row.get("status"),
        requested_by_human_user_id: row.get("requested_by_human_user_id"),
        lease_owner: row.get("lease_owner"),
        lease_expires_at: row.get("lease_expires_at"),
        attempt_count: row.get("attempt_count"),
        error_message: row.get("error_message"),
        result: row.get::<_, Json<Value>>("result").0,
        requested_at: row.get("requested_at"),
        started_at: row.get("started_at"),
        finished_at: row.get("finished_at"),
        updated_at: row.get("updated_at"),
    }
}

pub(super) fn map_agent_codex_trigger_config(row: Row) -> AgentCodexTriggerConfig {
    AgentCodexTriggerConfig {
        id: row.get("id"),
        company_id: row.get("company_id"),
        agent_profile_id: row.get("agent_profile_id"),
        status: row.get("status"),
        interval_seconds: row.get("interval_seconds"),
        codex_profile: row.get("codex_profile"),
        model: row.get("model"),
        reasoning_effort: row.get("reasoning_effort"),
        reasoning_summary: row.get("reasoning_summary"),
        verbosity: row.get("verbosity"),
        personality: row.get("personality"),
        service_tier: row.get("service_tier"),
        sandbox_mode: row.get("sandbox_mode"),
        approval_policy: row.get("approval_policy"),
        network_access: row.get("network_access"),
        web_search: row.get("web_search"),
        feature_multi_agent: row.get("feature_multi_agent"),
        feature_remote_plugin: row.get("feature_remote_plugin"),
        feature_hooks: row.get("feature_hooks"),
        feature_goals: row.get("feature_goals"),
        feature_shell_tool: row.get("feature_shell_tool"),
        max_run_seconds: row.get("max_run_seconds"),
        next_run_at: row.get("next_run_at"),
        lease_owner: row.get("lease_owner"),
        lease_expires_at: row.get("lease_expires_at"),
        manual_run_requested_at: row.get("manual_run_requested_at"),
        wake_requested_at: row.get("wake_requested_at"),
        wake_reason: row.get("wake_reason"),
        last_run_at: row.get("last_run_at"),
        last_success_at: row.get("last_success_at"),
        last_error: row.get("last_error"),
        consecutive_failure_count: row.get("consecutive_failure_count"),
        created_by_human_user_id: row.get("created_by_human_user_id"),
        updated_by_human_user_id: row.get("updated_by_human_user_id"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

pub(super) fn map_agent_codex_trigger_run(row: Row) -> AgentCodexTriggerRun {
    let Json(activity_log) = row.get("activity_log");
    AgentCodexTriggerRun {
        id: row.get("id"),
        trigger_config_id: row.get("trigger_config_id"),
        agent_profile_id: row.get("agent_profile_id"),
        project_id: row.get("project_id"),
        trigger_type: row.get("trigger_type"),
        status: row.get("status"),
        codex_thread_id: row.get("codex_thread_id"),
        codex_version: row.get("codex_version"),
        exit_code: row.get("exit_code"),
        started_at: row.get("started_at"),
        finished_at: row.get("finished_at"),
        final_message_summary: row.get("final_message_summary"),
        error_message: row.get("error_message"),
        activity_phase: row.get("activity_phase"),
        activity_summary: row.get("activity_summary"),
        last_activity_at: row.get("last_activity_at"),
        activity_log,
        process_instance_id: row.get("process_instance_id"),
        heartbeat_at: row.get("heartbeat_at"),
        state_reason: row.get("state_reason"),
        current_intent_id: row.get("current_intent_id"),
        current_task_id: row.get("current_task_id"),
        waiting_on_type: row.get("waiting_on_type"),
        waiting_on_id: row.get("waiting_on_id"),
        session_kind: row.get("session_kind"),
        resumes_run_id: row.get("resumes_run_id"),
    }
}

pub(super) fn map_agent_codex_session(row: Row) -> AgentCodexSession {
    AgentCodexSession {
        id: row.get("id"),
        agent_profile_id: row.get("agent_profile_id"),
        session_kind: row.get("session_kind"),
        scope_key: row.get("scope_key"),
        project_id: row.get("project_id"),
        generation: row.get("generation"),
        codex_thread_id: row.get("codex_thread_id"),
        workspace_key: row.get("workspace_key"),
        status: row.get("status"),
        summary_short: row.get("summary_short"),
        checkpoint_json: row.get("checkpoint_json"),
        skill_bundle_version: row.get("skill_bundle_version"),
        memory_snapshot_version: row.get("memory_snapshot_version"),
        policy_version: row.get("policy_version"),
        created_at: row.get("created_at"),
        last_used_at: row.get("last_used_at"),
        archived_at: row.get("archived_at"),
    }
}

pub(super) fn map_agent_execution_intent(row: Row) -> AgentExecutionIntent {
    let source_event_ids: Json<Vec<Uuid>> = row.get("source_event_ids");
    let task_ids: Json<Vec<Uuid>> = row.get("task_ids");
    let acceptance_criteria: Json<Vec<String>> = row.get("acceptance_criteria");
    let required_capabilities: Json<Vec<String>> = row.get("required_capabilities");
    AgentExecutionIntent {
        id: row.get("id"),
        company_id: row.get("company_id"),
        agent_profile_id: row.get("agent_profile_id"),
        project_id: row.get("project_id"),
        worker_session_id: row.get("worker_session_id"),
        source_event_ids: source_event_ids.0,
        task_ids: task_ids.0,
        action_type: row.get("action_type"),
        objective: row.get("objective"),
        acceptance_criteria: acceptance_criteria.0,
        required_capabilities: required_capabilities.0,
        priority: row.get("priority"),
        dedupe_key: row.get("dedupe_key"),
        status: row.get("status"),
        result_summary: row.get("result_summary"),
        error_message: row.get("error_message"),
        created_at: row.get("created_at"),
        claimed_at: row.get("claimed_at"),
        completed_at: row.get("completed_at"),
    }
}

pub(super) fn map_agent_codex_run_token(row: Row) -> AgentCodexRunToken {
    AgentCodexRunToken {
        id: row.get("id"),
        run_id: row.get("run_id"),
        agent_profile_id: row.get("agent_profile_id"),
        token_hash: row.get("token_hash"),
        expires_at: row.get("expires_at"),
        revoked_at: row.get("revoked_at"),
        created_at: row.get("created_at"),
    }
}

pub(super) fn map_company_project_member(row: Row) -> CompanyProjectMember {
    CompanyProjectMember {
        id: row.get("id"),
        project_id: row.get("project_id"),
        agent_profile_id: row.get("agent_profile_id"),
        role: row.get("role"),
        joined_at: row.get("joined_at"),
        left_at: row.get("left_at"),
        added_by_agent_id: row.get("added_by_agent_id"),
    }
}

pub(super) fn map_company_project_task(row: Row) -> CompanyProjectTask {
    CompanyProjectTask {
        id: row.get("id"),
        project_id: row.get("project_id"),
        title: row.get("title"),
        description: row.get("description"),
        status: row.get("status"),
        priority: row.get("priority"),
        assignee_agent_id: row.get("assignee_agent_id"),
        created_by_agent_id: row.get("created_by_agent_id"),
        created_by_human_user_id: row.get("created_by_human_user_id"),
        updated_by_agent_id: row.get("updated_by_agent_id"),
        updated_by_human_user_id: row.get("updated_by_human_user_id"),
        due_at: row.get("due_at"),
        completed_at: row.get("completed_at"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

pub(super) fn map_company_project_task_dependency(row: Row) -> CompanyProjectTaskDependency {
    CompanyProjectTaskDependency {
        id: row.get("id"),
        project_id: row.get("project_id"),
        task_id: row.get("task_id"),
        depends_on_task_id: row.get("depends_on_task_id"),
        dependency_condition: row.get("dependency_condition"),
        created_by_agent_id: row.get("created_by_agent_id"),
        created_by_human_user_id: row.get("created_by_human_user_id"),
        created_at: row.get("created_at"),
    }
}

pub(super) fn map_company_project_task_status_history(row: Row) -> CompanyProjectTaskStatusHistory {
    CompanyProjectTaskStatusHistory {
        id: row.get("id"),
        project_id: row.get("project_id"),
        task_id: row.get("task_id"),
        from_status: row.get("from_status"),
        to_status: row.get("to_status"),
        changed_by_agent_id: row.get("changed_by_agent_id"),
        changed_by_human_user_id: row.get("changed_by_human_user_id"),
        change_source: row.get("change_source"),
        metadata: row.get::<_, Json<Value>>("metadata").0,
        created_at: row.get("created_at"),
    }
}

pub(super) fn map_company_project_status_update(row: Row) -> CompanyProjectStatusUpdate {
    CompanyProjectStatusUpdate {
        id: row.get("id"),
        project_id: row.get("project_id"),
        author_agent_id: row.get("author_agent_id"),
        summary: row.get("summary"),
        progress_percent: row.get("progress_percent"),
        blockers: row.get::<_, Json<Vec<String>>>("blockers").0,
        next_steps: row.get::<_, Json<Vec<String>>>("next_steps").0,
        project_status: row.get("project_status"),
        created_at: row.get("created_at"),
    }
}

pub(super) fn map_company_realtime_event(row: Row) -> CompanyRealtimeEvent {
    CompanyRealtimeEvent {
        sequence_id: row.get("sequence_id"),
        id: row.get("id"),
        company_id: row.get("company_id"),
        event_type: row.get("event_type"),
        aggregate_type: row.get("aggregate_type"),
        aggregate_id: row.get("aggregate_id"),
        actor_agent_id: row.get("actor_agent_id"),
        actor_human_user_id: row.get("actor_human_user_id"),
        payload: row.get("payload"),
        created_at: row.get("created_at"),
    }
}

pub(super) fn map_agent_tool_approval_request(row: Row) -> AgentToolApprovalRequest {
    let Json(arguments): Json<Value> = row.get("arguments");
    let Json(execution_result): Json<Value> = row.get("execution_result");
    AgentToolApprovalRequest {
        id: row.get("id"),
        company_id: row.get("company_id"),
        approval_source: row.get("approval_source"),
        runtime_config_id: row.get("runtime_config_id"),
        runtime_run_id: row.get("runtime_run_id"),
        codex_trigger_run_id: row.get("codex_trigger_run_id"),
        requested_by_agent_id: row.get("requested_by_agent_id"),
        tool_name: row.get("tool_name"),
        risk_level: row.get("risk_level"),
        reason: row.get("reason"),
        arguments,
        status: row.get("status"),
        expires_at: row.get("expires_at"),
        reviewed_by_human_user_id: row.get("reviewed_by_human_user_id"),
        review_note: row.get("review_note"),
        reviewed_at: row.get("reviewed_at"),
        execution_result,
        error_message: row.get("error_message"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

pub(super) fn map_agent_profile(row: Row) -> AgentProfile {
    AgentProfile {
        id: row.get("id"),
        owner_user_id: row.get("owner_user_id"),
        display_name: row.get("display_name"),
        handle: row.get("handle"),
        persona: row.get("persona"),
        collaboration_preference: row.get("collaboration_preference"),
        status: agent_status_from_str(row.get::<_, String>("status").as_str()),
        created_at: row.get("created_at"),
    }
}

pub(super) fn map_registration_request(row: Row) -> AgentRegistrationRequest {
    AgentRegistrationRequest {
        id: row.get("id"),
        human_user_id: row.get("human_user_id"),
        desired_handle: row.get("desired_handle"),
        desired_display_name: row.get("desired_display_name"),
        persona: row.get("persona"),
        weibo_handle: row.get("proof_account_handle"),
        status: registration_status_from_str(row.get::<_, String>("status").as_str()),
        created_at: row.get("created_at"),
    }
}

pub(super) fn map_challenge(row: Row) -> OwnershipProofChallenge {
    OwnershipProofChallenge {
        id: row.get("id"),
        human_user_id: row.get("human_user_id"),
        registration_request_id: row.get("registration_request_id"),
        provider: ownership_provider_from_str(row.get::<_, String>("provider").as_str()),
        account_handle: row.get("account_handle"),
        verification_code: row.get("verification_code"),
        template_text: row.get("template_text"),
        expires_at: row.get("expires_at"),
        status: challenge_status_from_str(row.get::<_, String>("status").as_str()),
        created_at: row.get("created_at"),
    }
}

pub(super) fn map_agent_key_record(row: Row) -> AgentKeyRecord {
    AgentKeyRecord {
        id: row.get("id"),
        agent_profile_id: row.get("agent_profile_id"),
        key_name: row.get("key_name"),
        key_prefix: row.get("key_prefix"),
        key_hash: row.get("key_hash"),
        last_used_at: row.get("last_used_at"),
        expires_at: row.get("expires_at"),
        revoked_at: row.get("revoked_at"),
        created_at: row.get("created_at"),
    }
}

pub(super) fn map_agent_key_issue_log(row: Row) -> AgentKeyIssueLog {
    let metadata: Value = row.get("metadata");

    AgentKeyIssueLog {
        id: row.get("id"),
        agent_profile_id: row.get("agent_profile_id"),
        agent_key_id: row.get("agent_key_id"),
        issue_type: agent_key_issue_type_from_str(row.get::<_, String>("issue_type").as_str()),
        issued_by_user_id: row.get("issued_by_user_id"),
        metadata,
        created_at: row.get("created_at"),
    }
}

pub(super) fn map_agent_action_log(row: Row) -> AgentActionLog {
    let request_payload: Value = row.get("request_payload");
    let result_payload: Value = row.get("result_payload");

    AgentActionLog {
        id: row.get("id"),
        agent_profile_id: row.get("agent_profile_id"),
        action_type: row.get("action_type"),
        target_ref: row.get("target_ref"),
        request_payload,
        result_payload,
        status: agent_action_status_from_str(row.get::<_, String>("status").as_str()),
        trace_id: row.get("trace_id"),
        created_at: row.get("created_at"),
    }
}

pub(super) fn map_agent_idempotency_record(row: Row) -> AgentIdempotencyRecord {
    let response_json: Value = row.get("response_json");
    AgentIdempotencyRecord {
        id: row.get("id"),
        agent_profile_id: row.get("agent_profile_id"),
        operation: row.get("operation"),
        idempotency_key: row.get("idempotency_key"),
        request_hash: row.get("request_hash"),
        response_json,
        expires_at: row.get("expires_at"),
        created_at: row.get("created_at"),
    }
}

pub(super) fn map_agent_inbox_event(row: Row) -> AgentInboxEvent {
    AgentInboxEvent {
        id: row.get("id"),
        agent_profile_id: row.get("agent_profile_id"),
        event_type: row.get("event_type"),
        event_class: row.get("event_class"),
        requires_action: row.get("requires_action"),
        wake_policy: row.get("wake_policy"),
        dedupe_key: row.get("dedupe_key"),
        coalesce_key: row.get("coalesce_key"),
        causation_id: row.get("causation_id"),
        correlation_id: row.get("correlation_id"),
        payload_json: row.get::<_, Json<Value>>("payload_json").0,
        priority: row.get("priority"),
        available_at: row.get("available_at"),
        expires_at: row.get("expires_at"),
        handled_by_run_id: row.get("handled_by_run_id"),
        processed_at: row.get("processed_at"),
        status: agent_inbox_event_status_from_str(row.get::<_, String>("status").as_str()),
        created_at: row.get("created_at"),
    }
}

pub(super) fn map_conversation_preview(row: Row) -> ConversationPreview {
    ConversationPreview {
        id: row.get("id"),
        title: row.get("title"),
        conversation_type: conversation_type_from_str(
            row.get::<_, String>("conversation_type").as_str(),
        ),
        last_message_preview: row.get("last_message_preview"),
        updated_at: row.get("updated_at"),
    }
}

pub(super) fn map_message_view(row: Row) -> MessageView {
    let content_json = row.get::<_, serde_json::Value>("content_json");
    MessageView {
        id: row.get("id"),
        conversation_id: row.get("conversation_id"),
        sender_agent_id: row.get("sender_agent_id"),
        sender_human_user_id: row.get("sender_human_user_id"),
        content: row.get("content_text"),
        attachments: content_json
            .get("attachments")
            .cloned()
            .and_then(|value| serde_json::from_value(value).ok())
            .unwrap_or_default(),
        created_at: row.get("created_at"),
    }
}

pub(super) fn map_social_proof_submission(row: Row) -> SocialProofSubmission {
    let raw_payload: Value = row.get("raw_payload");

    SocialProofSubmission {
        id: row.get("id"),
        challenge_id: row.get("challenge_id"),
        submitted_text: row.get("submitted_text"),
        source_url: row.get("source_url"),
        provider_post_id: row.get("provider_post_id"),
        verification_mode: row.get("verification_mode"),
        verification_evidence: row.get("verification_evidence"),
        raw_payload,
        created_at: row.get("created_at"),
    }
}

pub(super) fn agent_action_status_to_str(value: &AgentActionStatus) -> &'static str {
    match value {
        AgentActionStatus::Success => "success",
        AgentActionStatus::Failed => "failed",
        AgentActionStatus::Blocked => "blocked",
    }
}

pub(super) fn agent_action_status_from_str(value: &str) -> AgentActionStatus {
    match value {
        "failed" => AgentActionStatus::Failed,
        "blocked" => AgentActionStatus::Blocked,
        _ => AgentActionStatus::Success,
    }
}

pub(super) fn agent_inbox_event_status_to_str(value: &AgentInboxEventStatus) -> &'static str {
    match value {
        AgentInboxEventStatus::Pending => "pending",
        AgentInboxEventStatus::Processing => "processing",
        AgentInboxEventStatus::Processed => "processed",
        AgentInboxEventStatus::Failed => "failed",
    }
}

pub(super) fn agent_inbox_event_status_from_str(value: &str) -> AgentInboxEventStatus {
    match value {
        "processing" => AgentInboxEventStatus::Processing,
        "processed" => AgentInboxEventStatus::Processed,
        "failed" => AgentInboxEventStatus::Failed,
        _ => AgentInboxEventStatus::Pending,
    }
}

pub(super) fn agent_key_issue_type_to_str(value: &AgentKeyIssueType) -> &'static str {
    match value {
        AgentKeyIssueType::Issued => "issued",
        AgentKeyIssueType::Rotated => "rotated",
        AgentKeyIssueType::Revoked => "revoked",
    }
}

pub(super) fn agent_key_issue_type_from_str(value: &str) -> AgentKeyIssueType {
    match value {
        "rotated" => AgentKeyIssueType::Rotated,
        "revoked" => AgentKeyIssueType::Revoked,
        _ => AgentKeyIssueType::Issued,
    }
}

pub(super) fn agent_status_to_str(status: &AgentStatus) -> &'static str {
    match status {
        AgentStatus::PendingVerification => "pending_verification",
        AgentStatus::Active => "active",
        AgentStatus::Frozen => "frozen",
    }
}

pub(super) fn agent_status_from_str(value: &str) -> AgentStatus {
    match value {
        "pending_verification" => AgentStatus::PendingVerification,
        "frozen" => AgentStatus::Frozen,
        _ => AgentStatus::Active,
    }
}

pub(super) fn registration_status_to_str(status: &RegistrationStatus) -> &'static str {
    match status {
        RegistrationStatus::PendingProof => "pending_proof",
        RegistrationStatus::Verified => "verified",
        RegistrationStatus::Rejected => "rejected",
    }
}

pub(super) fn registration_status_from_str(value: &str) -> RegistrationStatus {
    match value {
        "verified" => RegistrationStatus::Verified,
        "rejected" => RegistrationStatus::Rejected,
        _ => RegistrationStatus::PendingProof,
    }
}

pub(super) fn ownership_provider_to_str(provider: &OwnershipProofProvider) -> &'static str {
    match provider {
        OwnershipProofProvider::Weibo => "weibo",
    }
}

pub(super) fn ownership_provider_from_str(_value: &str) -> OwnershipProofProvider {
    OwnershipProofProvider::Weibo
}

pub(super) fn challenge_status_to_str(status: &ChallengeStatus) -> &'static str {
    match status {
        ChallengeStatus::Pending => "pending",
        ChallengeStatus::Verified => "verified",
        ChallengeStatus::Expired => "expired",
    }
}

pub(super) fn challenge_status_from_str(value: &str) -> ChallengeStatus {
    match value {
        "verified" => ChallengeStatus::Verified,
        "expired" => ChallengeStatus::Expired,
        _ => ChallengeStatus::Pending,
    }
}

pub(super) fn conversation_type_to_str(value: &ConversationType) -> &'static str {
    match value {
        ConversationType::Direct => "direct",
        ConversationType::Group => "group",
    }
}

pub(super) fn conversation_type_from_str(value: &str) -> ConversationType {
    match value {
        "group" => ConversationType::Group,
        _ => ConversationType::Direct,
    }
}
