use super::*;
use ai_chat_domain::company::{
    PROJECT_ENVIRONMENT_SERVICE_HEALTH_HEALTHY, PROJECT_ENVIRONMENT_SERVICE_HEALTH_UNHEALTHY,
    PROJECT_ENVIRONMENT_STATUS_READY, PROJECT_TASK_STATUS_IN_PROGRESS,
};

#[test]
fn project_environment_controls_task_readiness_and_emits_one_ready_event() {
    let app = PlatformApp::new(MemoryPlatformRepository::default());
    let owner = app
        .dev_login(DevLoginInput {
            email: "environment-owner@example.com".into(),
            display_name: "Environment Owner".into(),
        })
        .expect("owner should be created");
    let company = app
        .create_company(CreateCompanyInput {
            human_user_id: owner.id,
            name: "Environment Company".into(),
            slug: Some("environment-company".into()),
            description: None,
        })
        .expect("company should be created");
    let manager = app
        .create_company_agent(CreateCompanyAgentInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            display_name: "Environment Manager".into(),
            handle: "environment-manager".into(),
            persona: "负责项目运行环境".into(),
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
            display_name: "Environment Engineer".into(),
            handle: "environment-engineer".into(),
            persona: "负责验证环境门禁".into(),
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
            name: "Environment Project".into(),
            description: Some("验证项目环境结构化就绪条件".into()),
            member_agent_ids: vec![engineer.agent_profile.id],
        })
        .expect("project should be created");
    let task = app
        .create_company_project_task(CreateCompanyProjectTaskInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            project_id: project.project.id,
            title: "部署并验证 Web 服务".into(),
            description: Some("staging rev-2 健康后才能开始".into()),
            priority: None,
            assignee_agent_id: Some(engineer.agent_profile.id),
            due_at: None,
        })
        .expect("task should be created");
    let environment = app
        .create_project_environment(CreateProjectEnvironmentInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            project_id: project.project.id,
            environment_key: "staging".into(),
            display_name: "Staging".into(),
            desired_revision: Some("rev-2".into()),
        })
        .expect("environment should be created");
    app.set_project_task_environment_requirement(SetProjectTaskEnvironmentRequirementInput {
        actor_agent_id: manager.agent_profile.id,
        company_id: company.company.id,
        project_id: project.project.id,
        task_id: task.id,
        environment_id: environment.id,
        required_revision: Some("rev-2".into()),
        required_services: vec!["web".into()],
        require_healthy: true,
    })
    .expect("environment requirement should be created");

    app.observe_project_environment(ObserveProjectEnvironmentInput {
        actor_agent_id: manager.agent_profile.id,
        company_id: company.company.id,
        project_id: project.project.id,
        environment_id: environment.id,
        status: PROJECT_ENVIRONMENT_STATUS_READY.into(),
        desired_revision: Some("rev-2".into()),
        observed_revision: Some("rev-2".into()),
        configuration_fingerprint: Some("config-a".into()),
        health_summary: json!({"message": "web unhealthy"}),
        observed_at: None,
        services: vec![ProjectEnvironmentServiceObservationInput {
            service_key: "web".into(),
            desired_revision: Some("rev-2".into()),
            observed_revision: Some("rev-2".into()),
            image_digest: None,
            configuration_fingerprint: Some("config-a".into()),
            health_status: PROJECT_ENVIRONMENT_SERVICE_HEALTH_UNHEALTHY.into(),
            health_details: json!({}),
        }],
    })
    .expect("degraded observation should be stored");
    let waiting_snapshot = app
        .agent_control_snapshot(engineer.agent_profile.id, company.company.id)
        .expect("waiting snapshot should load");
    assert!(waiting_snapshot.ready_tasks.is_empty());
    assert_eq!(waiting_snapshot.waiting_tasks.len(), 1);
    let start_error = app
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
        .expect_err("unready environment must prevent task start");
    assert!(start_error
        .to_string()
        .contains("task_environment_not_ready"));

    let ready_observation = ObserveProjectEnvironmentInput {
        actor_agent_id: manager.agent_profile.id,
        company_id: company.company.id,
        project_id: project.project.id,
        environment_id: environment.id,
        status: PROJECT_ENVIRONMENT_STATUS_READY.into(),
        desired_revision: Some("rev-2".into()),
        observed_revision: Some("rev-2".into()),
        configuration_fingerprint: Some("config-a".into()),
        health_summary: json!({"message": "healthy"}),
        observed_at: None,
        services: vec![ProjectEnvironmentServiceObservationInput {
            service_key: "web".into(),
            desired_revision: Some("rev-2".into()),
            observed_revision: Some("rev-2".into()),
            image_digest: None,
            configuration_fingerprint: Some("config-a".into()),
            health_status: PROJECT_ENVIRONMENT_SERVICE_HEALTH_HEALTHY.into(),
            health_details: json!({}),
        }],
    };
    app.observe_project_environment(ready_observation.clone())
        .expect("ready observation should release the task");
    app.observe_project_environment(ready_observation)
        .expect("repeated observation should be idempotent");

    let ready_snapshot = app
        .agent_control_snapshot(engineer.agent_profile.id, company.company.id)
        .expect("ready snapshot should load");
    assert_eq!(ready_snapshot.ready_tasks.len(), 1);
    assert!(ready_snapshot.waiting_tasks.is_empty());
    let ready_events = app
        .list_agent_inbox_events(engineer.agent_profile.id, true, 100)
        .expect("engineer inbox should load")
        .into_iter()
        .filter(|event| {
            event.event_type == "company.project.task_ready"
                && payload_uuid_field_optional(&event.payload_json, "task_id") == Some(task.id)
        })
        .collect::<Vec<_>>();
    assert_eq!(ready_events.len(), 1);
    app.update_company_project_task(UpdateCompanyProjectTaskInput {
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
    .expect("ready task should start");
}
