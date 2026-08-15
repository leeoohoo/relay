use super::*;

#[test]
fn managed_project_creation_validates_before_commit_and_stores_git_atomically() {
    let app = PlatformApp::new(MemoryPlatformRepository::default());
    let human = app
        .dev_login(DevLoginInput {
            email: "managed-project@example.com".into(),
            display_name: "Managed Project Human".into(),
        })
        .expect("human should be created");
    let company = app
        .create_company(CreateCompanyInput {
            human_user_id: human.id,
            name: "Managed Project Company".into(),
            slug: Some("managed-project-company".into()),
            description: None,
        })
        .expect("company should be created");
    let owner = app
        .create_company_agent(CreateCompanyAgentInput {
            human_user_id: human.id,
            company_id: company.company.id,
            display_name: "Managed Owner".into(),
            handle: "managed-owner".into(),
            persona: "负责托管项目".into(),
            org_unit_id: None,
            job_title: Some("项目经理".into()),
            role_key: Some(COMPANY_AGENT_ROLE_MANAGER.into()),
            reports_to_membership_id: None,
        })
        .expect("owner should be created");
    let project_id = Uuid::new_v4();
    let project_input = CreateCompanyProjectForHumanInput {
        human_user_id: human.id,
        company_id: company.company.id,
        owner_agent_id: owner.agent_profile.id,
        name: "Managed Relay".into(),
        description: Some("托管 Harness 项目".into()),
        member_agent_ids: Vec::new(),
        project_type: Some("web_application".into()),
        project_type_source: Some(PROJECT_TYPE_SOURCE_HUMAN.into()),
        project_type_confidence: Some(100),
        project_type_evidence: Vec::new(),
        project_id: Some(project_id),
    };

    app.validate_company_project_creation_for_human(project_input.clone())
        .expect("preflight should validate without writing");
    assert!(app.repo.get_company_project(project_id).is_none());
    assert!(app
        .repo
        .get_company_project_git_config(project_id)
        .is_none());
    let cleanup_job_id = Uuid::new_v4();
    let cleanup_now = now_utc();
    app.save_project_provisioning_cleanup_job(ProjectProvisioningCleanupJob {
        id: cleanup_job_id,
        human_user_id: human.id,
        company_id: company.company.id,
        project_id,
        managed_local_path: format!("/tmp/relay-managed-projects/{project_id}"),
        repository_identifier: format!("managed-{project_id}"),
        access_token_identifier: format!("relay-project-{project_id}"),
        status: "pending".into(),
        attempts: 0,
        next_attempt_at: cleanup_now,
        lease_expires_at: None,
        last_error: None,
        created_at: cleanup_now,
        updated_at: cleanup_now,
        completed_at: None,
    })
    .expect("cleanup job should be durable before external provisioning");

    let (project, git) = app
        .create_managed_company_project_for_human(CreateManagedCompanyProjectForHumanInput {
            project: project_input,
            cleanup_job_id,
            remote_url: "https://git.example.test/relay/managed.git".into(),
            host_local_path: format!("/tmp/relay-managed-projects/{project_id}"),
            default_branch: "main".into(),
            auth_profile: format!("managed-git-token-{project_id}"),
            allow_agent_push: true,
            branch_prefix: "relay/".into(),
        })
        .expect("project and Git config should commit together");
    assert_eq!(project.project.id, project_id);
    assert_eq!(git.remote_url, "https://git.example.test/relay/managed.git");
    assert!(app.repo.get_company_project(project_id).is_some());
    assert!(app
        .repo
        .get_company_project_git_config(project_id)
        .is_some());
    assert!(app
        .claim_due_project_provisioning_cleanup_job(
            cleanup_now + chrono::Duration::minutes(1),
            cleanup_now + chrono::Duration::minutes(6),
        )
        .expect("completed project should not leave a cleanup job")
        .is_none());
}

#[test]
fn project_provisioning_cleanup_jobs_retry_with_a_lease_and_finish_idempotently() {
    let app = PlatformApp::new(MemoryPlatformRepository::default());
    let human = app
        .dev_login(DevLoginInput {
            email: "cleanup-retry@example.com".into(),
            display_name: "Cleanup Retry Human".into(),
        })
        .expect("human should be created");
    let company = app
        .create_company(CreateCompanyInput {
            human_user_id: human.id,
            name: "Cleanup Retry Company".into(),
            slug: Some("cleanup-retry-company".into()),
            description: None,
        })
        .expect("company should be created");
    let now = now_utc();
    let provisioning_lease = now + chrono::Duration::minutes(1);
    let job_id = Uuid::new_v4();
    app.save_project_provisioning_cleanup_job(ProjectProvisioningCleanupJob {
        id: job_id,
        human_user_id: human.id,
        company_id: company.company.id,
        project_id: Uuid::new_v4(),
        managed_local_path: "/tmp/relay-cleanup-retry".into(),
        repository_identifier: "cleanup-retry".into(),
        access_token_identifier: "relay-project-cleanup-retry".into(),
        status: "running".into(),
        attempts: 0,
        next_attempt_at: now,
        lease_expires_at: Some(provisioning_lease),
        last_error: None,
        created_at: now,
        updated_at: now,
        completed_at: None,
    })
    .expect("cleanup job should be saved");

    assert!(app
        .claim_due_project_provisioning_cleanup_job(now, now + chrono::Duration::minutes(5))
        .expect("active provisioning lease should be checked")
        .is_none());
    let claimed = app
        .claim_due_project_provisioning_cleanup_job(
            provisioning_lease,
            provisioning_lease + chrono::Duration::minutes(5),
        )
        .expect("cleanup job should be claimable")
        .expect("expired provisioning lease should be recoverable");
    assert_eq!(claimed.id, job_id);
    assert_eq!(claimed.attempts, 1);

    let retry_at = provisioning_lease + chrono::Duration::minutes(2);
    app.retry_project_provisioning_cleanup_job(job_id, "Harness unavailable".into(), retry_at, now)
        .expect("cleanup job should be rescheduled");
    assert!(app
        .claim_due_project_provisioning_cleanup_job(
            now + chrono::Duration::minutes(1),
            now + chrono::Duration::minutes(6),
        )
        .expect("early retry lookup should succeed")
        .is_none());
    assert!(app
        .claim_due_project_provisioning_cleanup_job(
            retry_at,
            retry_at + chrono::Duration::minutes(5),
        )
        .expect("retry should become due")
        .is_some());
    app.complete_project_provisioning_cleanup_job(job_id, retry_at)
        .expect("cleanup job should complete");
    assert!(app
        .claim_due_project_provisioning_cleanup_job(
            retry_at + chrono::Duration::hours(1),
            retry_at + chrono::Duration::hours(2),
        )
        .expect("completed cleanup should stay disarmed")
        .is_none());
}

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
            after_message_id: None,
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

    let pending_intent = AgentExecutionIntent {
        id: Uuid::new_v4(),
        company_id: company.company.id,
        agent_profile_id: engineer.agent_profile.id,
        project_id: project.project.id,
        worker_session_id: None,
        source_event_ids: Vec::new(),
        task_ids: vec![task.id],
        action_type: AGENT_EXECUTION_INTENT_ACTION_EXECUTE.into(),
        objective: "暂停后保留并等待恢复".into(),
        acceptance_criteria: vec!["恢复项目后继续执行".into()],
        required_capabilities: vec![],
        priority: "high".into(),
        dedupe_key: "pause-preserves-pending-intent".into(),
        status: AGENT_EXECUTION_INTENT_STATUS_PENDING.into(),
        result_summary: String::new(),
        error_message: None,
        created_at: now_utc(),
        claimed_at: None,
        completed_at: None,
    };
    app.create_agent_execution_intent(pending_intent.clone())
        .expect("active project should accept pending work");

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
    assert_eq!(paused_decision.pending_execution_intent_count, 0);
    let mut paused_dispatch = pending_intent.clone();
    paused_dispatch.id = Uuid::new_v4();
    paused_dispatch.dedupe_key = "pause-rejects-new-intent".into();
    assert!(matches!(
        app.create_agent_execution_intent(paused_dispatch),
        Err(AppError::Conflict(message)) if message.contains("project is paused")
    ));
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
    let resumed_decision = app
        .decide_agent_codex_work(&engineer_trigger)
        .expect("resumed project should expose its pending work again");
    assert!(resumed_decision.should_run);
    assert_eq!(resumed_decision.pending_execution_intent_count, 1);
    app.send_human_company_message(SendHumanCompanyMessageInput {
        human_user_id: owner.id,
        company_id: company.company.id,
        conversation_id: project.project.project_group_conversation_id,
        content: "项目已恢复".into(),
    })
    .expect("project group should resume after the project is active");
}
