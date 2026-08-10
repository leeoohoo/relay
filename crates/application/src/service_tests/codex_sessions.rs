use super::*;
use ai_chat_domain::company::{
    AgentCodexSession, AgentExecutionIntent, AGENT_CODEX_SESSION_KIND_CONTROL,
    AGENT_CODEX_SESSION_KIND_PROJECT, AGENT_CODEX_SESSION_STATUS_ACTIVE,
    AGENT_EXECUTION_INTENT_STATUS_FAILED, AGENT_EXECUTION_INTENT_STATUS_PENDING,
    AGENT_EXECUTION_INTENT_STATUS_RUNNING, COMPANY_AGENT_ROLE_MANAGER,
};

fn scoped_session(
    agent_id: Uuid,
    scope_key: String,
    session_kind: &str,
    project_id: Option<Uuid>,
    thread_id: &str,
) -> AgentCodexSession {
    let now = now_utc();
    AgentCodexSession {
        id: Uuid::new_v4(),
        agent_profile_id: agent_id,
        session_kind: session_kind.into(),
        scope_key,
        project_id,
        generation: 1,
        codex_thread_id: thread_id.into(),
        workspace_key: format!("relay-scoped-sessions-v9:{thread_id}"),
        status: AGENT_CODEX_SESSION_STATUS_ACTIVE.into(),
        summary_short: format!("checkpoint for {thread_id}"),
        checkpoint_json: json!({ "thread_id": thread_id }),
        skill_bundle_version: "skills-v1".into(),
        memory_snapshot_version: "memory-v1".into(),
        policy_version: "relay-scoped-sessions-v9".into(),
        created_at: now,
        last_used_at: now,
        archived_at: None,
    }
}

#[test]
fn control_and_multiple_project_sessions_are_stored_independently() {
    let app = PlatformApp::new(MemoryPlatformRepository::default());
    let agent_id = Uuid::new_v4();
    let first_project_id = Uuid::new_v4();
    let second_project_id = Uuid::new_v4();
    let control = scoped_session(
        agent_id,
        "control".into(),
        AGENT_CODEX_SESSION_KIND_CONTROL,
        None,
        "control-thread",
    );
    let first_worker = scoped_session(
        agent_id,
        format!("project:{first_project_id}"),
        AGENT_CODEX_SESSION_KIND_PROJECT,
        Some(first_project_id),
        "first-project-thread",
    );
    let second_worker = scoped_session(
        agent_id,
        format!("project:{second_project_id}"),
        AGENT_CODEX_SESSION_KIND_PROJECT,
        Some(second_project_id),
        "second-project-thread",
    );

    app.save_agent_codex_session(control.clone())
        .expect("control session should be saved");
    app.save_agent_codex_session(first_worker.clone())
        .expect("first project session should be saved");
    app.save_agent_codex_session(second_worker.clone())
        .expect("second project session should be saved");

    assert_eq!(
        app.get_agent_codex_session(agent_id, "control")
            .expect("control session")
            .codex_thread_id,
        control.codex_thread_id
    );
    assert_eq!(
        app.get_agent_codex_session(agent_id, &format!("project:{first_project_id}"))
            .expect("first project session")
            .codex_thread_id,
        first_worker.codex_thread_id
    );
    assert_eq!(
        app.get_agent_codex_session(agent_id, &format!("project:{second_project_id}"))
            .expect("second project session")
            .codex_thread_id,
        second_worker.codex_thread_id
    );
    assert_eq!(app.list_agent_codex_sessions(agent_id, 10).len(), 3);
}

#[test]
fn repeated_execution_intent_dedupe_key_returns_existing_work() {
    let app = PlatformApp::new(MemoryPlatformRepository::default());
    let human = app
        .dev_login(DevLoginInput {
            email: "intent-dedup@example.com".into(),
            display_name: "Intent Dedup Human".into(),
        })
        .expect("human should be created");
    let company = app
        .create_company(CreateCompanyInput {
            human_user_id: human.id,
            name: "Intent Dedup Company".into(),
            slug: Some("intent-dedup-company".into()),
            description: None,
        })
        .expect("company should be created");
    let agent = app
        .create_company_agent(CreateCompanyAgentInput {
            human_user_id: human.id,
            company_id: company.company.id,
            display_name: "Intent Dedup Manager".into(),
            handle: "intent-dedup-manager".into(),
            persona: "负责幂等派工".into(),
            org_unit_id: None,
            job_title: Some("项目经理".into()),
            role_key: Some(COMPANY_AGENT_ROLE_MANAGER.into()),
            reports_to_membership_id: None,
        })
        .expect("agent should be created");
    let project = app
        .create_company_project_for_human(CreateCompanyProjectForHumanInput {
            human_user_id: human.id,
            company_id: company.company.id,
            owner_agent_id: agent.agent_profile.id,
            name: "Intent Dedup Project".into(),
            description: Some("验证重复派工".into()),
            member_agent_ids: Vec::new(),
            project_type: Some("web_application".into()),
            project_type_source: Some(PROJECT_TYPE_SOURCE_HUMAN.into()),
            project_type_confidence: Some(100),
            project_type_evidence: Vec::new(),
            project_id: None,
        })
        .expect("project should be created");
    let intent = AgentExecutionIntent {
        id: Uuid::new_v4(),
        company_id: company.company.id,
        agent_profile_id: agent.agent_profile.id,
        project_id: project.project.id,
        worker_session_id: None,
        source_event_ids: Vec::new(),
        task_ids: Vec::new(),
        action_type: "execute".into(),
        objective: "完成同一个项目目标".into(),
        acceptance_criteria: vec!["结果可验证".into()],
        priority: "high".into(),
        dedupe_key: "same-logical-work".into(),
        status: AGENT_EXECUTION_INTENT_STATUS_PENDING.into(),
        result_summary: String::new(),
        error_message: None,
        created_at: now_utc(),
        claimed_at: None,
        completed_at: None,
    };

    let created = app
        .create_agent_execution_intent(intent.clone())
        .expect("first dispatch should create work");
    let mut retry = intent.clone();
    retry.id = Uuid::new_v4();
    retry.source_event_ids = vec![Uuid::new_v4()];
    let replayed = app
        .create_agent_execution_intent(retry)
        .expect("same logical work should return the existing intent");
    assert_eq!(replayed.id, created.id);

    let mut different_work = intent;
    different_work.id = Uuid::new_v4();
    different_work.objective = "这是不同的项目目标".into();
    assert!(matches!(
        app.create_agent_execution_intent(different_work),
        Err(AppError::Conflict(message))
            if message.contains("already belongs to execution intent")
                && message.contains("use a new dedupe_key")
    ));
}

#[test]
fn retryable_execution_failure_returns_the_intent_to_pending() {
    let app = PlatformApp::new(MemoryPlatformRepository::default());
    let now = now_utc();
    let intent = AgentExecutionIntent {
        id: Uuid::new_v4(),
        company_id: Uuid::new_v4(),
        agent_profile_id: Uuid::new_v4(),
        project_id: Uuid::new_v4(),
        worker_session_id: Some(Uuid::new_v4()),
        source_event_ids: Vec::new(),
        task_ids: vec![Uuid::new_v4()],
        action_type: "execute".into(),
        objective: "继续未完成的项目任务".into(),
        acceptance_criteria: vec!["任务完成并验证".into()],
        priority: "high".into(),
        dedupe_key: "retry-temporary-codex-failure".into(),
        status: AGENT_EXECUTION_INTENT_STATUS_RUNNING.into(),
        result_summary: "已完成部分实现".into(),
        error_message: None,
        created_at: now,
        claimed_at: Some(now),
        completed_at: Some(now),
    };
    app.repo
        .insert_agent_execution_intent(intent.clone())
        .expect("intent should be stored");

    let recovered = app
        .requeue_agent_execution_intent_after_retryable_failure(
            intent,
            "unexpected status 503 Service Unavailable: auth_unavailable: no auth available",
        )
        .expect("temporary failure should be evaluated")
        .expect("temporary failure should be retried");

    assert_eq!(recovered.status, AGENT_EXECUTION_INTENT_STATUS_PENDING);
    assert!(recovered.claimed_at.is_none());
    assert!(recovered.completed_at.is_none());
    assert_eq!(recovered.result_summary, "已完成部分实现");
    assert!(recovered
        .error_message
        .as_deref()
        .is_some_and(|message| message.contains("Relay 将自动重试")));
    assert_eq!(
        app.list_agent_execution_intents(
            recovered.agent_profile_id,
            Some(AGENT_EXECUTION_INTENT_STATUS_PENDING),
            10,
        )
        .len(),
        1
    );
}

#[test]
fn non_retryable_execution_failure_remains_terminal() {
    let app = PlatformApp::new(MemoryPlatformRepository::default());
    let now = now_utc();
    let intent = AgentExecutionIntent {
        id: Uuid::new_v4(),
        company_id: Uuid::new_v4(),
        agent_profile_id: Uuid::new_v4(),
        project_id: Uuid::new_v4(),
        worker_session_id: None,
        source_event_ids: Vec::new(),
        task_ids: Vec::new(),
        action_type: "execute".into(),
        objective: "执行无效请求".into(),
        acceptance_criteria: Vec::new(),
        priority: "normal".into(),
        dedupe_key: "terminal-validation-failure".into(),
        status: AGENT_EXECUTION_INTENT_STATUS_FAILED.into(),
        result_summary: String::new(),
        error_message: Some("validation failed".into()),
        created_at: now,
        claimed_at: Some(now),
        completed_at: Some(now),
    };
    app.repo
        .insert_agent_execution_intent(intent.clone())
        .expect("intent should be stored");

    let recovered = app
        .requeue_agent_execution_intent_after_retryable_failure(
            intent.clone(),
            "validation failed: missing project",
        )
        .expect("terminal failure should be evaluated");

    assert!(recovered.is_none());
    assert_eq!(
        app.list_agent_execution_intents(
            intent.agent_profile_id,
            Some(AGENT_EXECUTION_INTENT_STATUS_FAILED),
            10,
        )
        .len(),
        1
    );
}
