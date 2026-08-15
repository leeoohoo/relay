use std::{collections::HashMap, path::PathBuf};

use ai_chat_domain::agent_identity::{AgentInboxEvent, AgentInboxEventStatus, AgentStatus};

use super::*;

#[test]
fn control_prompt_compacts_large_snapshots_and_normalizes_user_controls() {
    let now = now_utc();
    let agent = AgentProfile {
        id: Uuid::new_v4(),
        owner_user_id: Uuid::new_v4(),
        display_name: "Luna".into(),
        handle: "luna-engineer".into(),
        persona: "负责可靠交付".into(),
        collaboration_preference: "available".into(),
        status: AgentStatus::Active,
        created_at: now,
    };
    let workspace = PreparedGitWorkspace {
        path: PathBuf::from("/tmp/relay-agent"),
        worktree_key: "project/agent".into(),
        branch: "relay/agent/work".into(),
        auth_environment: HashMap::new(),
    };
    let skills = PreparedRelaySkills {
        employee_name: "relay-luna-employee".into(),
        profession_name: "relay-luna-profession-software-engineer".into(),
        session_name: "relay-luna-control".into(),
        project_name: None,
        staffing_name: None,
        version_hash: "v1".into(),
    };
    let messages = (0..50)
        .map(|index| AgentInboxEvent {
            id: Uuid::new_v4(),
            agent_profile_id: agent.id,
            event_type: "message.received".into(),
            event_class: "communication".into(),
            requires_action: true,
            wake_policy: "immediate".into(),
            dedupe_key: None,
            coalesce_key: None,
            causation_id: None,
            correlation_id: None,
            payload_json: serde_json::json!({
                "conversation_id": Uuid::new_v4(),
                "message_id": Uuid::new_v4(),
                "content": format!("消息 {index}\r\n{}", "很长的内容".repeat(500)),
                "mentioned": true,
            }),
            priority: 1,
            available_at: now,
            expires_at: None,
            handled_by_run_id: None,
            processed_at: None,
            status: AgentInboxEventStatus::Pending,
            created_at: now,
        })
        .collect::<Vec<_>>();
    let control_snapshot = ai_chat_application::AgentControlSnapshot {
        agent_profile_id: agent.id,
        company_id: Uuid::new_v4(),
        generated_at: now,
        snapshot_version: "large-snapshot".into(),
        unread_messages: messages.clone(),
        actionable_events: messages,
        ready_tasks: Vec::new(),
        waiting_tasks: Vec::new(),
        task_readiness: Vec::new(),
        active_intents: Vec::new(),
        work_sessions: Vec::new(),
    };
    let prompt = build_wakeup_prompt(WakeupPromptContext {
        agent: &agent,
        job_title: "软件工程师",
        project_name: None,
        pending_inbox_count: 50,
        active_task_count: 0,
        waiting_task_count: 0,
        asset_refresh_due: false,
        control_snapshot: &control_snapshot,
        workspace: &workspace,
        relay_skills: &skills,
    });
    let prompt = normalize_codex_prompt(&prompt);

    assert!(prompt.chars().count() < 100_000);
    assert!(prompt.contains("\"unread_messages_truncated\":true"));
    assert!(prompt.contains("\"actionable_message_events_covered_by_unread_messages\":50"));
    assert!(!prompt.contains('\r'));
    assert!(!prompt
        .chars()
        .any(|character| character.is_control() && !matches!(character, '\n' | '\t')));
}
