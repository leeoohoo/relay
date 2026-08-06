use super::*;

#[test]
fn delegated_staffing_daily_limits_do_not_block_human_managers() {
    let app = PlatformApp::new(MemoryPlatformRepository::default());
    let owner = app
        .dev_login(DevLoginInput {
            email: "staffing-quota-owner@example.com".into(),
            display_name: "Staffing Quota Owner".into(),
        })
        .expect("owner should be created");
    let company = app
        .create_company(CreateCompanyInput {
            human_user_id: owner.id,
            name: "Staffing Quota Company".into(),
            slug: Some("staffing-quota-company".into()),
            description: None,
        })
        .expect("company should be created");
    let manager = app
        .create_company_agent(CreateCompanyAgentInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            display_name: "Quota Manager".into(),
            handle: "staffing-quota-manager".into(),
            persona: "执行受限人事动作".into(),
            org_unit_id: None,
            job_title: None,
            role_key: Some(COMPANY_AGENT_ROLE_MANAGER.into()),
            reports_to_membership_id: None,
        })
        .expect("manager should be created");
    let create_worker = |display_name: &str, handle: &str| {
        app.create_company_agent(CreateCompanyAgentInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            display_name: display_name.into(),
            handle: handle.into(),
            persona: "验证每日配额".into(),
            org_unit_id: None,
            job_title: None,
            role_key: Some(COMPANY_AGENT_ROLE_MEMBER.into()),
            reports_to_membership_id: Some(manager.membership.id),
        })
        .expect("worker should be created")
    };
    let worker_one = create_worker("Quota Worker One", "staffing-quota-worker-one");
    let worker_two = create_worker("Quota Worker Two", "staffing-quota-worker-two");
    let worker_three = create_worker("Quota Worker Three", "staffing-quota-worker-three");
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
        staffing_scope_org_unit_id: None,
        reason: Some("验证每日委派人事额度".into()),
    })
    .expect("permissions should be granted");
    app.publish_company_governance_policy(PublishCompanyGovernancePolicyInput {
        human_user_id: owner.id,
        company_id: company.company.id,
        settings: CompanyGovernancePolicySettings {
            agent_staff_limit: 20,
            delegated_agent_hiring_enabled: true,
            delegated_agent_suspension_enabled: true,
            delegated_agent_termination_enabled: true,
            max_active_projects: 100,
            max_project_members: 50,
            daily_delegated_hire_limit: 1,
            daily_delegated_suspension_limit: 1,
            daily_delegated_termination_limit: 1,
            managed_workspace_root: None,
            skill_language: ai_chat_domain::company::COMPANY_SKILL_LANGUAGE_ZH_CN.into(),
        },
        notes: Some("每类 Agent 人事动作每日只允许一次".into()),
    })
    .expect("governance should publish");

    app.hire_company_agent(AgentStaffingHireInput {
        actor_agent_id: manager.agent_profile.id,
        company_id: company.company.id,
        display_name: "Quota Hire One".into(),
        handle: "staffing-quota-hire-one".into(),
        persona: "首个扩招".into(),
        org_unit_id: None,
        job_title: None,
        reports_to_membership_id: Some(manager.membership.id),
        reason: None,
        idempotency_key: Some("staffing-quota-hire-one".into()),
    })
    .expect("first delegated hire should succeed");
    assert!(matches!(
        app.hire_company_agent(AgentStaffingHireInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            display_name: "Quota Hire Two".into(),
            handle: "staffing-quota-hire-two".into(),
            persona: "第二个扩招".into(),
            org_unit_id: None,
            job_title: None,
            reports_to_membership_id: Some(manager.membership.id),
            reason: None,
            idempotency_key: Some("staffing-quota-hire-two".into()),
        }),
        Err(AppError::RateLimited(_))
    ));

    app.suspend_company_agent_as_agent(AgentStaffingStatusInput {
        actor_agent_id: manager.agent_profile.id,
        company_id: company.company.id,
        target_agent_id: worker_one.agent_profile.id,
        reason: None,
        handoff_plan: None,
        handoff_agent_id: None,
        idempotency_key: Some("staffing-quota-suspend-one".into()),
    })
    .expect("first delegated suspension should succeed");
    assert!(matches!(
        app.suspend_company_agent_as_agent(AgentStaffingStatusInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            target_agent_id: worker_two.agent_profile.id,
            reason: None,
            handoff_plan: None,
            handoff_agent_id: None,
            idempotency_key: Some("staffing-quota-suspend-two".into()),
        }),
        Err(AppError::RateLimited(_))
    ));
    app.suspend_company_agent_as_human(HumanCompanyStaffingStatusInput {
        human_user_id: owner.id,
        company_id: company.company.id,
        target_agent_id: worker_two.agent_profile.id,
        reason: Some("Human 不受停职额度限制".into()),
        handoff_plan: None,
        handoff_agent_id: None,
    })
    .expect("human suspension should bypass the delegated limit");

    app.terminate_company_agent_as_agent(AgentStaffingStatusInput {
        actor_agent_id: manager.agent_profile.id,
        company_id: company.company.id,
        target_agent_id: worker_one.agent_profile.id,
        reason: None,
        handoff_plan: Some("无未完成任务".into()),
        handoff_agent_id: None,
        idempotency_key: Some("staffing-quota-terminate-one".into()),
    })
    .expect("first delegated termination should succeed");
    assert!(matches!(
        app.terminate_company_agent_as_agent(AgentStaffingStatusInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            target_agent_id: worker_three.agent_profile.id,
            reason: None,
            handoff_plan: Some("无未完成任务".into()),
            handoff_agent_id: None,
            idempotency_key: Some("staffing-quota-terminate-two".into()),
        }),
        Err(AppError::RateLimited(_))
    ));
    app.terminate_company_agent_as_human(HumanCompanyStaffingStatusInput {
        human_user_id: owner.id,
        company_id: company.company.id,
        target_agent_id: worker_three.agent_profile.id,
        reason: Some("Human 不受裁撤额度限制".into()),
        handoff_plan: Some("无未完成任务".into()),
        handoff_agent_id: None,
    })
    .expect("human termination should bypass the delegated limit");
}

#[test]
fn company_governance_versions_enforce_limits_and_delegated_staffing_controls() {
    let app = PlatformApp::new(MemoryPlatformRepository::default());
    let owner = app
        .dev_login(DevLoginInput {
            email: "governance-owner@example.com".into(),
            display_name: "Governance Owner".into(),
        })
        .expect("owner should be created");
    let company = app
        .create_company(CreateCompanyInput {
            human_user_id: owner.id,
            name: "Governance Company".into(),
            slug: Some("governance-company".into()),
            description: None,
        })
        .expect("company should be created");
    let defaults = app
        .get_company_governance_policy_for_human(owner.id, company.company.id)
        .expect("default governance should be readable");
    assert!(!defaults.configured);
    assert!(defaults.active_version.is_none());
    assert!(defaults.versions.is_empty());
    assert_eq!(defaults.effective_settings.agent_staff_limit, 100);
    assert!(defaults.effective_settings.delegated_agent_hiring_enabled);
    assert_eq!(defaults.effective_settings.max_active_projects, 1_000);
    assert_eq!(defaults.effective_settings.max_project_members, 50);

    let manager = app
        .create_company_agent(CreateCompanyAgentInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            display_name: "Governance Manager".into(),
            handle: "governance-manager".into(),
            persona: "负责执行公司治理策略".into(),
            org_unit_id: Some(company.org_units[0].id),
            job_title: Some("公司经理".into()),
            role_key: Some(COMPANY_AGENT_ROLE_MANAGER.into()),
            reports_to_membership_id: None,
        })
        .expect("manager should be created");
    let worker = app
        .create_company_agent(CreateCompanyAgentInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            display_name: "Governance Worker".into(),
            handle: "governance-worker".into(),
            persona: "执行治理测试任务".into(),
            org_unit_id: Some(company.org_units[0].id),
            job_title: Some("工程师".into()),
            role_key: Some(COMPANY_AGENT_ROLE_MEMBER.into()),
            reports_to_membership_id: Some(manager.membership.id),
        })
        .expect("worker should be created");
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
        staffing_scope_org_unit_id: None,
        reason: Some("验证公司级治理开关优先于 Agent 授权".into()),
    })
    .expect("manager should receive all staffing permissions");

    let v1 = app
        .publish_company_governance_policy(PublishCompanyGovernancePolicyInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            settings: CompanyGovernancePolicySettings {
                agent_staff_limit: 2,
                delegated_agent_hiring_enabled: false,
                delegated_agent_suspension_enabled: false,
                delegated_agent_termination_enabled: false,
                max_active_projects: 1,
                max_project_members: 1,
                daily_delegated_hire_limit: 20,
                daily_delegated_suspension_limit: 50,
                daily_delegated_termination_limit: 20,
                managed_workspace_root: None,
                skill_language: ai_chat_domain::company::COMPANY_SKILL_LANGUAGE_ZH_CN.into(),
            },
            notes: Some("首版关闭 Agent 自主人事权限并收紧项目规模".into()),
        })
        .expect("v1 governance should publish");
    assert!(v1.configured);
    assert_eq!(
        v1.active_version.as_ref().map(|version| version.version),
        Some(1)
    );
    assert_eq!(v1.versions.len(), 1);

    assert!(matches!(
        app.create_company_agent(CreateCompanyAgentInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            display_name: "Over Limit Worker".into(),
            handle: "over-limit-worker".into(),
            persona: "不应被创建".into(),
            org_unit_id: Some(company.org_units[0].id),
            job_title: None,
            role_key: Some(COMPANY_AGENT_ROLE_MEMBER.into()),
            reports_to_membership_id: Some(manager.membership.id),
        }),
        Err(AppError::RateLimited(_))
    ));
    assert!(matches!(
        app.hire_company_agent(AgentStaffingHireInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            display_name: "Delegated Hire Disabled".into(),
            handle: "delegated-hire-disabled".into(),
            persona: "不应被 Agent 扩招".into(),
            org_unit_id: Some(company.org_units[0].id),
            job_title: None,
            reports_to_membership_id: Some(manager.membership.id),
            reason: None,
            idempotency_key: Some("governance-v1-hire".into()),
        }),
        Err(AppError::Unauthorized(_))
    ));
    assert!(matches!(
        app.suspend_company_agent_as_agent(AgentStaffingStatusInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            target_agent_id: worker.agent_profile.id,
            reason: None,
            handoff_plan: None,
            handoff_agent_id: None,
            idempotency_key: Some("governance-v1-suspend".into()),
        }),
        Err(AppError::Unauthorized(_))
    ));
    assert!(matches!(
        app.terminate_company_agent_as_agent(AgentStaffingStatusInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            target_agent_id: worker.agent_profile.id,
            reason: None,
            handoff_plan: Some("交还经理".into()),
            handoff_agent_id: None,
            idempotency_key: Some("governance-v1-terminate".into()),
        }),
        Err(AppError::Unauthorized(_))
    ));

    app.suspend_company_agent_as_human(HumanCompanyStaffingStatusInput {
        human_user_id: owner.id,
        company_id: company.company.id,
        target_agent_id: worker.agent_profile.id,
        reason: Some("Human 停职不受 delegated 开关限制".into()),
        handoff_plan: None,
        handoff_agent_id: None,
    })
    .expect("human should still suspend an agent");
    app.reactivate_company_agent(HumanCompanyStaffingStatusInput {
        human_user_id: owner.id,
        company_id: company.company.id,
        target_agent_id: worker.agent_profile.id,
        reason: Some("继续验证 Human 裁撤".into()),
        handoff_plan: None,
        handoff_agent_id: None,
    })
    .expect("human should reactivate the worker");
    app.terminate_company_agent_as_human(HumanCompanyStaffingStatusInput {
        human_user_id: owner.id,
        company_id: company.company.id,
        target_agent_id: worker.agent_profile.id,
        reason: Some("Human 裁撤不受 delegated 开关限制".into()),
        handoff_plan: Some("任务交还经理".into()),
        handoff_agent_id: None,
    })
    .expect("human should still terminate an agent");

    let replacement = app
        .create_company_agent(CreateCompanyAgentInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            display_name: "Governance Replacement".into(),
            handle: "governance-replacement".into(),
            persona: "参与项目治理验证".into(),
            org_unit_id: Some(company.org_units[0].id),
            job_title: Some("工程师".into()),
            role_key: Some(COMPANY_AGENT_ROLE_MEMBER.into()),
            reports_to_membership_id: Some(manager.membership.id),
        })
        .expect("terminated staff should free one staff slot");
    assert!(matches!(
        app.create_company_project(CreateCompanyProjectInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            name: "Oversized Governance Project".into(),
            description: None,
            member_agent_ids: vec![replacement.agent_profile.id],
        }),
        Err(AppError::Validation(_))
    ));
    let first_project = app
        .create_company_project(CreateCompanyProjectInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            name: "First Governance Project".into(),
            description: None,
            member_agent_ids: vec![],
        })
        .expect("one-member project should fit v1");
    assert!(matches!(
        app.create_company_project(CreateCompanyProjectInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            name: "Second Governance Project".into(),
            description: None,
            member_agent_ids: vec![],
        }),
        Err(AppError::RateLimited(_))
    ));

    let v2 = app
        .publish_company_governance_policy(PublishCompanyGovernancePolicyInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            settings: CompanyGovernancePolicySettings {
                agent_staff_limit: 3,
                delegated_agent_hiring_enabled: true,
                delegated_agent_suspension_enabled: true,
                delegated_agent_termination_enabled: true,
                max_active_projects: 2,
                max_project_members: 2,
                daily_delegated_hire_limit: 20,
                daily_delegated_suspension_limit: 50,
                daily_delegated_termination_limit: 20,
                managed_workspace_root: None,
                skill_language: ai_chat_domain::company::COMPANY_SKILL_LANGUAGE_ZH_CN.into(),
            },
            notes: Some("第二版开放受授权 Agent 的人事动作并扩大项目容量".into()),
        })
        .expect("v2 governance should publish");
    assert_eq!(
        v2.active_version.as_ref().map(|version| version.version),
        Some(2)
    );
    assert_eq!(v2.versions.len(), 2);
    assert_eq!(
        v2.versions[0].status,
        COMPANY_GOVERNANCE_POLICY_STATUS_ACTIVE
    );
    assert_eq!(v2.versions[1].version, 1);
    assert_eq!(
        v2.versions[1].status,
        ai_chat_domain::company::COMPANY_GOVERNANCE_POLICY_STATUS_ARCHIVED
    );

    let expanded_project = app
        .add_company_project_member(AddCompanyProjectMemberInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            project_id: first_project.project.id,
            target_agent_id: replacement.agent_profile.id,
        })
        .expect("v2 member limit should apply immediately");
    assert_eq!(expanded_project.members.len(), 2);
    app.create_company_project(CreateCompanyProjectInput {
        actor_agent_id: manager.agent_profile.id,
        company_id: company.company.id,
        name: "Second Governance Project".into(),
        description: None,
        member_agent_ids: vec![],
    })
    .expect("v2 active project limit should apply immediately");
    app.hire_company_agent(AgentStaffingHireInput {
        actor_agent_id: manager.agent_profile.id,
        company_id: company.company.id,
        display_name: "Delegated Governance Hire".into(),
        handle: "delegated-governance-hire".into(),
        persona: "由授权 Agent 扩招".into(),
        org_unit_id: Some(company.org_units[0].id),
        job_title: None,
        reports_to_membership_id: Some(manager.membership.id),
        reason: Some("v2 已开放扩招".into()),
        idempotency_key: Some("governance-v2-hire".into()),
    })
    .expect("delegated hire should work under v2");
    app.suspend_company_agent_as_agent(AgentStaffingStatusInput {
        actor_agent_id: manager.agent_profile.id,
        company_id: company.company.id,
        target_agent_id: replacement.agent_profile.id,
        reason: Some("v2 已开放停职".into()),
        handoff_plan: None,
        handoff_agent_id: None,
        idempotency_key: Some("governance-v2-suspend".into()),
    })
    .expect("delegated suspension should work under v2");
    app.reactivate_company_agent(HumanCompanyStaffingStatusInput {
        human_user_id: owner.id,
        company_id: company.company.id,
        target_agent_id: replacement.agent_profile.id,
        reason: Some("继续验证 v2 裁撤".into()),
        handoff_plan: None,
        handoff_agent_id: None,
    })
    .expect("human should reactivate replacement");
    app.terminate_company_agent_as_agent(AgentStaffingStatusInput {
        actor_agent_id: manager.agent_profile.id,
        company_id: company.company.id,
        target_agent_id: replacement.agent_profile.id,
        reason: Some("v2 已开放裁撤".into()),
        handoff_plan: Some("项目交还经理".into()),
        handoff_agent_id: None,
        idempotency_key: Some("governance-v2-terminate".into()),
    })
    .expect("delegated termination should work under v2");

    let console = app
        .get_company_console(owner.id, company.company.id)
        .expect("console should include governance history");
    assert_eq!(console.governance_policy.versions.len(), 2);
    assert_eq!(
        console
            .governance_policy
            .active_version
            .as_ref()
            .map(|version| version.version),
        Some(2)
    );
}
