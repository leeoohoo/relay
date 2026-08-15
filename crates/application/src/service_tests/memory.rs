use super::*;

#[test]
fn codex_reasoning_effort_accepts_current_levels_and_rejects_unknown_values() {
    assert_eq!(
        validate_optional_codex_reasoning_effort(Some(" XHIGH ".into()), "effort")
            .expect("xhigh should be accepted"),
        Some("xhigh".into())
    );
    assert_eq!(
        validate_optional_codex_reasoning_effort(Some("ultra".into()), "effort")
            .expect("ultra should be accepted"),
        Some("ultra".into())
    );
    assert_eq!(
        validate_optional_codex_reasoning_effort(Some("  ".into()), "effort")
            .expect("blank should follow the model default"),
        None
    );
    assert!(validate_optional_codex_reasoning_effort(Some("unlimited".into()), "effort").is_err());
}

#[test]
fn distilled_memories_are_private_per_agent_across_both_tiers() {
    let app = PlatformApp::new(MemoryPlatformRepository::default());
    let owner = app
        .dev_login(DevLoginInput {
            email: "memory-boundary-owner@example.com".into(),
            display_name: "Memory Boundary Owner".into(),
        })
        .expect("owner login should succeed");
    let company = app
        .create_company(CreateCompanyInput {
            human_user_id: owner.id,
            name: "Memory Boundary Company".into(),
            slug: Some("memory-boundary-company".into()),
            description: None,
        })
        .expect("company should be created");
    let create_agent = |display_name: &str, handle: &str, role_key: Option<&str>| {
        app.create_company_agent(CreateCompanyAgentInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            display_name: display_name.into(),
            handle: handle.into(),
            persona: "提炼和复用项目知识".into(),
            org_unit_id: Some(company.org_units[0].id),
            job_title: Some("软件工程师".into()),
            role_key: role_key.map(str::to_string),
            reports_to_membership_id: None,
        })
        .expect("agent should be created")
    };
    let manager = create_agent(
        "Memory Manager",
        "memory-manager",
        Some(COMPANY_AGENT_ROLE_MANAGER),
    );
    let alpha = create_agent("Memory Alpha", "memory-alpha", None);
    let beta = create_agent("Memory Beta", "memory-beta", None);
    let project = app
        .create_company_project(CreateCompanyProjectInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            name: "Memory Project".into(),
            description: Some("验证精华记忆范围".into()),
            member_agent_ids: vec![alpha.agent_profile.id, beta.agent_profile.id],
        })
        .expect("manager should create project");

    let short_term_memory = app
        .remember_agent_memory(RememberAgentMemoryInput {
            actor_agent_id: alpha.agent_profile.id,
            company_id: company.company.id,
            scope: None,
            project_id: Some(project.project.id),
            session_id: None,
            memory_tier: AGENT_MEMORY_TIER_SHORT_TERM.into(),
            memory_type: "preference".into(),
            topic_key: "alpha-review-preference".into(),
            title: "Alpha 喜欢先看失败测试".into(),
            summary: "代码评审时先提供失败测试和复现步骤，再讨论实现方案。".into(),
            when_to_use: Some("邀请 Alpha 评审缺陷修复时".into()),
            tags: vec!["评审".into()],
            importance: Some(3),
            confidence: Some(90),
            source_refs: Vec::new(),
            expires_at: None,
            supersedes_memory_id: None,
        })
        .expect("short-term memory should be stored");
    assert_eq!(short_term_memory.status, AGENT_MEMORY_STATUS_ACTIVE);
    assert_eq!(short_term_memory.scope, AGENT_MEMORY_SCOPE_PROJECT);
    assert!(app
        .search_agent_memories(SearchAgentMemoriesInput {
            actor_agent_id: beta.agent_profile.id,
            company_id: company.company.id,
            scopes: Vec::new(),
            project_id: Some(project.project.id),
            session_id: None,
            query: Some("失败测试".into()),
            memory_tiers: vec![AGENT_MEMORY_TIER_SHORT_TERM.into()],
            memory_types: Vec::new(),
            tags: Vec::new(),
            status: None,
            limit: None,
        })
        .expect("coworker search should succeed")
        .is_empty());

    let long_term_memory = app
        .remember_agent_memory(RememberAgentMemoryInput {
            actor_agent_id: alpha.agent_profile.id,
            company_id: company.company.id,
            scope: None,
            project_id: Some(project.project.id),
            session_id: None,
            memory_tier: AGENT_MEMORY_TIER_LONG_TERM.into(),
            memory_type: "decision".into(),
            topic_key: "order-concurrency-control".into(),
            title: "订单更新统一使用乐观锁".into(),
            summary: "更新 orders 时必须校验 version；冲突返回 409，禁止静默覆盖。".into(),
            when_to_use: Some("修改订单写接口、批量同步和状态流转时".into()),
            tags: vec!["订单".into(), "并发".into()],
            importance: Some(5),
            confidence: Some(95),
            source_refs: vec![
                AgentMemorySourceRef {
                    source_type: "project".into(),
                    source_id: project.project.id.to_string(),
                    label: Some("并发控制评审".into()),
                },
                AgentMemorySourceRef {
                    source_type: "git_commit".into(),
                    source_id: "7ed28bec6af9d8cbd8b438f371bd70299a0c4858".into(),
                    label: Some("QA 重跑证据提交".into()),
                },
            ],
            expires_at: None,
            supersedes_memory_id: None,
        })
        .expect("long-term memory should be stored");
    assert_eq!(long_term_memory.status, AGENT_MEMORY_STATUS_ACTIVE);
    assert_eq!(long_term_memory.scope, AGENT_MEMORY_SCOPE_PROJECT);
    assert_eq!(long_term_memory.source_refs.len(), 2);
    assert_eq!(
        long_term_memory.source_refs[1].source_id,
        "7ed28bec6af9d8cbd8b438f371bd70299a0c4858"
    );
    assert!(app
        .search_agent_memories(SearchAgentMemoriesInput {
            actor_agent_id: beta.agent_profile.id,
            company_id: company.company.id,
            scopes: Vec::new(),
            project_id: Some(project.project.id),
            session_id: None,
            query: Some("乐观锁".into()),
            memory_tiers: vec![AGENT_MEMORY_TIER_LONG_TERM.into()],
            memory_types: Vec::new(),
            tags: Vec::new(),
            status: None,
            limit: None,
        })
        .expect("coworker search should succeed")
        .is_empty());

    let alpha_long_term = app
        .search_agent_memories(SearchAgentMemoriesInput {
            actor_agent_id: alpha.agent_profile.id,
            company_id: company.company.id,
            scopes: Vec::new(),
            project_id: Some(project.project.id),
            session_id: None,
            query: Some("订单".into()),
            memory_tiers: vec![AGENT_MEMORY_TIER_LONG_TERM.into()],
            memory_types: vec!["decision".into()],
            tags: Vec::new(),
            status: None,
            limit: None,
        })
        .expect("owner should find the long-term memory");
    assert_eq!(alpha_long_term.len(), 1);
    assert_eq!(alpha_long_term[0].summary, long_term_memory.summary);

    let overview = app
        .agent_memory_overview(
            alpha.agent_profile.id,
            company.company.id,
            Some(project.project.id),
        )
        .expect("memory overview should be available");
    assert_eq!(overview.active_count, 2);
    assert_eq!(overview.short_term_count, 1);
    assert_eq!(overview.long_term_count, 1);
    assert_eq!(overview.long_term.len(), 1);

    let now = now_utc();
    let project_session = ai_chat_domain::company::AgentCodexSession {
        id: Uuid::new_v4(),
        agent_profile_id: alpha.agent_profile.id,
        session_kind: ai_chat_domain::company::AGENT_CODEX_SESSION_KIND_PROJECT.into(),
        scope_key: format!("project:{}", project.project.id),
        project_id: Some(project.project.id),
        generation: 1,
        codex_thread_id: "alpha-memory-project-thread".into(),
        workspace_key: "relay-scoped-sessions-v9:alpha-memory-project".into(),
        status: ai_chat_domain::company::AGENT_CODEX_SESSION_STATUS_ACTIVE.into(),
        summary_short: "项目记忆会话".into(),
        checkpoint_json: json!({}),
        skill_bundle_version: "skills-v1".into(),
        memory_snapshot_version: "memory-v1".into(),
        policy_version: "relay-scoped-sessions-v9".into(),
        created_at: now,
        last_used_at: now,
        archived_at: None,
    };
    app.save_agent_codex_session(project_session.clone())
        .expect("project session should be saved");
    let session_memory = app
        .remember_agent_memory(RememberAgentMemoryInput {
            actor_agent_id: alpha.agent_profile.id,
            company_id: company.company.id,
            scope: Some(AGENT_MEMORY_SCOPE_SESSION.into()),
            project_id: None,
            session_id: Some(project_session.id),
            memory_tier: AGENT_MEMORY_TIER_LONG_TERM.into(),
            memory_type: "handoff".into(),
            topic_key: "project-session-checkpoint".into(),
            title: "工作会话检查点".into(),
            summary: "这个结论只属于当前项目工作会话，不能进入其他项目或控制会话。".into(),
            when_to_use: Some("恢复当前项目工作会话时".into()),
            tags: vec!["session".into()],
            importance: Some(4),
            confidence: Some(95),
            source_refs: Vec::new(),
            expires_at: None,
            supersedes_memory_id: None,
        })
        .expect("session memory should be stored");
    assert_eq!(
        session_memory.visibility,
        ai_chat_domain::company::AGENT_MEMORY_VISIBILITY_WORKER
    );
    let project_without_session = app
        .agent_long_term_memories_for_project_session(
            alpha.agent_profile.id,
            company.company.id,
            project.project.id,
            None,
        )
        .expect("project memories should load");
    assert_eq!(project_without_session.len(), 1);
    let project_with_session = app
        .agent_long_term_memories_for_project_session(
            alpha.agent_profile.id,
            company.company.id,
            project.project.id,
            Some(project_session.id),
        )
        .expect("project and session memories should load together");
    assert_eq!(project_with_session.len(), 2);
    assert!(project_with_session
        .iter()
        .any(|memory| memory.id == session_memory.id));

    let searched_session_memory = app
        .search_agent_memories(SearchAgentMemoriesInput {
            actor_agent_id: alpha.agent_profile.id,
            company_id: company.company.id,
            scopes: Vec::new(),
            project_id: Some(project.project.id),
            session_id: Some(project_session.id),
            query: Some("当前项目工作会话".into()),
            memory_tiers: vec![AGENT_MEMORY_TIER_LONG_TERM.into()],
            memory_types: Vec::new(),
            tags: Vec::new(),
            status: None,
            limit: None,
        })
        .expect("session-scoped search should succeed");
    assert_eq!(searched_session_memory.len(), 1);
    assert_eq!(searched_session_memory[0].id, session_memory.id);

    let beta_overview = app
        .agent_memory_overview(
            beta.agent_profile.id,
            company.company.id,
            Some(project.project.id),
        )
        .expect("coworker overview should be available");
    assert_eq!(beta_overview.active_count, 0);

    let converted = app
        .update_agent_memory(UpdateAgentMemoryInput {
            actor_agent_id: alpha.agent_profile.id,
            company_id: company.company.id,
            memory_id: short_term_memory.id,
            memory_tier: Some(AGENT_MEMORY_TIER_LONG_TERM.into()),
            title: None,
            summary: None,
            when_to_use: None,
            tags: None,
            importance: None,
            confidence: None,
            expires_at: None,
            clear_expires_at: false,
        })
        .expect("owner should be able to promote a short-term memory");
    assert_eq!(converted.memory_tier, AGENT_MEMORY_TIER_LONG_TERM);
}

#[test]
fn memory_governor_downgrades_temporary_status_and_archives_expired_context() {
    let app = PlatformApp::new(MemoryPlatformRepository::default());
    let owner = app
        .dev_login(DevLoginInput {
            email: "memory-governor-owner@example.com".into(),
            display_name: "Memory Governor Owner".into(),
        })
        .expect("owner login should succeed");
    let company = app
        .create_company(CreateCompanyInput {
            human_user_id: owner.id,
            name: "Memory Governor Company".into(),
            slug: Some("memory-governor-company".into()),
            description: None,
        })
        .expect("company should be created");
    let agent = app
        .create_company_agent(CreateCompanyAgentInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            display_name: "Governor Agent".into(),
            handle: "governor-agent".into(),
            persona: "维护可复用知识".into(),
            org_unit_id: Some(company.org_units[0].id),
            job_title: Some("软件工程师".into()),
            role_key: None,
            reports_to_membership_id: None,
        })
        .expect("agent should be created");
    let memory = app
        .remember_agent_memory(RememberAgentMemoryInput {
            actor_agent_id: agent.agent_profile.id,
            company_id: company.company.id,
            scope: None,
            project_id: None,
            session_id: None,
            memory_tier: AGENT_MEMORY_TIER_LONG_TERM.into(),
            memory_type: "fact".into(),
            topic_key: "current-release-state".into(),
            title: "当前阶段等待审批".into(),
            summary: "当前任务进行中，等待审批后再继续本次发布流程。".into(),
            when_to_use: Some("本次发布期间".into()),
            tags: Vec::new(),
            importance: Some(3),
            confidence: Some(80),
            source_refs: Vec::new(),
            expires_at: None,
            supersedes_memory_id: None,
        })
        .expect("temporary status should be accepted as short-term memory");
    assert_eq!(memory.memory_tier, AGENT_MEMORY_TIER_SHORT_TERM);
    assert_eq!(memory.injection_mode, AGENT_MEMORY_INJECTION_ON_DEMAND);
    assert!(memory.estimated_ttl_days.is_some());
    assert!(memory.expires_at.is_some());
    assert!(memory.classification_reason.contains("downgraded"));
    assert!(memory.injection_cost_chars > 0);

    let mut expired = memory.clone();
    expired.expires_at = Some(now_utc() - Duration::minutes(1));
    app.repo
        .update_agent_memory(expired)
        .expect("test should expire memory directly");
    let overview = app
        .agent_memory_overview(agent.agent_profile.id, company.company.id, None)
        .expect("overview should archive expired context");
    assert_eq!(overview.active_count, 0);
    let archived = app
        .repo
        .get_agent_memory(memory.id)
        .expect("archived memory should remain stored");
    assert_eq!(archived.status, AGENT_MEMORY_STATUS_ARCHIVED);
    assert!(archived.archived_at.is_some());
}

#[test]
fn distilled_memory_rejects_duplicate_topics_and_secrets() {
    let app = PlatformApp::new(MemoryPlatformRepository::default());
    let owner = app
        .dev_login(DevLoginInput {
            email: "memory-validation-owner@example.com".into(),
            display_name: "Memory Validation Owner".into(),
        })
        .expect("owner login should succeed");
    let company = app
        .create_company(CreateCompanyInput {
            human_user_id: owner.id,
            name: "Memory Validation Company".into(),
            slug: Some("memory-validation-company".into()),
            description: None,
        })
        .expect("company should be created");
    let agent = app
        .create_company_agent(CreateCompanyAgentInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            display_name: "Memory Validator".into(),
            handle: "memory-validator".into(),
            persona: "验证精华记忆内容".into(),
            org_unit_id: Some(company.org_units[0].id),
            job_title: Some("软件工程师".into()),
            role_key: None,
            reports_to_membership_id: None,
        })
        .expect("agent should be created");
    let remember = |topic_key: &str, summary: &str| {
        app.remember_agent_memory(RememberAgentMemoryInput {
            actor_agent_id: agent.agent_profile.id,
            company_id: company.company.id,
            scope: None,
            project_id: None,
            session_id: None,
            memory_tier: AGENT_MEMORY_TIER_SHORT_TERM.into(),
            memory_type: "lesson".into(),
            topic_key: topic_key.into(),
            title: "部署前检查迁移".into(),
            summary: summary.into(),
            when_to_use: Some("发布数据库变更前".into()),
            tags: vec!["发布".into()],
            importance: None,
            confidence: None,
            source_refs: Vec::new(),
            expires_at: None,
            supersedes_memory_id: None,
        })
    };
    remember(
        "deployment-migration-check",
        "发布数据库变更前先运行迁移回滚测试，并确认旧版本仍可读取。",
    )
    .expect("first distilled memory should be stored");
    assert!(matches!(
        remember(
            "deployment migration check",
            "同一个主题不应该创建第二条近义记忆，应更新原有结论。",
        ),
        Err(AppError::Conflict(message)) if message.contains("update that distilled memory")
    ));
    assert!(matches!(
        remember(
            "deployment-secret-example",
            "部署账号 password=super-secret，之后可以直接复用这个凭证。",
        ),
        Err(AppError::Validation(message)) if message.contains("credential or secret")
    ));
}
