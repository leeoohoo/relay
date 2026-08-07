use super::*;
use ai_chat_domain::company::{
    AgentCodexSession, AGENT_CODEX_SESSION_KIND_CONTROL, AGENT_CODEX_SESSION_KIND_PROJECT,
    AGENT_CODEX_SESSION_STATUS_ACTIVE,
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
