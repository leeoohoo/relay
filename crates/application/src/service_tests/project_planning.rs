use super::*;

#[test]
fn project_planning_supports_metadata_dependencies_and_atomic_batch_updates() {
    let app = PlatformApp::new(MemoryPlatformRepository::default());
    let owner = app
        .dev_login(DevLoginInput {
            email: "planning-owner@example.com".into(),
            display_name: "Planning Owner".into(),
        })
        .expect("owner should be created");
    let company = app
        .create_company(CreateCompanyInput {
            human_user_id: owner.id,
            name: "Planning Company".into(),
            slug: Some("planning-company".into()),
            description: None,
        })
        .expect("company should be created");
    let manager = app
        .create_company_agent(CreateCompanyAgentInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            display_name: "Planning Manager".into(),
            handle: "planning-manager".into(),
            persona: "负责项目计划和任务依赖".into(),
            org_unit_id: Some(company.org_units[0].id),
            job_title: Some("项目经理".into()),
            role_key: Some(COMPANY_AGENT_ROLE_MANAGER.into()),
            reports_to_membership_id: None,
        })
        .expect("manager should be created");
    let engineer = app
        .create_company_agent(CreateCompanyAgentInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            display_name: "Planning Engineer".into(),
            handle: "planning-engineer".into(),
            persona: "按依赖顺序交付任务".into(),
            org_unit_id: Some(company.org_units[0].id),
            job_title: Some("工程师".into()),
            role_key: Some(COMPANY_AGENT_ROLE_MEMBER.into()),
            reports_to_membership_id: Some(manager.membership.id),
        })
        .expect("engineer should be created");
    let project = app
        .create_company_project(CreateCompanyProjectInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            name: "Planning MVP".into(),
            description: Some("初始项目说明".into()),
            member_agent_ids: vec![engineer.agent_profile.id],
        })
        .expect("project should be created");
    let project_due_at = now_utc() + Duration::days(30);
    let updated_project = app
        .update_company_project(UpdateCompanyProjectInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            project_id: project.project.id,
            name: Some("Planning Controls".into()),
            description: Some("支持元数据、依赖和批量更新".into()),
            due_at: Some(project_due_at),
            clear_due_at: false,
        })
        .expect("project metadata should update");
    assert_eq!(updated_project.project.name, "Planning Controls");
    assert_eq!(updated_project.project.due_at, Some(project_due_at));
    assert_eq!(
        updated_project.project.updated_by_agent_id,
        Some(manager.agent_profile.id)
    );
    assert_eq!(
        updated_project.project_group.preview.title,
        "项目 · Planning Controls"
    );

    let foundation = app
        .create_company_project_task(CreateCompanyProjectTaskInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            project_id: project.project.id,
            title: "完成基础能力".into(),
            description: None,
            priority: Some(PROJECT_TASK_PRIORITY_HIGH.into()),
            assignee_agent_id: Some(engineer.agent_profile.id),
            due_at: None,
        })
        .expect("foundation task should be created");
    let delivery = app
        .create_company_project_task(CreateCompanyProjectTaskInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            project_id: project.project.id,
            title: "交付业务能力".into(),
            description: None,
            priority: Some(PROJECT_TASK_PRIORITY_NORMAL.into()),
            assignee_agent_id: Some(engineer.agent_profile.id),
            due_at: None,
        })
        .expect("delivery task should be created");
    let launch = app
        .create_company_project_task(CreateCompanyProjectTaskInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            project_id: project.project.id,
            title: "发布验收".into(),
            description: None,
            priority: Some(PROJECT_TASK_PRIORITY_NORMAL.into()),
            assignee_agent_id: Some(engineer.agent_profile.id),
            due_at: None,
        })
        .expect("launch task should be created");
    let late_requirement = app
        .create_company_project_task(CreateCompanyProjectTaskInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            project_id: project.project.id,
            title: "补充晚到需求".into(),
            description: None,
            priority: Some(PROJECT_TASK_PRIORITY_NORMAL.into()),
            assignee_agent_id: Some(manager.agent_profile.id),
            due_at: None,
        })
        .expect("late requirement task should be created");

    app.add_company_project_task_dependency(ChangeCompanyProjectTaskDependencyInput {
        actor_agent_id: manager.agent_profile.id,
        company_id: company.company.id,
        project_id: project.project.id,
        task_id: delivery.id,
        depends_on_task_id: foundation.id,
        dependency_condition: None,
    })
    .expect("delivery should depend on foundation");
    app.add_company_project_task_dependency(ChangeCompanyProjectTaskDependencyInput {
        actor_agent_id: manager.agent_profile.id,
        company_id: company.company.id,
        project_id: project.project.id,
        task_id: launch.id,
        depends_on_task_id: delivery.id,
        dependency_condition: None,
    })
    .expect("launch should depend on delivery");
    assert!(matches!(
        app.add_company_project_task_dependency(ChangeCompanyProjectTaskDependencyInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            project_id: project.project.id,
            task_id: delivery.id,
            depends_on_task_id: foundation.id,
            dependency_condition: None,
        }),
        Err(AppError::Conflict(_))
    ));
    assert!(matches!(
        app.add_company_project_task_dependency(ChangeCompanyProjectTaskDependencyInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            project_id: project.project.id,
            task_id: foundation.id,
            depends_on_task_id: launch.id,
            dependency_condition: None,
        }),
        Err(AppError::Validation(_))
    ));
    assert!(matches!(
        app.update_company_project_task(UpdateCompanyProjectTaskInput {
            actor_agent_id: engineer.agent_profile.id,
            company_id: company.company.id,
            project_id: project.project.id,
            task_id: delivery.id,
            title: None,
            description: None,
            status: Some(PROJECT_TASK_STATUS_IN_PROGRESS.into()),
            priority: None,
            assignee_agent_id: None,
            due_at: None,
        }),
        Err(AppError::Conflict(_))
    ));

    app.update_company_project_task(UpdateCompanyProjectTaskInput {
        actor_agent_id: engineer.agent_profile.id,
        company_id: company.company.id,
        project_id: project.project.id,
        task_id: foundation.id,
        title: None,
        description: None,
        status: Some(PROJECT_TASK_STATUS_DONE.into()),
        priority: None,
        assignee_agent_id: None,
        due_at: None,
    })
    .expect("foundation should complete");
    app.update_company_project_task(UpdateCompanyProjectTaskInput {
        actor_agent_id: engineer.agent_profile.id,
        company_id: company.company.id,
        project_id: project.project.id,
        task_id: delivery.id,
        title: None,
        description: None,
        status: Some(PROJECT_TASK_STATUS_IN_PROGRESS.into()),
        priority: None,
        assignee_agent_id: None,
        due_at: None,
    })
    .expect("delivery should start after foundation resolves");
    assert!(matches!(
        app.add_company_project_task_dependency(ChangeCompanyProjectTaskDependencyInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            project_id: project.project.id,
            task_id: delivery.id,
            depends_on_task_id: late_requirement.id,
            dependency_condition: None,
        }),
        Err(AppError::Conflict(_))
    ));
    let delivery_batch = app
        .batch_update_company_project_tasks(BatchUpdateCompanyProjectTasksInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            project_id: project.project.id,
            task_ids: vec![delivery.id],
            status: Some(PROJECT_TASK_STATUS_DONE.into()),
            priority: Some(PROJECT_TASK_PRIORITY_URGENT.into()),
            assignee_agent_id: None,
            clear_assignee: false,
            due_at: None,
            clear_due_at: false,
        })
        .expect("manager should batch-complete delivery");
    assert_eq!(delivery_batch[0].status, PROJECT_TASK_STATUS_DONE);
    assert_eq!(delivery_batch[0].priority, PROJECT_TASK_PRIORITY_URGENT);

    let launch_due_at = now_utc() + Duration::days(7);
    let launch_batch = app
        .batch_update_company_project_tasks(BatchUpdateCompanyProjectTasksInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            project_id: project.project.id,
            task_ids: vec![launch.id],
            status: Some(PROJECT_TASK_STATUS_IN_PROGRESS.into()),
            priority: Some(PROJECT_TASK_PRIORITY_HIGH.into()),
            assignee_agent_id: Some(manager.agent_profile.id),
            clear_assignee: false,
            due_at: Some(launch_due_at),
            clear_due_at: false,
        })
        .expect("launch should start after delivery resolves");
    assert_eq!(
        launch_batch[0].assignee_agent_id,
        Some(manager.agent_profile.id)
    );
    assert_eq!(launch_batch[0].due_at, Some(launch_due_at));
    assert_eq!(
        launch_batch[0].updated_by_agent_id,
        Some(manager.agent_profile.id)
    );

    assert!(matches!(
        app.batch_update_company_project_tasks(BatchUpdateCompanyProjectTasksInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            project_id: project.project.id,
            task_ids: vec![launch.id, Uuid::new_v4()],
            status: None,
            priority: Some(PROJECT_TASK_PRIORITY_LOW.into()),
            assignee_agent_id: None,
            clear_assignee: false,
            due_at: None,
            clear_due_at: false,
        }),
        Err(AppError::NotFound(_))
    ));
    assert_eq!(
        app.get_company_project(GetCompanyProjectInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            project_id: project.project.id,
        })
        .expect("project should remain readable")
        .tasks
        .into_iter()
        .find(|task| task.id == launch.id)
        .expect("launch task should remain")
        .priority,
        PROJECT_TASK_PRIORITY_HIGH
    );

    let review = app
        .create_company_project_task(CreateCompanyProjectTaskInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            project_id: project.project.id,
            title: "独立评审".into(),
            description: None,
            priority: Some(PROJECT_TASK_PRIORITY_NORMAL.into()),
            assignee_agent_id: Some(manager.agent_profile.id),
            due_at: None,
        })
        .expect("review task");
    let rework = app
        .create_company_project_task(CreateCompanyProjectTaskInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            project_id: project.project.id,
            title: "评审拒绝后的返工".into(),
            description: None,
            priority: Some(PROJECT_TASK_PRIORITY_HIGH.into()),
            assignee_agent_id: Some(engineer.agent_profile.id),
            due_at: None,
        })
        .expect("rework task");
    app.add_company_project_task_dependency(ChangeCompanyProjectTaskDependencyInput {
        actor_agent_id: manager.agent_profile.id,
        company_id: company.company.id,
        project_id: project.project.id,
        task_id: rework.id,
        depends_on_task_id: review.id,
        dependency_condition: Some(PROJECT_TASK_DEPENDENCY_COMPLETION.into()),
    })
    .expect("rework should wait for review completion even when rejected");
    app.update_company_project_task(UpdateCompanyProjectTaskInput {
        actor_agent_id: manager.agent_profile.id,
        company_id: company.company.id,
        project_id: project.project.id,
        task_id: review.id,
        title: None,
        description: None,
        status: Some(PROJECT_TASK_STATUS_FAILED.into()),
        priority: None,
        assignee_agent_id: None,
        due_at: None,
    })
    .expect("review can record a rejected result");
    app.update_company_project_task(UpdateCompanyProjectTaskInput {
        actor_agent_id: engineer.agent_profile.id,
        company_id: company.company.id,
        project_id: project.project.id,
        task_id: rework.id,
        title: None,
        description: None,
        status: Some(PROJECT_TASK_STATUS_IN_PROGRESS.into()),
        priority: None,
        assignee_agent_id: None,
        due_at: None,
    })
    .expect("failed review completion should unlock rework");

    app.remove_company_project_task_dependency(ChangeCompanyProjectTaskDependencyInput {
        actor_agent_id: manager.agent_profile.id,
        company_id: company.company.id,
        project_id: project.project.id,
        task_id: launch.id,
        depends_on_task_id: delivery.id,
        dependency_condition: None,
    })
    .expect("dependency should be removable");
    let final_view = app
        .get_company_project(GetCompanyProjectInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            project_id: project.project.id,
        })
        .expect("project view should include dependency graph");
    assert!(final_view.task_dependencies.iter().any(|dependency| {
        dependency.task_id == delivery.id && dependency.depends_on_task_id == foundation.id
    }));
    assert!(final_view.task_dependencies.iter().any(|dependency| {
        dependency.task_id == rework.id
            && dependency.depends_on_task_id == review.id
            && dependency.dependency_condition == PROJECT_TASK_DEPENDENCY_COMPLETION
    }));
    assert!(!final_view.task_dependencies.iter().any(|dependency| {
        dependency.task_id == launch.id && dependency.depends_on_task_id == delivery.id
    }));
}
