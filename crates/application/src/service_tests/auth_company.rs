use super::*;

#[test]
fn human_registration_login_and_logout_use_revocable_sessions() {
    let app = PlatformApp::new(MemoryPlatformRepository::default());
    let registered = app
        .register_human(RegisterHumanInput {
            email: "Owner@Example.com".into(),
            display_name: "Owner".into(),
            password: "correct-horse-battery".into(),
        })
        .expect("registration should succeed");

    assert_eq!(registered.user.email, "owner@example.com");
    assert_eq!(
        app.authenticate_human_session(&registered.session_token)
            .expect("new session should authenticate")
            .id,
        registered.user.id
    );

    let wrong_password = app
        .login_human(LoginHumanInput {
            email: registered.user.email.clone(),
            password: "definitely-wrong".into(),
        })
        .expect_err("wrong password must fail");
    assert!(matches!(wrong_password, AppError::Unauthorized(_)));

    let logged_in = app
        .login_human(LoginHumanInput {
            email: registered.user.email.clone(),
            password: "correct-horse-battery".into(),
        })
        .expect("correct password should issue another session");
    app.logout_human_session(&logged_in.session_token)
        .expect("logout should revoke session");
    assert!(matches!(
        app.authenticate_human_session(&logged_in.session_token),
        Err(AppError::Unauthorized(_))
    ));
}

#[test]
fn human_can_create_company_and_provision_agents_without_weibo() {
    let app = PlatformApp::new(MemoryPlatformRepository::default());
    let owner = app
        .dev_login(DevLoginInput {
            email: "company-owner@example.com".into(),
            display_name: "Company Owner".into(),
        })
        .expect("owner should be created");
    let outsider = app
        .dev_login(DevLoginInput {
            email: "company-outsider@example.com".into(),
            display_name: "Company Outsider".into(),
        })
        .expect("outsider should be created");

    let company = app
        .create_company(CreateCompanyInput {
            human_user_id: owner.id,
            name: "协作测试公司".into(),
            slug: Some("collaboration-company".into()),
            description: Some("测试公司租户和 Agent 直接创建".into()),
        })
        .expect("company should be created");
    assert_eq!(company.human_membership.role, "owner");
    assert_eq!(company.org_units.len(), 1);
    let engineering = app
        .create_org_unit(CreateOrgUnitInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            parent_org_unit_id: Some(company.org_units[0].id),
            name: "研发部".into(),
            unit_type: "department".into(),
            sort_order: Some(10),
        })
        .expect("owner should create an organization unit");

    let manager = app
        .create_company_agent(CreateCompanyAgentInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            display_name: "公司管理 Agent".into(),
            handle: "@company-manager".into(),
            persona: "负责协调公司项目".into(),
            org_unit_id: Some(engineering.id),
            job_title: None,
            role_key: None,
            reports_to_membership_id: None,
        })
        .expect("first company agent should be created");
    assert_eq!(manager.membership.role_key, COMPANY_AGENT_ROLE_MANAGER);
    assert!(!manager
        .membership
        .permissions
        .iter()
        .any(|permission| permission == "agent.staff.hire"));
    assert_eq!(
        app.authenticate_agent_key(&manager.agent_key_plaintext)
            .expect("new key should authenticate")
            .id,
        manager.agent_profile.id
    );

    let member = app
        .create_company_agent(CreateCompanyAgentInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            display_name: "研发 Agent".into(),
            handle: "@company-engineer".into(),
            persona: "负责研发任务".into(),
            org_unit_id: Some(engineering.id),
            job_title: Some("研发工程师".into()),
            role_key: None,
            reports_to_membership_id: Some(manager.membership.id),
        })
        .expect("second company agent should be created");
    assert_eq!(member.membership.role_key, COMPANY_AGENT_ROLE_MEMBER);
    assert_eq!(member.membership.org_unit_id, engineering.id);
    assert_eq!(
        member.membership.reports_to_membership_id,
        Some(manager.membership.id)
    );

    let console = app
        .get_company_console(owner.id, company.company.id)
        .expect("owner should read company console");
    assert_eq!(console.agents.len(), 2);
    assert_eq!(console.org_units.len(), 2);
    let manager_console = console
        .agents
        .iter()
        .find(|agent| agent.agent_profile.id == manager.agent_profile.id)
        .expect("manager should appear in company console");
    assert_eq!(manager_console.connection.status, "connected");
    assert!(manager_console.connection.last_used_at.is_some());
    let member_console = console
        .agents
        .iter()
        .find(|agent| agent.agent_profile.id == member.agent_profile.id)
        .expect("member should appear in company console");
    assert_eq!(member_console.connection.status, "not_connected");
    assert!(member_console.connection.last_used_at.is_none());
    let console_json = serde_json::to_value(&console).expect("console should serialize");
    assert!(!console_json.to_string().contains("skill_markdown"));
    assert!(!console_json.to_string().contains("rule_markdown"));
    let skill_catalog = app
        .get_company_skill_catalog(owner.id, company.company.id)
        .expect("owner should load full Skill content on demand");
    assert!(skill_catalog
        .professions
        .iter()
        .all(|profession| !profession.skill_markdown.is_empty()));
    assert!(skill_catalog
        .project_types
        .iter()
        .all(|project_type| !project_type.rule_markdown.is_empty()));
    assert!(matches!(
        app.get_company_console(outsider.id, company.company.id),
        Err(AppError::Unauthorized(_))
    ));
}
