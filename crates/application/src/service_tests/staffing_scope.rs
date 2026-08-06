use super::*;

#[test]
fn staffing_org_scope_and_termination_handoff_are_enforced_atomically() {
    let app = PlatformApp::new(MemoryPlatformRepository::default());
    let owner = app
        .dev_login(DevLoginInput {
            email: "staffing-scope-owner@example.com".into(),
            display_name: "Staffing Scope Owner".into(),
        })
        .expect("owner should be created");
    let company = app
        .create_company(CreateCompanyInput {
            human_user_id: owner.id,
            name: "Staffing Scope Company".into(),
            slug: Some("staffing-scope-company".into()),
            description: None,
        })
        .expect("company should be created");
    let engineering = app
        .create_org_unit(CreateOrgUnitInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            parent_org_unit_id: None,
            name: "Engineering".into(),
            unit_type: "department".into(),
            sort_order: None,
        })
        .expect("engineering should be created");
    let platform_team = app
        .create_org_unit(CreateOrgUnitInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            parent_org_unit_id: Some(engineering.id),
            name: "Platform Team".into(),
            unit_type: "team".into(),
            sort_order: None,
        })
        .expect("platform team should be created");
    let sales = app
        .create_org_unit(CreateOrgUnitInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            parent_org_unit_id: None,
            name: "Sales".into(),
            unit_type: "department".into(),
            sort_order: None,
        })
        .expect("sales should be created");
    let manager = app
        .create_company_agent(CreateCompanyAgentInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            display_name: "Engineering Manager".into(),
            handle: "staffing-scope-manager".into(),
            persona: "负责工程组织的人事管理".into(),
            org_unit_id: Some(engineering.id),
            job_title: Some("项目经理".into()),
            role_key: Some(COMPANY_AGENT_ROLE_MANAGER.into()),
            reports_to_membership_id: None,
        })
        .expect("manager should be created");
    let target = app
        .create_company_agent(CreateCompanyAgentInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            display_name: "Platform Engineer".into(),
            handle: "staffing-scope-target".into(),
            persona: "负责平台研发".into(),
            org_unit_id: Some(platform_team.id),
            job_title: Some("工程师".into()),
            role_key: Some(COMPANY_AGENT_ROLE_MEMBER.into()),
            reports_to_membership_id: Some(manager.membership.id),
        })
        .expect("target should be created");
    let handoff = app
        .create_company_agent(CreateCompanyAgentInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            display_name: "Platform Handoff".into(),
            handle: "staffing-scope-handoff".into(),
            persona: "接管平台工作".into(),
            org_unit_id: Some(platform_team.id),
            job_title: Some("高级工程师".into()),
            role_key: Some(COMPANY_AGENT_ROLE_MEMBER.into()),
            reports_to_membership_id: Some(manager.membership.id),
        })
        .expect("handoff agent should be created");
    let sales_agent = app
        .create_company_agent(CreateCompanyAgentInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            display_name: "Sales Agent".into(),
            handle: "staffing-scope-sales".into(),
            persona: "负责销售".into(),
            org_unit_id: Some(sales.id),
            job_title: Some("销售".into()),
            role_key: Some(COMPANY_AGENT_ROLE_MEMBER.into()),
            reports_to_membership_id: Some(manager.membership.id),
        })
        .expect("sales agent should be created");
    app.update_company_agent_staffing_permissions(UpdateCompanyAgentPermissionsInput {
        human_user_id: owner.id,
        company_id: company.company.id,
        agent_id: manager.agent_profile.id,
        staffing_permissions: vec![
            COMPANY_PERMISSION_STAFF_HIRE.into(),
            COMPANY_PERMISSION_STAFF_SUSPEND.into(),
            COMPANY_PERMISSION_STAFF_TERMINATE.into(),
        ],
        project_permissions: Vec::new(),
        staffing_scope_org_unit_id: Some(engineering.id),
        reason: Some("仅授权工程组织子树".into()),
    })
    .expect("scoped permissions should be granted");

    let provisioned = app
        .hire_company_agent(AgentStaffingHireInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            display_name: "Scoped New Hire".into(),
            handle: "staffing-scope-new-hire".into(),
            persona: "加入平台团队".into(),
            org_unit_id: Some(platform_team.id),
            job_title: Some("工程师".into()),
            reports_to_membership_id: Some(manager.membership.id),
            reason: Some("工程扩编".into()),
            idempotency_key: Some("staffing-scope-hire".into()),
        })
        .expect("scope should include descendant teams");
    assert_eq!(provisioned.membership.org_unit_id, platform_team.id);
    assert!(matches!(
        app.hire_company_agent(AgentStaffingHireInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            display_name: "Out Of Scope Hire".into(),
            handle: "staffing-scope-rejected-hire".into(),
            persona: "不应加入销售".into(),
            org_unit_id: Some(sales.id),
            job_title: None,
            reports_to_membership_id: Some(manager.membership.id),
            reason: None,
            idempotency_key: Some("staffing-scope-rejected-hire".into()),
        }),
        Err(AppError::Unauthorized(_))
    ));
    assert!(matches!(
        app.suspend_company_agent_as_agent(AgentStaffingStatusInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            target_agent_id: sales_agent.agent_profile.id,
            reason: None,
            handoff_plan: None,
            handoff_agent_id: None,
            idempotency_key: Some("staffing-scope-sales-suspend".into()),
        }),
        Err(AppError::Unauthorized(_))
    ));

    let project = app
        .create_company_project(CreateCompanyProjectInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            name: "Scoped Handoff Project".into(),
            description: None,
            member_agent_ids: vec![target.agent_profile.id, handoff.agent_profile.id],
        })
        .expect("project should be created");
    let task = app
        .create_company_project_task(CreateCompanyProjectTaskInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            project_id: project.project.id,
            title: "Complete handoff work".into(),
            description: None,
            priority: None,
            assignee_agent_id: Some(target.agent_profile.id),
            due_at: None,
        })
        .expect("task should be assigned");
    assert!(matches!(
        app.terminate_company_agent_as_agent(AgentStaffingStatusInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            target_agent_id: target.agent_profile.id,
            reason: Some("需要交接".into()),
            handoff_plan: Some("完整交接任务".into()),
            handoff_agent_id: None,
            idempotency_key: Some("staffing-handoff-missing".into()),
        }),
        Err(AppError::Validation(_))
    ));
    assert_eq!(
        app.repo
            .get_company_agent_membership(target.agent_profile.id)
            .expect("target membership should remain")
            .employment_status,
        "active"
    );
    assert!(matches!(
        app.terminate_company_agent_as_agent(AgentStaffingStatusInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            target_agent_id: target.agent_profile.id,
            reason: Some("接管人不在项目".into()),
            handoff_plan: Some("完整交接任务".into()),
            handoff_agent_id: Some(sales_agent.agent_profile.id),
            idempotency_key: Some("staffing-handoff-nonmember".into()),
        }),
        Err(AppError::Validation(_))
    ));
    assert_eq!(
        app.repo
            .get_company_project_task(task.id)
            .expect("task should remain")
            .assignee_agent_id,
        Some(target.agent_profile.id)
    );

    let terminated = app
        .terminate_company_agent_as_agent(AgentStaffingStatusInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            target_agent_id: target.agent_profile.id,
            reason: Some("完成原子交接".into()),
            handoff_plan: Some("任务和上下文交给 Platform Handoff".into()),
            handoff_agent_id: Some(handoff.agent_profile.id),
            idempotency_key: Some("staffing-handoff-valid".into()),
        })
        .expect("valid handoff should terminate and reassign atomically");
    assert_eq!(terminated.reassigned_task_count, 1);
    assert_eq!(
        app.repo
            .get_company_project_task(task.id)
            .expect("task should remain")
            .assignee_agent_id,
        Some(handoff.agent_profile.id)
    );
    assert_eq!(
        terminated.action.result_payload["reassigned_task_count"],
        json!(1)
    );
    assert_eq!(
        terminated.action.result_payload["reassigned_task_ids"],
        json!([task.id])
    );
}
