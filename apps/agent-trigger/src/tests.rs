use std::{collections::HashMap, path::PathBuf};

use ai_chat_domain::agent_identity::AgentStatus;

use super::*;

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
        "relay-skills-v8:project/agent"
    );
    assert!(codex_session_key_matches(
        "relay-skills-v8:project/agent:old-skill-hash:old-plugin-hash",
        "relay-skills-v8:project/agent"
    ));
    assert!(!codex_session_key_matches(
        "relay-skills-v8:another-project/agent:old-skill-hash:old-plugin-hash",
        "relay-skills-v8:project/agent"
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
fn relay_skills_are_materialized_in_the_codex_repo_skill_location() {
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
    assert!(workspace
        .join(".agents/skills")
        .join(&prepared.profession_name)
        .join("SKILL.md")
        .is_file());
    assert!(!prepared.version_hash.is_empty());
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
    assert!(employee_skill.contains("Relay Account Binding"));
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
        memory_tier: "long_term".into(),
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
fn runner_overrides_resolve_without_losing_company_defaults() {
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
    assert!(!effective.network_access);
    assert_eq!(effective.web_search, "live");
    assert!(!effective.feature_remote_plugin);
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
