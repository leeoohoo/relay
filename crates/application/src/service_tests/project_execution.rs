use super::*;

#[test]
fn project_owner_transfer_is_atomic_and_keeps_previous_owner_as_member() {
    let app = PlatformApp::new(MemoryPlatformRepository::default());
    let human = app
        .dev_login(DevLoginInput {
            email: "owner-transfer@example.com".into(),
            display_name: "Owner Transfer Human".into(),
        })
        .expect("human should be created");
    let company = app
        .create_company(CreateCompanyInput {
            human_user_id: human.id,
            name: "Owner Transfer Company".into(),
            slug: Some("owner-transfer-company".into()),
            description: None,
        })
        .expect("company should be created");
    let manager = app
        .create_company_agent(CreateCompanyAgentInput {
            human_user_id: human.id,
            company_id: company.company.id,
            display_name: "Initial Owner".into(),
            handle: "initial-owner".into(),
            persona: "负责项目管理".into(),
            org_unit_id: None,
            job_title: Some("项目经理".into()),
            role_key: Some(COMPANY_AGENT_ROLE_MANAGER.into()),
            reports_to_membership_id: None,
        })
        .expect("manager should be created");
    let engineer = app
        .create_company_agent(CreateCompanyAgentInput {
            human_user_id: human.id,
            company_id: company.company.id,
            display_name: "New Owner".into(),
            handle: "new-owner".into(),
            persona: "负责项目交付".into(),
            org_unit_id: None,
            job_title: Some("软件工程师".into()),
            role_key: Some(COMPANY_AGENT_ROLE_MEMBER.into()),
            reports_to_membership_id: Some(manager.membership.id),
        })
        .expect("engineer should be created");
    let project = app
        .create_company_project(CreateCompanyProjectInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            name: "Owner Transfer Project".into(),
            description: None,
            member_agent_ids: vec![],
        })
        .expect("manager should create project");

    assert!(matches!(
        app.transfer_company_project_owner(TransferCompanyProjectOwnerInput {
            actor_agent_id: engineer.agent_profile.id,
            company_id: company.company.id,
            project_id: project.project.id,
            owner_agent_id: engineer.agent_profile.id,
        }),
        Err(AppError::Unauthorized(_))
    ));

    let transferred = app
        .transfer_company_project_owner_for_human(TransferCompanyProjectOwnerForHumanInput {
            human_user_id: human.id,
            company_id: company.company.id,
            project_id: project.project.id,
            owner_agent_id: engineer.agent_profile.id,
        })
        .expect("human manager should transfer project ownership");
    assert_eq!(
        transferred.project.owner_agent_id,
        engineer.agent_profile.id
    );
    assert_eq!(transferred.members.len(), 2);
    assert_eq!(
        transferred
            .members
            .iter()
            .find(|member| member.member.agent_profile_id == manager.agent_profile.id)
            .expect("previous owner remains a member")
            .member
            .role,
        PROJECT_MEMBER_ROLE_MEMBER
    );
    assert_eq!(
        transferred
            .members
            .iter()
            .find(|member| member.member.agent_profile_id == engineer.agent_profile.id)
            .expect("new owner is a member")
            .member
            .role,
        PROJECT_MEMBER_ROLE_OWNER
    );
    assert!(transferred
        .project_group
        .member_agent_ids
        .contains(&engineer.agent_profile.id));

    let after_removal = app
        .remove_company_project_member(RemoveCompanyProjectMemberInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            project_id: project.project.id,
            target_agent_id: manager.agent_profile.id,
        })
        .expect("previous owner should be removable after transfer");
    assert_eq!(after_removal.members.len(), 1);
    assert_eq!(
        after_removal.members[0].member.agent_profile_id,
        engineer.agent_profile.id
    );
}

#[test]
fn company_agents_can_run_projects_with_synced_group_tasks_and_status() {
    let app = PlatformApp::new(MemoryPlatformRepository::default());
    let owner = app
        .dev_login(DevLoginInput {
            email: "project-owner@example.com".into(),
            display_name: "Project Owner".into(),
        })
        .expect("owner should be created");
    let company = app
        .create_company(CreateCompanyInput {
            human_user_id: owner.id,
            name: "Project Company".into(),
            slug: Some("project-company".into()),
            description: None,
        })
        .expect("company should be created");
    let manager = app
        .create_company_agent(CreateCompanyAgentInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            display_name: "Project Manager".into(),
            handle: "project-manager".into(),
            persona: "负责项目管理".into(),
            org_unit_id: None,
            job_title: Some("项目经理".into()),
            role_key: Some(COMPANY_AGENT_ROLE_MANAGER.into()),
            reports_to_membership_id: None,
        })
        .expect("manager should be created");
    let engineer = app
        .create_company_agent(CreateCompanyAgentInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            display_name: "Project Engineer".into(),
            handle: "project-engineer".into(),
            persona: "负责项目研发".into(),
            org_unit_id: None,
            job_title: Some("软件工程师".into()),
            role_key: Some(COMPANY_AGENT_ROLE_MEMBER.into()),
            reports_to_membership_id: Some(manager.membership.id),
        })
        .expect("engineer should be created");

    let project = app
        .create_company_project(CreateCompanyProjectInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            name: "Agent 协作平台".into(),
            description: Some("交付公司 Agent 项目协作闭环".into()),
            member_agent_ids: vec![engineer.agent_profile.id],
        })
        .expect("manager should create project");
    assert_eq!(project.members.len(), 2);
    assert_eq!(
        project.project_group.context.context_type,
        CONVERSATION_CONTEXT_PROJECT_GROUP
    );
    assert_eq!(
        project.project_group.context.project_id,
        Some(project.project.id)
    );

    let future_check = now_utc() + Duration::hours(4);
    for agent_id in [manager.agent_profile.id, engineer.agent_profile.id] {
        app.repo
            .save_agent_codex_trigger_config(AgentCodexTriggerConfig {
                id: Uuid::new_v4(),
                company_id: company.company.id,
                agent_profile_id: agent_id,
                status: AGENT_CODEX_TRIGGER_STATUS_ACTIVE.into(),
                interval_seconds: 14_400,
                codex_profile: "default".into(),
                model: None,
                reasoning_effort: None,
                reasoning_summary: None,
                verbosity: None,
                personality: None,
                service_tier: None,
                sandbox_mode: AGENT_CODEX_SANDBOX_WORKSPACE_WRITE.into(),
                approval_policy: AGENT_CODEX_APPROVAL_POLICY_NEVER.into(),
                network_access: None,
                web_search: None,
                feature_multi_agent: None,
                feature_remote_plugin: None,
                feature_hooks: None,
                feature_goals: None,
                feature_shell_tool: None,
                max_run_seconds: 1_800,
                next_run_at: future_check,
                lease_owner: None,
                lease_expires_at: None,
                manual_run_requested_at: None,
                wake_requested_at: None,
                wake_reason: None,
                last_run_at: None,
                last_success_at: None,
                last_error: None,
                consecutive_failure_count: 0,
                created_by_human_user_id: owner.id,
                updated_by_human_user_id: Some(owner.id),
                created_at: now_utc(),
                updated_at: now_utc(),
            })
            .expect("test should configure the Agent Codex trigger");
    }
    let _owner_broadcast = app
        .send_company_message(SendCompanyMessageInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            conversation_id: project.project_group.preview.id,
            content: "阶段判断已完成，请项目成员按任务计划继续推进。".into(),
        })
        .expect("project owner should broadcast to the project group");
    let engineer_trigger = app
        .repo
        .get_agent_codex_trigger_config_by_agent(engineer.agent_profile.id)
        .expect("project member trigger should exist");
    assert!(engineer_trigger.wake_requested_at.is_none());
    assert!(engineer_trigger.wake_reason.is_none());
    assert_eq!(engineer_trigger.next_run_at, future_check);
    assert!(app
        .repo
        .get_agent_codex_trigger_config_by_agent(manager.agent_profile.id)
        .expect("project owner trigger should exist")
        .wake_requested_at
        .is_none());

    let task = app
        .create_company_project_task(CreateCompanyProjectTaskInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            project_id: project.project.id,
            title: "实现项目任务 API".into(),
            description: None,
            priority: Some(PROJECT_TASK_PRIORITY_HIGH.into()),
            assignee_agent_id: Some(engineer.agent_profile.id),
            due_at: None,
        })
        .expect("manager should assign task");
    assert!(matches!(
        app.create_company_project_task(CreateCompanyProjectTaskInput {
            actor_agent_id: engineer.agent_profile.id,
            company_id: company.company.id,
            project_id: project.project.id,
            title: "执行者不应自行建任务".into(),
            description: None,
            priority: None,
            assignee_agent_id: Some(engineer.agent_profile.id),
            due_at: None,
        }),
        Err(AppError::Unauthorized(_))
    ));
    assert!(matches!(
        app.update_company_project_task(UpdateCompanyProjectTaskInput {
            actor_agent_id: engineer.agent_profile.id,
            company_id: company.company.id,
            project_id: project.project.id,
            task_id: task.id,
            title: None,
            description: None,
            status: Some(PROJECT_TASK_STATUS_CANCELLED.into()),
            priority: None,
            assignee_agent_id: None,
            due_at: None,
        }),
        Err(AppError::Unauthorized(_))
    ));
    let updated_task = app
        .update_company_project_task(UpdateCompanyProjectTaskInput {
            actor_agent_id: engineer.agent_profile.id,
            company_id: company.company.id,
            project_id: project.project.id,
            task_id: task.id,
            title: None,
            description: None,
            status: Some(PROJECT_TASK_STATUS_IN_PROGRESS.into()),
            priority: None,
            assignee_agent_id: None,
            due_at: None,
        })
        .expect("assignee should update task status");
    assert_eq!(updated_task.status, PROJECT_TASK_STATUS_IN_PROGRESS);

    let status_update = app
        .create_company_project_status_update(CreateCompanyProjectStatusUpdateInput {
            actor_agent_id: engineer.agent_profile.id,
            company_id: company.company.id,
            project_id: project.project.id,
            summary: "任务 API 已进入开发阶段".into(),
            progress_percent: 35,
            blockers: vec![],
            next_steps: vec!["完成 PostgreSQL 持久化".into()],
            project_status: None,
        })
        .expect("project member should report status");
    assert_eq!(status_update.progress_percent, 35);
    app.send_company_message(SendCompanyMessageInput {
        actor_agent_id: engineer.agent_profile.id,
        company_id: company.company.id,
        conversation_id: project.project.project_group_conversation_id,
        content: "当前进度 35%，暂无阻塞".into(),
    })
    .expect("project member should chat in project group");
    let manager_unread = app
        .list_company_group_unread_messages(ListCompanyGroupUnreadInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            conversation_id: Some(project.project.project_group_conversation_id),
            message_limit: 20,
        })
        .expect("project group member should see unread group messages");
    assert_eq!(manager_unread.total_unread_count, 1);
    assert_eq!(manager_unread.groups[0].unread_messages.len(), 1);
    assert_eq!(
        manager_unread.groups[0].unread_messages[0].content,
        "当前进度 35%，暂无阻塞"
    );

    let console = app
        .get_company_console(owner.id, company.company.id)
        .expect("human should observe projects");
    assert_eq!(console.projects.len(), 1);
    assert_eq!(console.projects[0].tasks.len(), 1);
    assert_eq!(console.projects[0].status_updates.len(), 1);
    assert!(console.conversations.iter().any(|conversation| {
        conversation.context.context_type == CONVERSATION_CONTEXT_PROJECT_GROUP
    }));

    let asset_refresh_due_at = now_utc() - Duration::minutes(1);
    app.repo
        .save_company_project_asset_refresh_config(CompanyProjectAssetRefreshConfig {
            project_id: project.project.id,
            maintainer_agent_id: engineer.agent_profile.id,
            interval_minutes: 60,
            enabled: true,
            next_refresh_at: asset_refresh_due_at,
            last_requested_at: None,
            last_completed_at: None,
            created_by_human_user_id: owner.id,
            updated_by_human_user_id: None,
            created_at: asset_refresh_due_at,
            updated_at: asset_refresh_due_at,
        })
        .expect("asset refresh schedule should be stored");

    let paused = app
        .pause_company_project_for_human(SetCompanyProjectPauseForHumanInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            project_id: project.project.id,
        })
        .expect("company owner should pause the project");
    assert_eq!(paused.project.status, PROJECT_STATUS_PAUSED);
    assert!(matches!(
        app.send_human_company_message(SendHumanCompanyMessageInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            conversation_id: project.project.project_group_conversation_id,
            content: "暂停后不应继续群聊".into(),
        }),
        Err(AppError::Conflict(_))
    ));
    assert!(matches!(
        app.update_company_project_task(UpdateCompanyProjectTaskInput {
            actor_agent_id: engineer.agent_profile.id,
            company_id: company.company.id,
            project_id: project.project.id,
            task_id: task.id,
            title: None,
            description: None,
            status: Some(PROJECT_TASK_STATUS_DONE.into()),
            priority: None,
            assignee_agent_id: None,
            due_at: None,
        }),
        Err(AppError::Conflict(_))
    ));
    assert!(matches!(
        app.remove_company_project_member(RemoveCompanyProjectMemberInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            project_id: project.project.id,
            target_agent_id: engineer.agent_profile.id,
        }),
        Err(AppError::Conflict(_))
    ));
    assert!(matches!(
        app.update_company_project_rule_for_human(UpdateCompanyProjectRuleForHumanInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            project_id: project.project.id,
            content: "暂停期间不应修改 Rule".into(),
        }),
        Err(AppError::Conflict(_))
    ));
    assert!(matches!(
        app.batch_update_company_project_tasks(BatchUpdateCompanyProjectTasksInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            project_id: project.project.id,
            task_ids: vec![task.id],
            status: None,
            priority: Some(PROJECT_TASK_PRIORITY_HIGH.into()),
            assignee_agent_id: None,
            clear_assignee: false,
            due_at: None,
            clear_due_at: false,
        }),
        Err(AppError::Conflict(_))
    ));
    let trigger_now = now_utc();
    let engineer_trigger = AgentCodexTriggerConfig {
        id: Uuid::new_v4(),
        company_id: company.company.id,
        agent_profile_id: engineer.agent_profile.id,
        status: AGENT_CODEX_TRIGGER_STATUS_ACTIVE.into(),
        interval_seconds: 60,
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
        max_run_seconds: 600,
        next_run_at: trigger_now,
        lease_owner: None,
        lease_expires_at: None,
        manual_run_requested_at: None,
        wake_requested_at: None,
        wake_reason: None,
        last_run_at: None,
        last_success_at: None,
        last_error: None,
        consecutive_failure_count: 0,
        created_by_human_user_id: owner.id,
        updated_by_human_user_id: None,
        created_at: trigger_now,
        updated_at: trigger_now,
    };
    let paused_decision = app
        .decide_agent_codex_work(&engineer_trigger)
        .expect("paused project should produce a safe trigger decision");
    assert!(!paused_decision.should_run);
    assert!(paused_decision.project.is_none());
    let paused_refresh = app
        .repo
        .get_company_project_asset_refresh_config(project.project.id)
        .expect("paused project refresh schedule should remain available");
    assert_eq!(paused_refresh.next_refresh_at, asset_refresh_due_at);
    assert!(paused_refresh.last_requested_at.is_none());

    let resumed = app
        .resume_company_project_for_human(SetCompanyProjectPauseForHumanInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            project_id: project.project.id,
        })
        .expect("company owner should resume the project");
    assert_eq!(resumed.project.status, PROJECT_STATUS_ACTIVE);
    app.send_human_company_message(SendHumanCompanyMessageInput {
        human_user_id: owner.id,
        company_id: company.company.id,
        conversation_id: project.project.project_group_conversation_id,
        content: "项目已恢复".into(),
    })
    .expect("project group should resume after the project is active");
}
