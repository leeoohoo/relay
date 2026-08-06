use super::*;

#[test]
fn human_managers_create_assign_and_update_project_tasks() {
    let app = PlatformApp::new(MemoryPlatformRepository::default());
    let owner = app
        .dev_login(DevLoginInput {
            email: "task-owner@example.com".into(),
            display_name: "Task Owner".into(),
        })
        .expect("owner should be created");
    let outsider = app
        .dev_login(DevLoginInput {
            email: "task-outsider@example.com".into(),
            display_name: "Task Outsider".into(),
        })
        .expect("outsider should be created");
    let company = app
        .create_company(CreateCompanyInput {
            human_user_id: owner.id,
            name: "Human Task Company".into(),
            slug: Some("human-task-company".into()),
            description: None,
        })
        .expect("company should be created");
    let manager = app
        .create_company_agent(CreateCompanyAgentInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            display_name: "Task Manager".into(),
            handle: "task-manager".into(),
            persona: "负责拆分任务".into(),
            org_unit_id: None,
            job_title: None,
            role_key: Some(COMPANY_AGENT_ROLE_MANAGER.into()),
            reports_to_membership_id: None,
        })
        .expect("manager should be created");
    let engineer = app
        .create_company_agent(CreateCompanyAgentInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            display_name: "Task Engineer".into(),
            handle: "task-engineer".into(),
            persona: "负责交付任务".into(),
            org_unit_id: None,
            job_title: None,
            role_key: Some(COMPANY_AGENT_ROLE_MEMBER.into()),
            reports_to_membership_id: Some(manager.membership.id),
        })
        .expect("engineer should be created");
    let project = app
        .create_company_project(CreateCompanyProjectInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            name: "Human Task Center".into(),
            description: Some("Human 分配，Agent 执行".into()),
            member_agent_ids: vec![engineer.agent_profile.id],
        })
        .expect("project should be created");

    let foundation = app
        .create_company_project_task_for_human(CreateCompanyProjectTaskForHumanInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            project_id: project.project.id,
            title: "准备验收环境".into(),
            description: None,
            priority: Some(PROJECT_TASK_PRIORITY_HIGH.into()),
            assignee_agent_id: Some(manager.agent_profile.id),
            due_at: None,
            depends_on_task_ids: Vec::new(),
        })
        .expect("owner should create a prerequisite task");
    let task = app
        .create_company_project_task_for_human(CreateCompanyProjectTaskForHumanInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            project_id: project.project.id,
            title: "完成功能验收".into(),
            description: Some("通过任务中心和 MCP 验收".into()),
            priority: Some(PROJECT_TASK_PRIORITY_URGENT.into()),
            assignee_agent_id: Some(engineer.agent_profile.id),
            due_at: None,
            depends_on_task_ids: vec![foundation.id],
        })
        .expect("owner should create and assign a task");
    assert_eq!(task.created_by_human_user_id, Some(owner.id));
    assert!(task.created_by_agent_id.is_none());
    let assignment = app
        .list_agent_inbox_events(engineer.agent_profile.id, true, 50)
        .expect("engineer inbox should load")
        .into_iter()
        .find(|event| {
            event.event_type == "company.project.task_assigned"
                && payload_uuid_field_optional(&event.payload_json, "task_id") == Some(task.id)
        })
        .expect("human assignment should enqueue a task event");
    assert_eq!(
        payload_uuid_field_optional(&assignment.payload_json, "assigned_by_human_user_id"),
        Some(owner.id)
    );

    let listed = app
        .list_company_project_tasks_for_human(owner.id, company.company.id)
        .expect("active company human should list tasks");
    assert_eq!(listed.len(), 2);
    assert!(listed.iter().any(|listed_task| listed_task.id == task.id));
    let project_view = app
        .get_company_console(owner.id, company.company.id)
        .expect("company console should include task dependencies");
    let dependency = project_view.projects[0]
        .task_dependencies
        .iter()
        .find(|dependency| dependency.task_id == task.id)
        .expect("human-created dependency should be stored");
    assert_eq!(dependency.depends_on_task_id, foundation.id);
    assert_eq!(dependency.created_by_human_user_id, Some(owner.id));
    assert!(dependency.created_by_agent_id.is_none());
    assert!(matches!(
        app.list_company_project_tasks_for_human(outsider.id, company.company.id),
        Err(AppError::Unauthorized(_))
    ));
    assert!(matches!(
        app.create_company_project_task_for_human(CreateCompanyProjectTaskForHumanInput {
            human_user_id: outsider.id,
            company_id: company.company.id,
            project_id: project.project.id,
            title: "越权任务".into(),
            description: None,
            priority: None,
            assignee_agent_id: None,
            due_at: None,
            depends_on_task_ids: Vec::new(),
        }),
        Err(AppError::Unauthorized(_))
    ));

    for event in app
        .list_agent_inbox_events(engineer.agent_profile.id, true, 50)
        .expect("engineer pending inbox should load")
    {
        app.mark_agent_inbox_event_processed(MarkInboxEventProcessedInput {
            actor_agent_id: engineer.agent_profile.id,
            event_id: event.id,
        })
        .expect("test should acknowledge assignment events");
    }
    let now = now_utc();
    let trigger = AgentCodexTriggerConfig {
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
        next_run_at: now,
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
        created_at: now,
        updated_at: now,
    };
    let waiting_decision = app
        .decide_agent_codex_work(&trigger)
        .expect("trigger should classify waiting tasks");
    assert!(!waiting_decision.should_run);
    assert_eq!(waiting_decision.active_task_count, 0);
    assert_eq!(waiting_decision.waiting_task_count, 1);

    assert!(matches!(
        app.update_company_project_task_for_human(UpdateCompanyProjectTaskForHumanInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            project_id: project.project.id,
            task_id: task.id,
            title: None,
            description: None,
            status: Some(PROJECT_TASK_STATUS_IN_PROGRESS.into()),
            priority: None,
            assignee_agent_id: None,
            clear_assignee: false,
            due_at: None,
            clear_due_at: false,
            depends_on_task_ids: Some(vec![foundation.id]),
        }),
        Err(AppError::Conflict(_))
    ));
    app.update_company_project_task_for_human(UpdateCompanyProjectTaskForHumanInput {
        human_user_id: owner.id,
        company_id: company.company.id,
        project_id: project.project.id,
        task_id: foundation.id,
        title: None,
        description: None,
        status: Some(PROJECT_TASK_STATUS_DONE.into()),
        priority: None,
        assignee_agent_id: None,
        clear_assignee: false,
        due_at: None,
        clear_due_at: false,
        depends_on_task_ids: Some(Vec::new()),
    })
    .expect("prerequisite should complete");
    let ready_decision = app
        .decide_agent_codex_work(&trigger)
        .expect("trigger should re-check completed prerequisites");
    assert!(ready_decision.should_run);
    assert_eq!(ready_decision.active_task_count, 1);
    assert_eq!(ready_decision.waiting_task_count, 0);

    let updated = app
        .update_company_project_task_for_human(UpdateCompanyProjectTaskForHumanInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            project_id: project.project.id,
            task_id: task.id,
            title: None,
            description: None,
            status: Some(PROJECT_TASK_STATUS_DONE.into()),
            priority: None,
            assignee_agent_id: Some(manager.agent_profile.id),
            clear_assignee: false,
            due_at: None,
            clear_due_at: false,
            depends_on_task_ids: None,
        })
        .expect("owner should update and reassign a task");
    assert_eq!(updated.status, PROJECT_TASK_STATUS_DONE);
    assert_eq!(updated.assignee_agent_id, Some(manager.agent_profile.id));
    assert_eq!(updated.updated_by_human_user_id, Some(owner.id));
    assert!(updated.updated_by_agent_id.is_none());
    assert!(updated.completed_at.is_some());
    let history = app
        .get_company_console(owner.id, company.company.id)
        .expect("company console should include task status history")
        .projects
        .into_iter()
        .find(|item| item.project.id == project.project.id)
        .expect("project should remain visible")
        .task_status_history
        .into_iter()
        .filter(|entry| entry.task_id == task.id)
        .collect::<Vec<_>>();
    assert!(history
        .iter()
        .any(|entry| entry.to_status == PROJECT_TASK_STATUS_TODO));
    assert!(history
        .iter()
        .any(|entry| entry.to_status == PROJECT_TASK_STATUS_DONE));
    assert!(app
        .list_agent_inbox_events(manager.agent_profile.id, true, 50)
        .expect("manager inbox should load")
        .iter()
        .any(|event| {
            event.event_type == "company.project.task_assigned"
                && payload_uuid_field_optional(&event.payload_json, "task_id") == Some(task.id)
        }));
}
