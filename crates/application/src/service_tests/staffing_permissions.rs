use super::*;

#[test]
fn staffing_permissions_gate_hire_suspend_and_terminate() {
    let app = PlatformApp::new(MemoryPlatformRepository::default());
    let owner = app
        .dev_login(DevLoginInput {
            email: "staffing-owner@example.com".into(),
            display_name: "Staffing Owner".into(),
        })
        .expect("owner should be created");
    let company = app
        .create_company(CreateCompanyInput {
            human_user_id: owner.id,
            name: "Staffing Company".into(),
            slug: Some("staffing-company".into()),
            description: None,
        })
        .expect("company should be created");
    let manager = app
        .create_company_agent(CreateCompanyAgentInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            display_name: "Staffing Manager".into(),
            handle: "staffing-manager".into(),
            persona: "负责公司人员管理".into(),
            org_unit_id: None,
            job_title: None,
            role_key: Some(COMPANY_AGENT_ROLE_MANAGER.into()),
            reports_to_membership_id: None,
        })
        .expect("manager should be created");

    let denied = app
        .hire_company_agent(AgentStaffingHireInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            display_name: "Provisioning Engineer".into(),
            handle: "provisioning-engineer".into(),
            persona: "负责工程实现".into(),
            org_unit_id: None,
            job_title: Some("工程师".into()),
            reports_to_membership_id: None,
            reason: Some("项目扩编".into()),
            idempotency_key: Some("hire-engineer-1".into()),
        })
        .expect_err("hire must require explicit permission");
    assert!(matches!(denied, AppError::Unauthorized(_)));

    let updated = app
        .update_company_agent_staffing_permissions(UpdateCompanyAgentPermissionsInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            agent_id: manager.agent_profile.id,
            staffing_permissions: vec![
                COMPANY_PERMISSION_STAFF_HIRE.into(),
                COMPANY_PERMISSION_STAFF_SUSPEND.into(),
                COMPANY_PERMISSION_STAFF_TERMINATE.into(),
            ],
            project_permissions: Vec::new(),
            staffing_scope_org_unit_id: None,
            reason: Some("授权管理 Agent 扩招和裁撤".into()),
        })
        .expect("owner should grant staffing permissions");
    assert!(updated
        .permissions
        .iter()
        .any(|permission| permission == COMPANY_PERMISSION_STAFF_HIRE));

    let hired = app
        .hire_company_agent(AgentStaffingHireInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            display_name: "Provisioning Engineer".into(),
            handle: "provisioning-engineer".into(),
            persona: "负责工程实现".into(),
            org_unit_id: None,
            job_title: Some("工程师".into()),
            reports_to_membership_id: None,
            reason: Some("项目扩编".into()),
            idempotency_key: Some("hire-engineer-1".into()),
        })
        .expect("authorized manager should hire");
    assert_eq!(hired.membership.employment_status, "provisioning");
    assert!(matches!(
        hired.agent_profile.status,
        AgentStatus::PendingVerification
    ));
    assert_eq!(
        hired.membership.created_by_agent_id,
        Some(manager.agent_profile.id)
    );
    assert!(!hired.membership.permissions.iter().any(|permission| {
        matches!(
            permission.as_str(),
            COMPANY_PERMISSION_STAFF_HIRE
                | COMPANY_PERMISSION_STAFF_SUSPEND
                | COMPANY_PERMISSION_STAFF_TERMINATE
        )
    }));

    let activated = app
        .activate_provisioned_company_agent(HumanCompanyStaffingStatusInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            target_agent_id: hired.agent_profile.id,
            reason: Some("Runtime 已绑定".into()),
            handoff_plan: None,
            handoff_agent_id: None,
        })
        .expect("human should activate provisioning agent");
    assert_eq!(activated.membership.employment_status, "active");
    assert_eq!(
        app.authenticate_agent_key(&activated.agent_key_plaintext)
            .expect("activated key should authenticate")
            .id,
        hired.agent_profile.id
    );

    let suspended = app
        .suspend_company_agent_as_agent(AgentStaffingStatusInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            target_agent_id: hired.agent_profile.id,
            reason: Some("暂停等待资源".into()),
            handoff_plan: None,
            handoff_agent_id: None,
            idempotency_key: Some("suspend-engineer-1".into()),
        })
        .expect("authorized manager should suspend member");
    assert_eq!(suspended.membership.employment_status, "suspended");
    assert_eq!(suspended.revoked_key_count, 1);
    assert!(app
        .authenticate_agent_key(&activated.agent_key_plaintext)
        .is_err());

    let reactivated = app
        .reactivate_company_agent(HumanCompanyStaffingStatusInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            target_agent_id: hired.agent_profile.id,
            reason: Some("恢复工作".into()),
            handoff_plan: None,
            handoff_agent_id: None,
        })
        .expect("human should reactivate suspended agent");
    assert!(app
        .authenticate_agent_key(&reactivated.agent_key_plaintext)
        .is_ok());

    let self_terminate = app
        .terminate_company_agent_as_agent(AgentStaffingStatusInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            target_agent_id: manager.agent_profile.id,
            reason: Some("self terminate".into()),
            handoff_plan: Some("交给 Owner".into()),
            handoff_agent_id: None,
            idempotency_key: Some("self-terminate".into()),
        })
        .expect_err("agent must not terminate itself");
    assert!(matches!(self_terminate, AppError::Unauthorized(_)));

    let terminated = app
        .terminate_company_agent_as_agent(AgentStaffingStatusInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            target_agent_id: hired.agent_profile.id,
            reason: Some("项目结束".into()),
            handoff_plan: Some("当前上下文和后续事项交还 Staffing Manager".into()),
            handoff_agent_id: None,
            idempotency_key: Some("terminate-engineer-1".into()),
        })
        .expect("authorized manager should terminate member");
    assert_eq!(terminated.membership.employment_status, "terminated");
    assert!(terminated.membership.terminated_at.is_some());
    assert!(app
        .authenticate_agent_key(&reactivated.agent_key_plaintext)
        .is_err());

    let actions = app
        .list_company_staffing_actions_for_human(owner.id, company.company.id)
        .expect("owner should list staffing audit");
    assert!(actions
        .iter()
        .any(|action| action.action_type == STAFFING_ACTION_HIRE));
    assert!(actions
        .iter()
        .any(|action| action.action_type == STAFFING_ACTION_TERMINATE));

    let last_manager = app
        .terminate_company_agent_as_human(HumanCompanyStaffingStatusInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            target_agent_id: manager.agent_profile.id,
            reason: Some("close company".into()),
            handoff_plan: Some("由 Owner 接管".into()),
            handoff_agent_id: None,
        })
        .expect_err("last active manager must be protected");
    assert!(matches!(last_manager, AppError::Conflict(_)));
}
