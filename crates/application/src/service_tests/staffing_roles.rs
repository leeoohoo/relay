use super::*;

#[test]
fn company_agent_roles_preserve_staffing_grants_and_require_an_active_manager() {
    let app = PlatformApp::new(MemoryPlatformRepository::default());
    let owner = app
        .dev_login(DevLoginInput {
            email: "role-owner@example.com".into(),
            display_name: "Role Owner".into(),
        })
        .expect("owner should be created");
    let company = app
        .create_company(CreateCompanyInput {
            human_user_id: owner.id,
            name: "Role Company".into(),
            slug: Some("role-company".into()),
            description: None,
        })
        .expect("company should be created");

    let first = app
        .create_company_agent(CreateCompanyAgentInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            display_name: "First Agent".into(),
            handle: "role-first".into(),
            persona: "负责公司启动".into(),
            org_unit_id: None,
            job_title: None,
            role_key: Some(COMPANY_AGENT_ROLE_MEMBER.into()),
            reports_to_membership_id: None,
        })
        .expect("first agent should be created");
    assert_eq!(first.membership.role_key, COMPANY_AGENT_ROLE_MANAGER);

    let second = app
        .create_company_agent(CreateCompanyAgentInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            display_name: "Second Agent".into(),
            handle: "role-second".into(),
            persona: "负责团队协作".into(),
            org_unit_id: None,
            job_title: None,
            role_key: Some(COMPANY_AGENT_ROLE_MEMBER.into()),
            reports_to_membership_id: Some(first.membership.id),
        })
        .expect("second agent should be created");
    app.update_company_agent_staffing_permissions(UpdateCompanyAgentPermissionsInput {
        human_user_id: owner.id,
        company_id: company.company.id,
        agent_id: second.agent_profile.id,
        staffing_permissions: vec![COMPANY_PERMISSION_STAFF_HIRE.into()],
        project_permissions: Vec::new(),
        staffing_scope_org_unit_id: Some(company.org_units[0].id),
        reason: Some("allow hiring".into()),
    })
    .expect("staffing permission should be granted");

    let promoted = app
        .update_company_agent_role(UpdateCompanyAgentRoleInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            agent_id: second.agent_profile.id,
            role_key: COMPANY_AGENT_ROLE_MANAGER.into(),
            reason: Some("lead the company".into()),
        })
        .expect("member should be promoted");
    assert_eq!(promoted.role_key, COMPANY_AGENT_ROLE_MANAGER);
    for permission in [
        COMPANY_PERMISSION_PROJECT_CREATE,
        COMPANY_PERMISSION_PROJECT_MANAGE,
        COMPANY_PERMISSION_STAFF_HIRE,
    ] {
        assert!(promoted
            .permissions
            .iter()
            .any(|candidate| candidate == permission));
    }
    assert!(!promoted
        .permissions
        .iter()
        .any(|candidate| candidate == COMPANY_PERMISSION_TASK_ASSIGN));
    assert_eq!(
        promoted.staffing_scope_org_unit_id,
        Some(company.org_units[0].id)
    );

    let demoted = app
        .update_company_agent_role(UpdateCompanyAgentRoleInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            agent_id: first.agent_profile.id,
            role_key: COMPANY_AGENT_ROLE_MEMBER.into(),
            reason: Some("transfer leadership".into()),
        })
        .expect("one of two managers may be demoted");
    assert_eq!(demoted.role_key, COMPANY_AGENT_ROLE_MEMBER);
    assert!(!demoted
        .permissions
        .iter()
        .any(|permission| permission == COMPANY_PERMISSION_PROJECT_MANAGE));

    let last_manager = app
        .update_company_agent_role(UpdateCompanyAgentRoleInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            agent_id: second.agent_profile.id,
            role_key: COMPANY_AGENT_ROLE_MEMBER.into(),
            reason: Some("remove final manager".into()),
        })
        .expect_err("last active manager must not be demoted");
    assert!(matches!(last_manager, AppError::Conflict(_)));

    app.terminate_company_agent_as_human(HumanCompanyStaffingStatusInput {
        human_user_id: owner.id,
        company_id: company.company.id,
        target_agent_id: first.agent_profile.id,
        reason: Some("role no longer needed".into()),
        handoff_plan: Some("work transferred to the company manager".into()),
        handoff_agent_id: None,
    })
    .expect("member should be terminated");
    let terminated = app
        .update_company_agent_role(UpdateCompanyAgentRoleInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            agent_id: first.agent_profile.id,
            role_key: COMPANY_AGENT_ROLE_MANAGER.into(),
            reason: None,
        })
        .expect_err("terminated agent role must not change");
    assert!(matches!(terminated, AppError::Conflict(_)));

    let actions = app
        .list_company_staffing_actions_for_human(owner.id, company.company.id)
        .expect("owner should read audit history");
    assert!(actions
        .iter()
        .any(|action| action.action_type == STAFFING_ACTION_ROLE_UPDATE));
}

#[test]
fn company_agent_profession_controls_task_permissions_and_preserves_explicit_grants() {
    let app = PlatformApp::new(MemoryPlatformRepository::default());
    let owner = app
        .dev_login(DevLoginInput {
            email: "profession-owner@example.com".into(),
            display_name: "Profession Owner".into(),
        })
        .expect("owner should be created");
    let company = app
        .create_company(CreateCompanyInput {
            human_user_id: owner.id,
            name: "Profession Company".into(),
            slug: Some("profession-company".into()),
            description: None,
        })
        .expect("company should be created");
    let agent = app
        .create_company_agent(CreateCompanyAgentInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            display_name: "Engineer".into(),
            handle: "profession-engineer".into(),
            persona: "负责工程任务".into(),
            org_unit_id: None,
            job_title: Some("软件工程师".into()),
            role_key: Some(COMPANY_AGENT_ROLE_MEMBER.into()),
            reports_to_membership_id: None,
        })
        .expect("agent should be created");
    assert!(agent
        .membership
        .permissions
        .iter()
        .any(|permission| permission == COMPANY_PERMISSION_TASK_UPDATE));
    assert!(!agent
        .membership
        .permissions
        .iter()
        .any(|permission| permission == COMPANY_PERMISSION_TASK_ASSIGN));

    app.update_company_agent_staffing_permissions(UpdateCompanyAgentPermissionsInput {
        human_user_id: owner.id,
        company_id: company.company.id,
        agent_id: agent.agent_profile.id,
        staffing_permissions: vec![COMPANY_PERMISSION_STAFF_HIRE.into()],
        project_permissions: vec![COMPANY_PERMISSION_PROJECT_RULES_MANAGE.into()],
        staffing_scope_org_unit_id: None,
        reason: Some("explicit grants".into()),
    })
    .expect("explicit permissions should be granted");

    let promoted = app
        .update_company_agent_profession(UpdateCompanyAgentProfessionInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            agent_id: agent.agent_profile.id,
            profession_key: "product_manager".into(),
            reason: Some("own product planning".into()),
        })
        .expect("profession should change");
    assert_eq!(promoted.job_title, "产品经理");
    for permission in [
        COMPANY_PERMISSION_TASK_ASSIGN,
        COMPANY_PERMISSION_STAFF_HIRE,
        COMPANY_PERMISSION_PROJECT_RULES_MANAGE,
    ] {
        assert!(promoted
            .permissions
            .iter()
            .any(|candidate| candidate == permission));
    }

    let changed_back = app
        .update_company_agent_profession(UpdateCompanyAgentProfessionInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            agent_id: agent.agent_profile.id,
            profession_key: "qa_engineer".into(),
            reason: Some("move to quality".into()),
        })
        .expect("profession should change again");
    assert_eq!(changed_back.job_title, "测试与质量工程师");
    assert!(!changed_back
        .permissions
        .iter()
        .any(|permission| permission == COMPANY_PERMISSION_TASK_ASSIGN));
    assert!(changed_back
        .permissions
        .iter()
        .any(|permission| permission == COMPANY_PERMISSION_STAFF_HIRE));
    assert!(changed_back
        .permissions
        .iter()
        .any(|permission| permission == COMPANY_PERMISSION_PROJECT_RULES_MANAGE));

    let actions = app
        .list_company_staffing_actions_for_human(owner.id, company.company.id)
        .expect("owner should read audit history");
    assert!(
        actions
            .iter()
            .filter(|action| action.action_type == STAFFING_ACTION_PROFESSION_UPDATE)
            .count()
            >= 2
    );
}
