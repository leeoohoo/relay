use super::*;

fn seed_logged_in_company(
    repo: &MemoryPlatformRepository,
    company_id: Uuid,
    now: chrono::DateTime<chrono::Utc>,
) {
    let human_user_id = Uuid::new_v4();
    let session = HumanSession {
        id: Uuid::new_v4(),
        human_user_id,
        token_prefix: "hus_test".into(),
        token_hash: Uuid::new_v4().simple().to_string(),
        expires_at: now + chrono::Duration::hours(1),
        revoked_at: None,
        last_used_at: Some(now),
        created_at: now,
    };
    let membership = CompanyHumanMember {
        id: Uuid::new_v4(),
        company_id,
        human_user_id,
        role: "owner".into(),
        status: "active".into(),
        created_at: now,
        updated_at: now,
    };
    let mut guard = repo.inner.write().expect("memory repo lock poisoned");
    guard.human_sessions.insert(session.id, session);
    guard
        .company_human_members
        .insert((company_id, human_user_id), membership);
}

fn running_codex_run(agent_profile_id: Uuid) -> AgentCodexTriggerRun {
    AgentCodexTriggerRun {
        id: Uuid::new_v4(),
        trigger_config_id: Uuid::new_v4(),
        agent_profile_id,
        project_id: None,
        trigger_type: "scheduled".into(),
        status: AGENT_CODEX_RUN_STATUS_RUNNING.into(),
        codex_thread_id: None,
        codex_version: None,
        exit_code: None,
        started_at: now_utc(),
        finished_at: None,
        final_message_summary: None,
        error_message: None,
        activity_phase: "running".into(),
        activity_summary: Some("Codex 已开始执行".into()),
        last_activity_at: Some(now_utc()),
        activity_log: Vec::new(),
    }
}

#[test]
fn codex_trigger_allows_only_one_running_cycle_per_agent() {
    let repo = MemoryPlatformRepository::default();
    let agent_id = Uuid::new_v4();
    let mut first = running_codex_run(agent_id);
    let second = running_codex_run(agent_id);

    repo.insert_agent_codex_trigger_run(first.clone())
        .expect("first running cycle should be inserted");
    let error = repo
        .insert_agent_codex_trigger_run(second.clone())
        .expect_err("second running cycle for the same Agent must be rejected");
    assert!(matches!(
        error,
        ai_chat_shared::AppError::Conflict(message)
            if message.contains("already has a running Codex trigger run")
    ));

    first.status = "succeeded".into();
    first.finished_at = Some(now_utc());
    repo.update_agent_codex_trigger_run(first)
        .expect("completed cycle should be saved");
    repo.insert_agent_codex_trigger_run(second)
        .expect("a new cycle should be allowed after the previous one ends");
}

#[test]
fn due_codex_trigger_requires_a_logged_in_company_human() {
    let repo = MemoryPlatformRepository::default();
    let company_id = Uuid::new_v4();
    let human_user_id = Uuid::new_v4();
    let agent_id = Uuid::new_v4();
    let now = now_utc();
    {
        let mut guard = repo.inner.write().expect("memory repo lock poisoned");
        guard.company_human_members.insert(
            (company_id, human_user_id),
            CompanyHumanMember {
                id: Uuid::new_v4(),
                company_id,
                human_user_id,
                role: "owner".into(),
                status: "active".into(),
                created_at: now,
                updated_at: now,
            },
        );
        guard.agent_codex_trigger_configs.insert(
            agent_id,
            AgentCodexTriggerConfig {
                id: Uuid::new_v4(),
                company_id,
                agent_profile_id: agent_id,
                status: AGENT_CODEX_TRIGGER_STATUS_ACTIVE.into(),
                interval_seconds: 3_600,
                codex_profile: "default".into(),
                model: None,
                reasoning_effort: None,
                reasoning_summary: None,
                verbosity: None,
                personality: None,
                service_tier: None,
                sandbox_mode: "workspace_write".into(),
                approval_policy: "never".into(),
                network_access: None,
                web_search: None,
                feature_multi_agent: None,
                feature_remote_plugin: None,
                feature_hooks: None,
                feature_goals: None,
                feature_shell_tool: None,
                max_run_seconds: 1_800,
                next_run_at: now,
                lease_owner: None,
                lease_expires_at: None,
                manual_run_requested_at: None,
                wake_requested_at: None,
                wake_reason: None,
                last_run_at: None,
                last_success_at: None,
                last_error: None,
                consecutive_failure_count: 0,
                created_by_human_user_id: human_user_id,
                updated_by_human_user_id: None,
                created_at: now,
                updated_at: now,
            },
        );
    }

    assert!(repo
        .claim_due_agent_codex_trigger_configs("worker-1", now, 1)
        .expect("logged-out company should be checked")
        .is_empty());

    repo.insert_human_session(HumanSession {
        id: Uuid::new_v4(),
        human_user_id,
        token_prefix: "hus_expired".into(),
        token_hash: Uuid::new_v4().simple().to_string(),
        expires_at: now,
        revoked_at: None,
        last_used_at: None,
        created_at: now - chrono::Duration::hours(1),
    })
    .expect("expired session should be inserted");
    assert!(repo
        .claim_due_agent_codex_trigger_configs("worker-1", now, 1)
        .expect("expired login should be checked")
        .is_empty());

    repo.insert_human_session(HumanSession {
        id: Uuid::new_v4(),
        human_user_id,
        token_prefix: "hus_stale".into(),
        token_hash: Uuid::new_v4().simple().to_string(),
        expires_at: now + chrono::Duration::hours(1),
        revoked_at: None,
        last_used_at: Some(now - chrono::Duration::minutes(2)),
        created_at: now - chrono::Duration::minutes(2),
    })
    .expect("stale session should be inserted");
    assert!(repo
        .claim_due_agent_codex_trigger_configs("worker-1", now, 1)
        .expect("inactive login should be checked")
        .is_empty());

    repo.insert_human_session(HumanSession {
        id: Uuid::new_v4(),
        human_user_id,
        token_prefix: "hus_active".into(),
        token_hash: Uuid::new_v4().simple().to_string(),
        expires_at: now + chrono::Duration::hours(1),
        revoked_at: None,
        last_used_at: Some(now),
        created_at: now,
    })
    .expect("active session should be inserted");
    let claimed = repo
        .claim_due_agent_codex_trigger_configs("worker-1", now, 1)
        .expect("logged-in company trigger should be claimable");
    assert_eq!(claimed.len(), 1);
    assert_eq!(claimed[0].company_id, company_id);
}

#[test]
fn message_wake_requested_during_a_run_is_preserved_for_the_next_cycle() {
    let repo = MemoryPlatformRepository::default();
    let agent_id = Uuid::new_v4();
    let config_id = Uuid::new_v4();
    let company_id = Uuid::new_v4();
    let claimed_at = now_utc();
    seed_logged_in_company(&repo, company_id, claimed_at);
    repo.inner
        .write()
        .expect("memory repo lock poisoned")
        .agent_codex_trigger_configs
        .insert(
            agent_id,
            AgentCodexTriggerConfig {
                id: config_id,
                company_id,
                agent_profile_id: agent_id,
                status: AGENT_CODEX_TRIGGER_STATUS_ACTIVE.into(),
                interval_seconds: 3_600,
                codex_profile: "default".into(),
                model: None,
                reasoning_effort: None,
                reasoning_summary: None,
                verbosity: None,
                personality: None,
                service_tier: None,
                sandbox_mode: "workspace_write".into(),
                approval_policy: "never".into(),
                network_access: None,
                web_search: None,
                feature_multi_agent: None,
                feature_remote_plugin: None,
                feature_hooks: None,
                feature_goals: None,
                feature_shell_tool: None,
                max_run_seconds: 1_800,
                next_run_at: claimed_at,
                lease_owner: None,
                lease_expires_at: None,
                manual_run_requested_at: None,
                wake_requested_at: None,
                wake_reason: None,
                last_run_at: None,
                last_success_at: None,
                last_error: None,
                consecutive_failure_count: 0,
                created_by_human_user_id: Uuid::new_v4(),
                updated_by_human_user_id: None,
                created_at: claimed_at,
                updated_at: claimed_at,
            },
        );

    let claimed = repo
        .claim_due_agent_codex_trigger_configs("worker-1", claimed_at, 1)
        .expect("trigger should be claimed");
    assert_eq!(claimed.len(), 1);
    let wake_requested_at = claimed_at + chrono::Duration::seconds(1);
    assert!(repo
        .request_agent_codex_trigger_wake(agent_id, wake_requested_at, "message")
        .expect("message wake should be recorded"));
    repo.complete_agent_codex_trigger_lease(CompleteAgentCodexTriggerLeaseInput {
        trigger_config_id: config_id,
        lease_owner: "worker-1".into(),
        finished_at: claimed_at + chrono::Duration::seconds(2),
        next_run_at: claimed_at + chrono::Duration::hours(1),
        succeeded: true,
        error_message: None,
    })
    .expect("lease should complete");

    let config = repo
        .get_agent_codex_trigger_config_by_agent(agent_id)
        .expect("trigger config should remain");
    assert_eq!(config.wake_requested_at, Some(wake_requested_at));
    assert_eq!(config.wake_reason.as_deref(), Some("message"));
    assert_eq!(config.next_run_at, wake_requested_at);
    assert!(config.lease_owner.is_none());
}

#[test]
fn trigger_shutdown_marks_owned_running_cycles_as_lease_lost() {
    let repo = MemoryPlatformRepository::default();
    let agent_id = Uuid::new_v4();
    let config_id = Uuid::new_v4();
    let company_id = Uuid::new_v4();
    let claimed_at = now_utc();
    seed_logged_in_company(&repo, company_id, claimed_at);
    repo.inner
        .write()
        .expect("memory repo lock poisoned")
        .agent_codex_trigger_configs
        .insert(
            agent_id,
            AgentCodexTriggerConfig {
                id: config_id,
                company_id,
                agent_profile_id: agent_id,
                status: AGENT_CODEX_TRIGGER_STATUS_ACTIVE.into(),
                interval_seconds: 3_600,
                codex_profile: "default".into(),
                model: None,
                reasoning_effort: None,
                reasoning_summary: None,
                verbosity: None,
                personality: None,
                service_tier: None,
                sandbox_mode: "workspace_write".into(),
                approval_policy: "never".into(),
                network_access: None,
                web_search: None,
                feature_multi_agent: None,
                feature_remote_plugin: None,
                feature_hooks: None,
                feature_goals: None,
                feature_shell_tool: None,
                max_run_seconds: 1_800,
                next_run_at: claimed_at,
                lease_owner: None,
                lease_expires_at: None,
                manual_run_requested_at: None,
                wake_requested_at: None,
                wake_reason: None,
                last_run_at: None,
                last_success_at: None,
                last_error: None,
                consecutive_failure_count: 0,
                created_by_human_user_id: Uuid::new_v4(),
                updated_by_human_user_id: None,
                created_at: claimed_at,
                updated_at: claimed_at,
            },
        );
    repo.claim_due_agent_codex_trigger_configs("worker-1", claimed_at, 1)
        .expect("trigger should be claimed");
    let mut run = running_codex_run(agent_id);
    run.trigger_config_id = config_id;
    let run_id = run.id;
    repo.insert_agent_codex_trigger_run(run)
        .expect("running cycle should be inserted");

    let stopped_at = claimed_at + chrono::Duration::seconds(5);
    let abandoned = repo
        .abandon_agent_codex_trigger_leases("worker-1", stopped_at)
        .expect("owned leases should be abandoned");

    assert_eq!(abandoned, 1);
    let guard = repo.inner.read().expect("memory repo lock poisoned");
    let config = guard
        .agent_codex_trigger_configs
        .get(&agent_id)
        .expect("trigger config");
    assert!(config.lease_owner.is_none());
    assert!(config.lease_expires_at.is_none());
    assert_eq!(config.next_run_at, claimed_at);
    let run = guard
        .agent_codex_trigger_runs
        .get(&run_id)
        .expect("trigger run");
    assert_eq!(run.status, AGENT_CODEX_RUN_STATUS_LEASE_LOST);
    assert_eq!(run.finished_at, Some(stopped_at));
    assert_eq!(run.activity_phase, "lease_lost");
}

#[test]
fn manual_wake_requested_during_a_run_is_preserved_for_the_next_cycle() {
    let repo = MemoryPlatformRepository::default();
    let agent_id = Uuid::new_v4();
    let config_id = Uuid::new_v4();
    let company_id = Uuid::new_v4();
    let claimed_at = now_utc();
    seed_logged_in_company(&repo, company_id, claimed_at);
    repo.inner
        .write()
        .expect("memory repo lock poisoned")
        .agent_codex_trigger_configs
        .insert(
            agent_id,
            AgentCodexTriggerConfig {
                id: config_id,
                company_id,
                agent_profile_id: agent_id,
                status: AGENT_CODEX_TRIGGER_STATUS_ACTIVE.into(),
                interval_seconds: 3_600,
                codex_profile: "default".into(),
                model: None,
                reasoning_effort: None,
                reasoning_summary: None,
                verbosity: None,
                personality: None,
                service_tier: None,
                sandbox_mode: "workspace_write".into(),
                approval_policy: "never".into(),
                network_access: None,
                web_search: None,
                feature_multi_agent: None,
                feature_remote_plugin: None,
                feature_hooks: None,
                feature_goals: None,
                feature_shell_tool: None,
                max_run_seconds: 1_800,
                next_run_at: claimed_at,
                lease_owner: None,
                lease_expires_at: None,
                manual_run_requested_at: None,
                wake_requested_at: None,
                wake_reason: None,
                last_run_at: None,
                last_success_at: None,
                last_error: None,
                consecutive_failure_count: 0,
                created_by_human_user_id: Uuid::new_v4(),
                updated_by_human_user_id: None,
                created_at: claimed_at,
                updated_at: claimed_at,
            },
        );

    repo.claim_due_agent_codex_trigger_configs("worker-1", claimed_at, 1)
        .expect("trigger should be claimed");
    let manual_requested_at = claimed_at + chrono::Duration::seconds(1);
    {
        let mut guard = repo.inner.write().expect("memory repo lock poisoned");
        let config = guard
            .agent_codex_trigger_configs
            .get_mut(&agent_id)
            .expect("trigger config should exist");
        config.manual_run_requested_at = Some(manual_requested_at);
        config.next_run_at = manual_requested_at;
    }
    repo.complete_agent_codex_trigger_lease(CompleteAgentCodexTriggerLeaseInput {
        trigger_config_id: config_id,
        lease_owner: "worker-1".into(),
        finished_at: claimed_at + chrono::Duration::seconds(2),
        next_run_at: claimed_at + chrono::Duration::hours(1),
        succeeded: true,
        error_message: None,
    })
    .expect("lease should complete");

    let config = repo
        .get_agent_codex_trigger_config_by_agent(agent_id)
        .expect("trigger config should remain");
    assert_eq!(config.manual_run_requested_at, Some(manual_requested_at));
    assert_eq!(config.next_run_at, manual_requested_at);
    assert!(config.lease_owner.is_none());
}
