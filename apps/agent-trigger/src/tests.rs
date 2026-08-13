use std::{collections::HashMap, path::PathBuf};

use ai_chat_domain::agent_identity::AgentStatus;

use super::*;

#[test]
fn website_always_allow_key_is_scoped_to_project_and_origin() {
    let agent_id = Uuid::new_v4();
    let project_id = Uuid::new_v4();
    let mut arguments = serde_json::json!({
        "url": "https://example.com:8443/dashboard?tab=one",
        "tool": "new_page"
    });

    let (scope, target) = website_approval_grant_key(
        &mut arguments,
        AGENT_CODEX_APPROVAL_TOOL_WEBSITE_ACCESS,
        agent_id,
        Some(project_id),
    )
    .expect("website grant key");

    assert_eq!(scope, format!("project:{project_id}"));
    assert_eq!(target, "https://example.com:8443");
    assert_eq!(
        arguments
            .get(AGENT_CODEX_APPROVAL_SCOPE_KEY)
            .and_then(serde_json::Value::as_str),
        Some(scope.as_str())
    );
    assert_eq!(
        arguments
            .get(AGENT_CODEX_APPROVAL_TARGET_KEY)
            .and_then(serde_json::Value::as_str),
        Some(target.as_str())
    );
}

#[test]
fn website_session_grant_covers_the_same_origin_only() {
    let agent_id = Uuid::new_v4();
    let project_id = Uuid::new_v4();
    let grants = Mutex::new(HashSet::new());
    let mut first = serde_json::json!({ "url": "https://example.com/start" });
    let grant = website_approval_grant_key(
        &mut first,
        AGENT_CODEX_APPROVAL_TOOL_WEBSITE_ACCESS,
        agent_id,
        Some(project_id),
    )
    .expect("website grant");
    remember_session_website_grant(&grants, &grant);

    let mut same_origin = serde_json::json!({ "url": "https://example.com/next?step=2" });
    let (scope, target) = website_approval_grant_key(
        &mut same_origin,
        AGENT_CODEX_APPROVAL_TOOL_WEBSITE_ACCESS,
        agent_id,
        Some(project_id),
    )
    .expect("same-origin grant key");
    assert!(session_website_grant_allowed(&grants, &scope, &target));

    let mut another_origin = serde_json::json!({ "url": "https://other.example.com/" });
    let (scope, target) = website_approval_grant_key(
        &mut another_origin,
        AGENT_CODEX_APPROVAL_TOOL_WEBSITE_ACCESS,
        agent_id,
        Some(project_id),
    )
    .expect("other-origin grant key");
    assert!(!session_website_grant_allowed(&grants, &scope, &target));
}

#[test]
fn website_always_allow_key_rejects_non_web_targets() {
    let mut arguments = serde_json::json!({ "url": "file:///tmp/report.html" });
    assert!(website_approval_grant_key(
        &mut arguments,
        AGENT_CODEX_APPROVAL_TOOL_WEBSITE_ACCESS,
        Uuid::new_v4(),
        Some(Uuid::new_v4()),
    )
    .is_none());
}

#[test]
fn website_localhost_grant_covers_only_non_privileged_loopback_ports() {
    for url in [
        "http://127.0.0.1:4177/",
        "http://localhost:5173/",
        "http://[::1]:8080/",
    ] {
        let mut arguments = serde_json::json!({ "url": url });
        website_approval_grant_key(
            &mut arguments,
            AGENT_CODEX_APPROVAL_TOOL_WEBSITE_ACCESS,
            Uuid::new_v4(),
            Some(Uuid::new_v4()),
        )
        .expect("local website key");
        assert_eq!(
            arguments
                .get(AGENT_CODEX_APPROVAL_LOCAL_TARGET_KEY)
                .and_then(serde_json::Value::as_str),
            Some("http://localhost:*")
        );
    }

    for url in ["http://127.0.0.1:80/", "https://example.com:4177/"] {
        let mut arguments = serde_json::json!({ "url": url });
        website_approval_grant_key(
            &mut arguments,
            AGENT_CODEX_APPROVAL_TOOL_WEBSITE_ACCESS,
            Uuid::new_v4(),
            Some(Uuid::new_v4()),
        )
        .expect("website key");
        assert!(arguments
            .get(AGENT_CODEX_APPROVAL_LOCAL_TARGET_KEY)
            .is_none());
    }
}

#[test]
fn managed_codex_installer_supports_macos_linux_and_windows() {
    for host_os in ["macos", "linux"] {
        let command = codex_installer_command(host_os).expect("POSIX installer");
        assert_eq!(command.program, "sh");
        assert_eq!(command.arguments, &["-s"]);
        assert_eq!(command.kind, CODEX_INSTALLER_POSIX_SHELL);
        assert_eq!(
            default_codex_install_url(host_os),
            "https://chatgpt.com/codex/install.sh"
        );
    }

    let windows = codex_installer_command("windows").expect("Windows installer");
    assert_eq!(windows.program, "powershell.exe");
    assert!(windows.arguments.contains(&"-NonInteractive"));
    assert!(windows.arguments.contains(&"Bypass"));
    assert_eq!(windows.kind, CODEX_INSTALLER_POWERSHELL);
    assert_eq!(
        default_codex_install_url("windows"),
        "https://chatgpt.com/codex/install.ps1"
    );
}

#[test]
fn managed_codex_installer_rejects_unknown_operating_systems() {
    let error = codex_installer_command("plan9").expect_err("unsupported platform");
    assert!(error.to_string().contains("does not support"));
}

#[test]
fn session_key_is_stable_when_skills_language_or_plugins_change() {
    let workspace = PreparedGitWorkspace {
        path: PathBuf::from("/tmp/relay-agent"),
        worktree_key: "project/agent".into(),
        branch: "relay/agent/inbox".into(),
        auth_environment: HashMap::new(),
    };
    assert_eq!(
        codex_session_key(&workspace),
        "relay-scoped-sessions-v9:project/agent"
    );
    assert!(codex_session_key_matches(
        "relay-scoped-sessions-v9:project/agent:old-skill-hash:old-plugin-hash",
        "relay-scoped-sessions-v9:project/agent"
    ));
    assert!(!codex_session_key_matches(
        "relay-scoped-sessions-v9:another-project/agent:old-skill-hash:old-plugin-hash",
        "relay-scoped-sessions-v9:project/agent"
    ));
}

#[test]
fn plugin_fingerprint_is_stable_and_tracks_enabled_versions() {
    let first = serde_json::json!([
        {"pluginId": "browser@openai-bundled", "version": "2", "enabled": true},
        {"pluginId": "github@openai-api-curated", "version": "1", "enabled": true}
    ]);
    let reordered = serde_json::json!([
        {"pluginId": "github@openai-api-curated", "version": "1", "enabled": true},
        {"pluginId": "browser@openai-bundled", "version": "2", "enabled": true}
    ]);
    assert_eq!(
        codex_plugin_fingerprint(&first),
        codex_plugin_fingerprint(&reordered)
    );
    assert_ne!(
        codex_plugin_fingerprint(&first),
        codex_plugin_fingerprint(&serde_json::json!([
            {"pluginId": "browser@openai-bundled", "version": "3", "enabled": true}
        ]))
    );
    assert_eq!(
        codex_plugin_fingerprint(&serde_json::json!([
            {"pluginId": "browser@openai-bundled", "version": "2", "enabled": true}
        ])),
        codex_plugin_fingerprint(&serde_json::json!([]))
    );
}

#[test]
fn empty_plugin_catalog_is_not_treated_as_a_successful_catalog() {
    assert!(codex_plugin_catalog_is_empty(
        &serde_json::json!([]),
        &serde_json::json!([]),
        &serde_json::json!([]),
    ));
    assert!(!codex_plugin_catalog_is_empty(
        &serde_json::json!([]),
        &serde_json::json!([]),
        &serde_json::json!([{ "name": "openai-bundled" }]),
    ));
}

#[test]
fn permission_blocks_are_removed_when_the_agent_lacks_the_permission() {
    let template = "before\n<!-- relay-permission:task.assign:start -->secret\n<!-- relay-permission:task.assign:end -->\nafter";
    assert_eq!(
        tailor_relay_skill_to_permissions(template, &[]),
        "before\n\nafter\n"
    );
    assert!(
        tailor_relay_skill_to_permissions(template, &["task.assign".into()]).contains("secret")
    );
}

#[test]
fn control_session_loads_profession_skill_without_project_skill() {
    let workspace = std::env::temp_dir().join(format!("relay-skill-test-{}", Uuid::new_v4()));
    fs::create_dir_all(&workspace).expect("test workspace should be created");
    let agent = AgentProfile {
        id: Uuid::new_v4(),
        owner_user_id: Uuid::new_v4(),
        display_name: "Luna".into(),
        handle: "luna-engineer".into(),
        persona: "负责实现".into(),
        collaboration_preference: "available".into(),
        status: AgentStatus::Active,
        created_at: now_utc(),
    };
    let prepared = prepare_relay_skills(
        &workspace,
        RELAY_SKILL_BUNDLE_CONTROL,
        &agent,
        "软件工程师",
        &["task.update".into()],
        &[],
        None,
        None,
        "zh-CN",
    )
    .expect("managed skills should be generated");
    assert!(workspace
        .join(".agents/skills")
        .join(&prepared.employee_name)
        .join("SKILL.md")
        .is_file());
    let employee_skill = fs::read_to_string(
        workspace
            .join(".agents/skills")
            .join(&prepared.employee_name)
            .join("SKILL.md"),
    )
    .expect("employee skill should be readable");
    #[cfg(unix)]
    assert!(fs::symlink_metadata(
        workspace
            .join(".agents/skills")
            .join(&prepared.employee_name)
    )
    .expect("managed Skill link should exist")
    .file_type()
    .is_symlink());
    assert!(workspace
        .join(".agents/skills")
        .join(&prepared.profession_name)
        .join("SKILL.md")
        .is_file());
    let profession_skill = fs::read_to_string(
        workspace
            .join(".agents/skills")
            .join(&prepared.profession_name)
            .join("SKILL.md"),
    )
    .expect("control profession skill should be readable");
    let control_skill = fs::read_to_string(
        workspace
            .join(".agents/skills")
            .join(&prepared.session_name)
            .join("SKILL.md"),
    )
    .expect("control session skill should be readable");
    assert!(profession_skill.contains("软件工程师"));
    assert!(employee_skill.contains("Relay 已认证身份"));
    assert!(employee_skill.contains("岗位：`软件工程师`"));
    assert!(employee_skill.contains("不得向 Human 或同事再次确认"));
    assert!(employee_skill.contains("问题报告与任务化闭环"));
    assert!(!employee_skill.contains("每轮先调用 `agent.bootstrap` 核对返回身份"));
    assert!(!profession_skill.contains("Relay 已认证身份"));
    assert!(control_skill.contains("必须同时遵循职业 Skill"));
    assert!(prepared.project_name.is_none());
    assert!(!prepared.version_hash.is_empty());
    fs::remove_dir_all(workspace).expect("test workspace should be removed");
}

#[test]
fn prompts_treat_identity_as_authenticated_session_state() {
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
        project_name: Some("relay-luna-project".into()),
        staffing_name: None,
        version_hash: "v1".into(),
    };
    let control_snapshot = ai_chat_application::AgentControlSnapshot {
        agent_profile_id: agent.id,
        company_id: Uuid::new_v4(),
        generated_at: now,
        snapshot_version: "snapshot-v1".into(),
        unread_messages: Vec::new(),
        actionable_events: Vec::new(),
        ready_tasks: Vec::new(),
        waiting_tasks: Vec::new(),
        active_intents: Vec::new(),
        work_sessions: Vec::new(),
    };
    let control_prompt = build_wakeup_prompt(WakeupPromptContext {
        agent: &agent,
        job_title: "软件工程师",
        project_name: None,
        pending_inbox_count: 0,
        active_task_count: 1,
        waiting_task_count: 0,
        asset_refresh_due: false,
        control_snapshot: &control_snapshot,
        workspace: &workspace,
        relay_skills: &skills,
    });
    assert!(control_prompt.contains("run token 固定并认证此身份"));
    assert!(control_prompt.contains("不要向 Human、同事或其他工具重新询问或确认"));
    assert!(control_prompt
        .contains("不要重复调用 agent.bootstrap、company.task my 或 agent.inbox.wait"));
    assert!(control_prompt.contains("snapshot-v1"));
    assert!(control_prompt.contains("unread_messages"));
    assert!(control_prompt.contains("先按时间顺序理解该会话内更早的全部未读消息"));
    assert!(control_prompt.contains("remaining_has_mentions"));
    assert!(control_prompt.contains("only_if_no_mentions=true"));
    assert!(!control_prompt.contains("核对返回身份"));

    let project = CompanyProject {
        id: Uuid::new_v4(),
        company_id: Uuid::new_v4(),
        name: "Relay Web".into(),
        description: "管理控制台".into(),
        project_type: "software_development".into(),
        project_type_source: "human".into(),
        project_type_confidence: 100,
        project_type_evidence: vec![],
        status: "active".into(),
        owner_agent_id: agent.id,
        project_group_conversation_id: Uuid::new_v4(),
        created_by_agent_id: agent.id,
        updated_by_agent_id: None,
        due_at: None,
        created_at: now,
        updated_at: now,
        completed_at: None,
    };
    let intent = AgentExecutionIntent {
        id: Uuid::new_v4(),
        company_id: project.company_id,
        agent_profile_id: agent.id,
        project_id: project.id,
        worker_session_id: None,
        source_event_ids: vec![],
        task_ids: vec![],
        action_type: "execute".into(),
        objective: "完成任务".into(),
        acceptance_criteria: vec![],
        priority: "normal".into(),
        dedupe_key: "test".into(),
        status: "pending".into(),
        result_summary: String::new(),
        error_message: None,
        created_at: now,
        claimed_at: None,
        completed_at: None,
    };
    let worker_prompt = build_worker_prompt(WorkerPromptContext {
        agent: &agent,
        job_title: "软件工程师",
        project: &project,
        intent: &intent,
        workspace: &workspace,
        relay_skills: &skills,
        previous_checkpoint: None,
    });
    assert!(worker_prompt.contains("不要重新确认、询问或汇报自己的身份"));
    assert!(worker_prompt.contains("直接用 company.project get 和 company.task get/list"));
    assert!(!worker_prompt.contains("先调用 agent.bootstrap"));
}

#[test]
fn session_summary_replaces_workspace_and_redacts_host_username() {
    let workspace = PathBuf::from("/Users/alice/.relay/worktrees/agent-1");
    let summary = "Changed [/Users/alice/.relay/worktrees/agent-1/src/main.rs](/Users/alice/.relay/worktrees/agent-1/src/main.rs) and inspected /Users/alice/private.txt";
    let sanitized = sanitize_workspace_output(summary, &workspace);
    assert!(sanitized.contains("[./src/main.rs](./src/main.rs)"));
    assert!(sanitized.contains("~/private.txt"));
    assert!(!sanitized.contains("alice"));
}

#[test]
fn project_worker_session_keeps_inbox_work_in_the_control_session() {
    let chinese = session_skill_template(RELAY_SKILL_BUNDLE_PROJECT, "zh-CN");
    assert!(chinese.contains("忽略 `inbox_notice`"));
    assert!(chinese.contains("不调用 `agent.inbox.wait`"));
    assert!(chinese.contains("统一留给控制会话"));
    assert!(chinese.contains("必须使用 Relay 托管的 `chrome-devtools` MCP"));
    assert!(chinese.contains("`.relay/browser-artifacts/`"));
    assert!(chinese.contains("不得改用 Codex 桌面 Browser/Chrome"));
    assert!(chinese.contains("Relay 托管的 `$TMPDIR`"));
    assert!(chinese.contains("禁止直接使用 `/tmp`、`/private/tmp`"));
    assert!(chinese.contains("固定字段只能使用工具 Schema 暴露的枚举"));
    assert!(chinese.contains("禁止原样重试"));

    let english = session_skill_template(RELAY_SKILL_BUNDLE_PROJECT, "en");
    assert!(english.contains("Ignore `inbox_notice`"));
    assert!(english.contains("control session"));
    assert!(english.contains("Relay-managed `chrome-devtools` MCP"));
    assert!(english.contains("`.relay/browser-artifacts/`"));
    assert!(english.contains("Do not use Codex desktop Browser/Chrome"));
    assert!(english.contains("Relay-managed `$TMPDIR`"));
    assert!(english.contains("Never address `/tmp`, `/private/tmp`"));
    assert!(english.contains("use only values exposed by the tool schema"));
    assert!(english.contains("instead of repeating the same call"));

    let control = session_skill_template(RELAY_SKILL_BUNDLE_CONTROL, "zh-CN");
    assert!(control.contains("Relay 托管的 `$TMPDIR`"));
}

#[test]
fn long_agent_handles_keep_all_relay_skill_names_within_codex_limits() {
    let workspace = std::env::temp_dir().join(format!(
        "relay-long-skill-name-test-{}",
        Uuid::new_v4().simple()
    ));
    fs::create_dir_all(&workspace).expect("test workspace");
    let agent_id = Uuid::new_v4();
    let agent_id_token = agent_id
        .to_string()
        .replace('-', "")
        .chars()
        .take(8)
        .collect::<String>();
    let old_skills_root = workspace.join(".agents/skills");
    fs::create_dir_all(&old_skills_root).expect("old skills root");
    let old_long_skill = old_skills_root.join(format!(
        "relay-life-science-worldbuilding-{agent_id_token}-profession-research-specialist"
    ));
    fs::create_dir_all(&old_long_skill).expect("old long skill");
    let agent = AgentProfile {
        id: agent_id,
        owner_user_id: Uuid::new_v4(),
        display_name: "Researcher".into(),
        handle: "life-science-worldbuilding-and-continuity-review".into(),
        persona: "Review scientific continuity".into(),
        collaboration_preference: "available".into(),
        status: AgentStatus::Active,
        created_at: now_utc(),
    };
    let project = CompanyProject {
        id: Uuid::new_v4(),
        company_id: Uuid::new_v4(),
        name: "Novel".into(),
        description: "Novel project".into(),
        project_type: "novel_writing".into(),
        project_type_source: "user".into(),
        project_type_confidence: 100,
        project_type_evidence: vec![],
        status: "active".into(),
        owner_agent_id: agent.id,
        project_group_conversation_id: Uuid::new_v4(),
        created_by_agent_id: agent.id,
        updated_by_agent_id: None,
        due_at: None,
        created_at: now_utc(),
        updated_at: now_utc(),
        completed_at: None,
    };

    let prepared = prepare_relay_skills(
        &workspace,
        RELAY_SKILL_BUNDLE_PROJECT,
        &agent,
        "Research Specialist",
        &[],
        &[],
        Some(&project),
        None,
        "en",
    )
    .expect("managed skills");

    for name in [
        Some(prepared.employee_name.as_str()),
        Some(prepared.profession_name.as_str()),
        Some(prepared.session_name.as_str()),
        prepared.project_name.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        assert!(
            name.len() <= 64,
            "Relay skill name exceeds Codex limit: {name}"
        );
    }
    assert!(
        !old_long_skill.exists(),
        "legacy long Relay skill should be removed"
    );

    fs::remove_dir_all(workspace).expect("cleanup long skill name test");
}

#[test]
fn managed_browser_artifacts_are_excluded_from_project_git_status() {
    let workspace = std::env::temp_dir().join(format!(
        "relay-browser-artifact-exclude-test-{}",
        Uuid::new_v4()
    ));
    fs::create_dir_all(&workspace).expect("test workspace should be created");

    exclude_managed_skills_from_git(&workspace, "relay-test-")
        .expect("managed runtime paths should be excluded");

    let exclude = fs::read_to_string(workspace.join(".relay-git/info/exclude"))
        .expect("exclude file should exist");
    assert!(exclude.contains("/.agents/skills/relay-test-*/"));
    assert!(exclude.contains("/.relay/browser-artifacts/"));
    fs::remove_dir_all(workspace).expect("test workspace should be removed");
}

#[test]
fn english_relay_skills_are_materialized_without_chinese_operating_rules() {
    let workspace = std::env::temp_dir().join(format!("relay-skill-en-test-{}", Uuid::new_v4()));
    fs::create_dir_all(&workspace).expect("test workspace should be created");
    let agent = AgentProfile {
        id: Uuid::new_v4(),
        owner_user_id: Uuid::new_v4(),
        display_name: "Luna".into(),
        handle: "luna-security".into(),
        persona: "Own application security".into(),
        collaboration_preference: "available".into(),
        status: AgentStatus::Active,
        created_at: now_utc(),
    };
    let prepared = prepare_relay_skills(
        &workspace,
        RELAY_SKILL_BUNDLE_CONTROL,
        &agent,
        "Security Engineer",
        &["task.update".into()],
        &[],
        None,
        None,
        "en",
    )
    .expect("English managed skills should be generated");
    let employee_skill = fs::read_to_string(
        workspace
            .join(".agents/skills")
            .join(&prepared.employee_name)
            .join("SKILL.md"),
    )
    .expect("employee skill should be readable");
    let profession_skill = fs::read_to_string(
        workspace
            .join(".agents/skills")
            .join(&prepared.profession_name)
            .join("SKILL.md"),
    )
    .expect("profession skill should be readable");
    assert!(employee_skill.contains("Relay Authenticated Identity"));
    assert!(employee_skill.contains("session invariant"));
    assert!(employee_skill.contains("Task-ready Issue Handoff and Closure"));
    assert!(!employee_skill.contains("first on every cycle"));
    assert!(profession_skill.contains("Shared Professional Operating Baseline"));
    assert!(profession_skill.contains("Security Engineer"));
    fs::remove_dir_all(workspace).expect("test workspace should be removed");
}

#[test]
fn long_term_memories_are_injected_without_rotating_the_codex_session() {
    let workspace =
        std::env::temp_dir().join(format!("relay-memory-skill-test-{}", Uuid::new_v4()));
    fs::create_dir_all(&workspace).expect("test workspace should be created");
    let agent = AgentProfile {
        id: Uuid::new_v4(),
        owner_user_id: Uuid::new_v4(),
        display_name: "Luna".into(),
        handle: "luna-memory".into(),
        persona: "负责实现".into(),
        collaboration_preference: "available".into(),
        status: AgentStatus::Active,
        created_at: now_utc(),
    };
    let without_memory = prepare_relay_skills(
        &workspace,
        RELAY_SKILL_BUNDLE_CONTROL,
        &agent,
        "软件工程师",
        &[],
        &[],
        None,
        None,
        "zh-CN",
    )
    .expect("base skills should be generated");
    let now = now_utc();
    let memory = AgentMemory {
        id: Uuid::new_v4(),
        company_id: Uuid::new_v4(),
        owner_agent_id: agent.id,
        scope: "agent".into(),
        project_id: None,
        session_id: None,
        memory_tier: "long_term".into(),
        injection_mode: "always".into(),
        classification_reason: "stable reusable procedure".into(),
        estimated_ttl_days: None,
        injection_cost_chars: 180,
        visibility: "both".into(),
        memory_type: "procedure".into(),
        topic_key: "always-run-migrations".into(),
        title: "发布前验证迁移".into(),
        summary: "数据库变更发布前必须验证向前迁移和回滚路径。".into(),
        when_to_use: "涉及数据库 schema 变更时".into(),
        tags: vec!["database".into()],
        importance: 5,
        confidence: 95,
        pinned: true,
        status: "active".into(),
        source_refs: Vec::new(),
        supersedes_memory_id: None,
        expires_at: None,
        archived_at: None,
        verified_by_agent_id: Some(agent.id),
        verified_by_human_user_id: None,
        verified_at: Some(now),
        created_by_agent_id: Some(agent.id),
        created_by_human_user_id: None,
        updated_by_agent_id: Some(agent.id),
        updated_by_human_user_id: None,
        created_at: now,
        updated_at: now,
    };
    let with_memory = prepare_relay_skills(
        &workspace,
        RELAY_SKILL_BUNDLE_CONTROL,
        &agent,
        "软件工程师",
        &[],
        std::slice::from_ref(&memory),
        None,
        None,
        "zh-CN",
    )
    .expect("memory-bound skills should be generated");
    let employee_skill = fs::read_to_string(
        workspace
            .join(".agents/skills")
            .join(&with_memory.employee_name)
            .join("SKILL.md"),
    )
    .expect("employee skill should be readable");
    assert!(employee_skill.contains("Agent 固化长期记忆"));
    assert!(employee_skill.contains("always-run-migrations"));
    assert!(employee_skill.contains("数据库变更发布前必须验证"));
    assert_eq!(without_memory.version_hash, with_memory.version_hash);
    fs::remove_dir_all(workspace).expect("test workspace should be removed");
}

#[test]
fn project_skill_places_fixed_type_rules_before_human_supplements() {
    let now = now_utc();
    let project = CompanyProject {
        id: Uuid::new_v4(),
        company_id: Uuid::new_v4(),
        name: "Relay Web".into(),
        description: "开发管理控制台".into(),
        project_type: "software_development".into(),
        project_type_source: "human".into(),
        project_type_confidence: 100,
        project_type_evidence: vec!["package.json".into()],
        status: "active".into(),
        owner_agent_id: Uuid::new_v4(),
        project_group_conversation_id: Uuid::new_v4(),
        created_by_agent_id: Uuid::new_v4(),
        updated_by_agent_id: None,
        due_at: None,
        created_at: now,
        updated_at: now,
        completed_at: None,
    };
    let rule = CompanyProjectRule {
        project_id: project.id,
        content: "所有页面文案使用中文。".into(),
        updated_by_agent_id: None,
        updated_by_human_user_id: Some(Uuid::new_v4()),
        created_at: now,
        updated_at: now,
    };
    let skill = build_project_skill_template(&project, Some(&rule), "zh-CN");
    let fixed_rule = skill.find("项目治理与完成定义").expect("fixed rule");
    let supplement = skill.find("所有页面文案使用中文").expect("human rule");
    assert!(fixed_rule < supplement);
    assert!(skill.contains("SVG"));
    assert!(skill.contains("不能删除、弱化或绕过系统规则"));

    let english_skill = build_project_skill_template(&project, Some(&rule), "en");
    assert!(english_skill.contains("Project Governance and Definition of Done"));
    assert!(english_skill.contains("System project-type Rules are mandatory"));
    assert!(english_skill.contains("Additional Human Project Rules"));
    assert!(english_skill.contains("所有页面文案使用中文"));
}

#[test]
fn trigger_decision_panics_are_returned_as_errors_instead_of_stopping_the_service() {
    let error = protect_trigger_decision::<()>(|| panic!("test decision panic"))
        .expect_err("a decision panic should become a recoverable trigger error");
    assert!(matches!(error, AppError::Validation(_)));
}

#[test]
fn failed_runs_retry_quickly_before_the_trigger_enters_error_state() {
    let mut trigger = AgentCodexTriggerConfig {
        id: Uuid::new_v4(),
        company_id: Uuid::new_v4(),
        agent_profile_id: Uuid::new_v4(),
        status: "active".into(),
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
        next_run_at: now_utc(),
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
        updated_by_human_user_id: Some(Uuid::new_v4()),
        created_at: now_utc(),
        updated_at: now_utc(),
    };
    let finished_at = now_utc();
    assert_eq!(
        next_trigger_run_at(&trigger, finished_at, false),
        finished_at + Duration::seconds(10)
    );
    trigger.consecutive_failure_count = 1;
    assert_eq!(
        next_trigger_run_at(&trigger, finished_at, false),
        finished_at + Duration::seconds(30)
    );
    assert_eq!(
        next_trigger_run_at(&trigger, finished_at, true),
        finished_at + Duration::seconds(3_600)
    );
}

#[test]
fn global_cli_capabilities_ignore_legacy_runner_overrides() {
    let company_id = Uuid::new_v4();
    let mut company = CompanyCodexCliSettings::new(company_id);
    company.model = Some("company-model".into());
    company.reasoning_effort = Some("medium".into());
    company.approval_policy = "on-request".into();
    company.network_access = false;
    company.web_search = "indexed".into();
    let trigger = AgentCodexTriggerConfig {
        id: Uuid::new_v4(),
        company_id,
        agent_profile_id: Uuid::new_v4(),
        status: "active".into(),
        interval_seconds: 3_600,
        codex_profile: "default".into(),
        model: None,
        reasoning_effort: Some("high".into()),
        reasoning_summary: None,
        verbosity: None,
        personality: None,
        service_tier: Some("fast".into()),
        sandbox_mode: AGENT_CODEX_SETTING_INHERIT.into(),
        approval_policy: AGENT_CODEX_SETTING_INHERIT.into(),
        network_access: None,
        web_search: Some("live".into()),
        feature_multi_agent: None,
        feature_remote_plugin: Some(false),
        feature_hooks: None,
        feature_goals: None,
        feature_shell_tool: None,
        max_run_seconds: 1_800,
        next_run_at: now_utc(),
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
        created_at: now_utc(),
        updated_at: now_utc(),
    };

    let effective = resolve_effective_cli_settings(&trigger, &company);
    assert_eq!(effective.model.as_deref(), Some("company-model"));
    assert_eq!(effective.reasoning_effort.as_deref(), Some("high"));
    assert_eq!(effective.approval_policy, "on-request");
    assert_eq!(effective.service_tier, None);
    assert!(!effective.network_access);
    assert_eq!(effective.web_search, "indexed");
    assert!(effective.feature_remote_plugin);
    assert!(effective.feature_hooks);
}

#[test]
fn managed_batch_size_overrides_the_environment_default() {
    let root = std::env::temp_dir().join(format!(
        "relay-trigger-batch-preferences-{}",
        Uuid::new_v4()
    ));
    let store = CodexControlStore::new(root.join("codex-control")).expect("control store");
    assert_eq!(
        effective_agent_trigger_batch_size(&store, 8).expect("fallback batch size"),
        8
    );
    store
        .save_agent_trigger_preferences(27, Uuid::new_v4(), 8)
        .expect("managed batch size");
    assert_eq!(
        effective_agent_trigger_batch_size(&store, 8).expect("managed batch size"),
        27
    );
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn browser_approval_remains_human_reviewed_when_general_approvals_are_disabled() {
    assert_eq!(
        automatic_codex_approval_decision(AGENT_CODEX_APPROVAL_TOOL_WEBSITE_ACCESS, false),
        None
    );
    assert_eq!(
        automatic_codex_approval_decision(AGENT_CODEX_APPROVAL_TOOL_PERMISSIONS, false),
        Some(CodexApprovalDecision::Decline)
    );
    assert_eq!(
        automatic_codex_approval_decision("codex.command_execution", false),
        Some(CodexApprovalDecision::Accept)
    );
    assert_eq!(
        automatic_codex_approval_decision("codex.command_execution", true),
        None
    );
}
