use super::*;
use ai_chat_domain::company::{PROJECT_GATE_STATUS_PASSED, PROJECT_TASK_STATUS_IN_PROGRESS};

#[test]
fn project_gate_controls_task_readiness_and_emits_one_ready_event() {
    let app = PlatformApp::new(MemoryPlatformRepository::default());
    let owner = app
        .dev_login(DevLoginInput {
            email: "gate-owner@example.com".into(),
            display_name: "Gate Owner".into(),
        })
        .expect("owner should be created");
    let company = app
        .create_company(CreateCompanyInput {
            human_user_id: owner.id,
            name: "Gate Company".into(),
            slug: Some("gate-company".into()),
            description: None,
        })
        .expect("company should be created");
    let manager = app
        .create_company_agent(CreateCompanyAgentInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            display_name: "Gate Manager".into(),
            handle: "gate-manager".into(),
            persona: "负责阶段门禁".into(),
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
            display_name: "Gate Engineer".into(),
            handle: "gate-engineer".into(),
            persona: "负责实现".into(),
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
            name: "Gate Project".into(),
            description: Some("验证结构化阶段门禁".into()),
            member_agent_ids: vec![engineer.agent_profile.id],
        })
        .expect("project should be created");
    let task = app
        .create_company_project_task(CreateCompanyProjectTaskInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            project_id: project.project.id,
            title: "实现登录页面".into(),
            description: Some("设计门禁通过后才能开始".into()),
            priority: None,
            assignee_agent_id: Some(engineer.agent_profile.id),
            due_at: None,
        })
        .expect("task should be created");
    let gate = app
        .create_project_gate(CreateProjectGateInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            project_id: project.project.id,
            gate_key: "design-approved".into(),
            gate_type: "design".into(),
            title: "登录页面设计评审".into(),
            related_task_id: Some(task.id),
            required_evidence: vec!["可编辑设计源文件".into(), "SVG 审阅稿".into()],
        })
        .expect("gate should be created");
    app.set_project_task_gate_requirement(SetProjectTaskGateRequirementInput {
        actor_agent_id: manager.agent_profile.id,
        company_id: company.company.id,
        project_id: project.project.id,
        task_id: task.id,
        gate_id: gate.id,
        required_status: PROJECT_GATE_STATUS_PASSED.into(),
    })
    .expect("task gate requirement should be created");

    let waiting_snapshot = app
        .agent_control_snapshot(engineer.agent_profile.id, company.company.id)
        .expect("control snapshot should load");
    assert!(waiting_snapshot.actionable_events.iter().all(|event| {
        event.event_type != "company.project.task_assigned"
            && event.event_type != "company.project.task_ready"
    }));
    assert!(waiting_snapshot.ready_tasks.is_empty());
    assert_eq!(waiting_snapshot.waiting_tasks.len(), 1);
    let waiting_readiness = waiting_snapshot
        .task_readiness
        .iter()
        .find(|item| item.task_id == task.id)
        .expect("waiting task readiness should be included");
    assert!(!waiting_readiness.can_start);
    assert!(waiting_readiness
        .waiting_reasons
        .iter()
        .any(|reason| reason.kind == "gate" && reason.related_id == Some(gate.id)));
    let execution = app
        .get_project_task_execution(
            engineer.agent_profile.id,
            company.company.id,
            project.project.id,
            task.id,
        )
        .expect("execution view should expose Gate readiness");
    assert_eq!(execution.readiness.gate_requirements.len(), 1);
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
        .expect_err("unresolved Gate must prevent task start");
    assert!(start_error.to_string().contains("task_gate_unresolved"));

    app.decide_project_gate(DecideProjectGateInput {
        actor_agent_id: manager.agent_profile.id,
        company_id: company.company.id,
        project_id: project.project.id,
        gate_id: gate.id,
        status: PROJECT_GATE_STATUS_PASSED.into(),
        decision_summary: "设计稿和 SVG 已评审通过".into(),
    })
    .expect("gate should pass");
    app.decide_project_gate(DecideProjectGateInput {
        actor_agent_id: manager.agent_profile.id,
        company_id: company.company.id,
        project_id: project.project.id,
        gate_id: gate.id,
        status: PROJECT_GATE_STATUS_PASSED.into(),
        decision_summary: "重复确认不应重复发出 Ready".into(),
    })
    .expect("idempotent repeated decision should remain valid");

    let ready_snapshot = app
        .agent_control_snapshot(engineer.agent_profile.id, company.company.id)
        .expect("ready snapshot should load");
    assert_eq!(ready_snapshot.ready_tasks.len(), 1);
    assert!(ready_snapshot.waiting_tasks.is_empty());
    assert!(ready_snapshot
        .task_readiness
        .iter()
        .find(|item| item.task_id == task.id)
        .is_some_and(|item| item.can_start));
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
    assert_eq!(ready_events[0].event_class, "execution_ready");
}
