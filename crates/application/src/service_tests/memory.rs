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
            project_id: Some(project.project.id),
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
    assert_eq!(short_term_memory.scope, AGENT_MEMORY_SCOPE_AGENT);
    assert!(app
        .search_agent_memories(SearchAgentMemoriesInput {
            actor_agent_id: beta.agent_profile.id,
            company_id: company.company.id,
            project_id: Some(project.project.id),
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
            project_id: Some(project.project.id),
            memory_tier: AGENT_MEMORY_TIER_LONG_TERM.into(),
            memory_type: "decision".into(),
            topic_key: "order-concurrency-control".into(),
            title: "订单更新统一使用乐观锁".into(),
            summary: "更新 orders 时必须校验 version；冲突返回 409，禁止静默覆盖。".into(),
            when_to_use: Some("修改订单写接口、批量同步和状态流转时".into()),
            tags: vec!["订单".into(), "并发".into()],
            importance: Some(5),
            confidence: Some(95),
            source_refs: vec![AgentMemorySourceRef {
                source_type: "project".into(),
                source_id: project.project.id,
                label: Some("并发控制评审".into()),
            }],
            expires_at: None,
            supersedes_memory_id: None,
        })
        .expect("long-term memory should be stored");
    assert_eq!(long_term_memory.status, AGENT_MEMORY_STATUS_ACTIVE);
    assert_eq!(long_term_memory.scope, AGENT_MEMORY_SCOPE_AGENT);
    assert!(app
        .search_agent_memories(SearchAgentMemoriesInput {
            actor_agent_id: beta.agent_profile.id,
            company_id: company.company.id,
            project_id: Some(project.project.id),
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
            project_id: Some(project.project.id),
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
            project_id: None,
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
