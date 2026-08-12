use super::*;
use ai_chat_domain::company::{
    TASK_ATTEMPT_STATUS_FAILED, TASK_ATTEMPT_STATUS_RUNNING, TASK_BLOCKER_STATUS_RESOLVED,
};

#[test]
fn attempts_blockers_relations_and_evidence_preserve_task_business_state() {
    let app = PlatformApp::new(MemoryPlatformRepository::default());
    let owner = app
        .dev_login(DevLoginInput {
            email: "execution-owner@example.com".into(),
            display_name: "Execution Owner".into(),
        })
        .unwrap();
    let company = app
        .create_company(CreateCompanyInput {
            human_user_id: owner.id,
            name: "Execution Company".into(),
            slug: Some("execution-company".into()),
            description: None,
        })
        .unwrap();
    let manager = app
        .create_company_agent(CreateCompanyAgentInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            display_name: "Execution Manager".into(),
            handle: "execution-manager".into(),
            persona: "负责执行治理".into(),
            org_unit_id: None,
            job_title: Some("项目经理".into()),
            role_key: Some(COMPANY_AGENT_ROLE_MANAGER.into()),
            reports_to_membership_id: None,
        })
        .unwrap();
    let engineer = app
        .create_company_agent(CreateCompanyAgentInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            display_name: "Execution Engineer".into(),
            handle: "execution-engineer".into(),
            persona: "负责实现".into(),
            org_unit_id: None,
            job_title: Some("软件工程师".into()),
            role_key: Some(COMPANY_AGENT_ROLE_MEMBER.into()),
            reports_to_membership_id: Some(manager.membership.id),
        })
        .unwrap();
    let project = app
        .create_company_project(CreateCompanyProjectInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            name: "Execution Project".into(),
            description: None,
            member_agent_ids: vec![engineer.agent_profile.id],
        })
        .unwrap();
    let task = app
        .create_company_project_task(CreateCompanyProjectTaskInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            project_id: project.project.id,
            title: "实现执行历史".into(),
            description: None,
            priority: None,
            assignee_agent_id: Some(engineer.agent_profile.id),
            due_at: None,
        })
        .unwrap();
    let related = app
        .create_company_project_task(CreateCompanyProjectTaskInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            project_id: project.project.id,
            title: "复测执行历史".into(),
            description: None,
            priority: None,
            assignee_agent_id: Some(engineer.agent_profile.id),
            due_at: None,
        })
        .unwrap();

    let attempt = app
        .start_project_task_attempt(StartProjectTaskAttemptInput {
            actor_agent_id: engineer.agent_profile.id,
            company_id: company.company.id,
            project_id: project.project.id,
            task_id: task.id,
            intent_id: None,
            attempt_type: "execution".into(),
            objective: "实现第一版并运行测试".into(),
        })
        .unwrap();
    assert_eq!(attempt.status, TASK_ATTEMPT_STATUS_RUNNING);
    let duplicate = app
        .start_project_task_attempt(StartProjectTaskAttemptInput {
            actor_agent_id: engineer.agent_profile.id,
            company_id: company.company.id,
            project_id: project.project.id,
            task_id: task.id,
            intent_id: None,
            attempt_type: "execution".into(),
            objective: "重复开始".into(),
        })
        .expect_err("only one active attempt is allowed");
    assert!(duplicate
        .to_string()
        .contains("task_attempt_already_running"));

    app.finish_project_task_attempt(FinishProjectTaskAttemptInput {
        actor_agent_id: engineer.agent_profile.id,
        company_id: company.company.id,
        project_id: project.project.id,
        task_id: task.id,
        attempt_id: attempt.id,
        status: TASK_ATTEMPT_STATUS_FAILED.into(),
        result_summary: "测试发现环境依赖缺失".into(),
        failure_category: Some("environment".into()),
    })
    .unwrap();
    let task_after_failure = app
        .get_company_project(GetCompanyProjectInput {
            actor_agent_id: engineer.agent_profile.id,
            company_id: company.company.id,
            project_id: project.project.id,
        })
        .unwrap()
        .tasks
        .into_iter()
        .find(|item| item.id == task.id)
        .unwrap();
    assert_eq!(task_after_failure.status, PROJECT_TASK_STATUS_TODO);

    let blocker = app
        .open_project_task_blocker(OpenProjectTaskBlockerInput {
            actor_agent_id: engineer.agent_profile.id,
            company_id: company.company.id,
            project_id: project.project.id,
            task_id: task.id,
            attempt_id: Some(attempt.id),
            blocker_type: "environment".into(),
            summary: "缺少测试依赖".into(),
            owner_agent_id: Some(engineer.agent_profile.id),
            resolution_condition: "依赖安装并验证通过".into(),
        })
        .unwrap();
    let blocked_start = app
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
        .expect_err("open blocker must prevent task start");
    assert!(blocked_start.to_string().contains("task_blocker_open"));
    let waiting = app
        .agent_control_snapshot(engineer.agent_profile.id, company.company.id)
        .unwrap();
    assert!(waiting.waiting_tasks.iter().any(|item| item.id == task.id));

    let resolved = app
        .resolve_project_task_blocker(ResolveProjectTaskBlockerInput {
            actor_agent_id: engineer.agent_profile.id,
            company_id: company.company.id,
            project_id: project.project.id,
            task_id: task.id,
            blocker_id: blocker.id,
            status: "resolved".into(),
            resolution_summary: "依赖已安装并验证".into(),
        })
        .unwrap();
    assert_eq!(resolved.status, TASK_BLOCKER_STATUS_RESOLVED);
    let relation = app
        .add_project_task_relation(AddProjectTaskRelationInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            project_id: project.project.id,
            source_task_id: related.id,
            target_task_id: task.id,
            relation_type: "validates".into(),
        })
        .unwrap();
    assert_eq!(relation.relation_type, "validates");

    let evidence_input = CreateProjectEvidenceInput {
        actor_agent_id: engineer.agent_profile.id,
        company_id: company.company.id,
        project_id: project.project.id,
        task_id: Some(task.id),
        attempt_id: Some(attempt.id),
        gate_id: None,
        environment_id: None,
        evidence_type: "test".into(),
        title: "单元测试结果".into(),
        summary: "全部测试通过".into(),
        result: "passed".into(),
        artifact_refs: vec![],
        metrics: json!({"passed": 42}),
        dedupe_key: Some("test:task:execution-history:v1".into()),
    };
    let evidence = app.create_project_evidence(evidence_input.clone()).unwrap();
    let repeated = app.create_project_evidence(evidence_input).unwrap();
    assert_eq!(evidence.id, repeated.id);

    let view = app
        .get_project_task_execution(
            engineer.agent_profile.id,
            company.company.id,
            project.project.id,
            task.id,
        )
        .unwrap();
    assert_eq!(view.attempts.len(), 1);
    assert_eq!(view.blockers.len(), 1);
    assert_eq!(view.relations.len(), 1);
    assert_eq!(view.evidence.len(), 1);
}
