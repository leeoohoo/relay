use super::*;
use ai_chat_application::{
    CreateCompanyAgentInput, CreateCompanyInput, CreateCompanyProjectForHumanInput, DevLoginInput,
    MemoryPlatformRepository,
};
use ai_chat_domain::company::{COMPANY_AGENT_ROLE_MANAGER, PROJECT_TYPE_SOURCE_HUMAN};

#[test]
fn parse_input_reports_the_invalid_uuid_field_path() {
    let error = handler::parse_input::<AgentWorkSessionToolInput>(json!({
        "action": "dispatch",
        "company_id": Uuid::nil(),
        "project_id": "5",
        "objective": "Validate field path errors",
        "acceptance_criteria": []
    }))
    .expect_err("invalid project id should fail");
    let message = error.to_string();
    assert!(message.contains("invalid field project_id"), "{message}");
    assert!(message.contains("expected a full UUID"), "{message}");

    handler::parse_input::<CompanyProjectToolInput>(json!({
        "action": "assets_replace",
        "company_id": Uuid::nil(),
        "project_id": Uuid::nil(),
        "assets": [{
            "name": "Build output",
            "asset_type": "artifact",
            "locator": "dist/app.js",
            "metadata": { "build_id": "external-build-5" }
        }]
    }))
    .expect("open metadata identifiers must not be forced to UUIDs");
}

#[test]
fn project_get_resolves_safe_references_and_explains_ambiguous_ids() {
    let app = PlatformApp::new(MemoryPlatformRepository::default());
    let human = app
        .dev_login(DevLoginInput {
            email: "mcp-project-reference@example.com".into(),
            display_name: "MCP Project Reference".into(),
        })
        .expect("human should be created");
    let company = app
        .create_company(CreateCompanyInput {
            human_user_id: human.id,
            name: "MCP Project Reference Company".into(),
            slug: Some("mcp-project-reference-company".into()),
            description: None,
        })
        .expect("company should be created");
    let manager = app
        .create_company_agent(CreateCompanyAgentInput {
            human_user_id: human.id,
            company_id: company.company.id,
            display_name: "Project Reference Manager".into(),
            handle: "project-reference-manager".into(),
            persona: "负责项目引用测试".into(),
            org_unit_id: None,
            job_title: Some("项目经理".into()),
            role_key: Some(COMPANY_AGENT_ROLE_MANAGER.into()),
            reports_to_membership_id: None,
        })
        .expect("manager should be created");
    let first = app
        .create_company_project_for_human(CreateCompanyProjectForHumanInput {
            human_user_id: human.id,
            company_id: company.company.id,
            owner_agent_id: manager.agent_profile.id,
            name: "真实 WMS 项目".into(),
            description: None,
            member_agent_ids: Vec::new(),
            project_type: Some("web_application".into()),
            project_type_source: Some(PROJECT_TYPE_SOURCE_HUMAN.into()),
            project_type_confidence: Some(100),
            project_type_evidence: Vec::new(),
            project_id: None,
        })
        .expect("first project should be created");
    let gateway = McpGateway::new(app.clone(), None);

    let by_name = gateway
        .invoke(
            Some(&manager.agent_key_plaintext),
            "company.project",
            json!({
                "action": "get",
                "company_id": company.company.id,
                "project_id": "真实 WMS 项目"
            }),
        )
        .expect("unique exact project name should resolve for get");
    assert_eq!(
        by_name.output["project"]["project"]["id"],
        first.project.id.to_string()
    );

    let only_visible = gateway
        .invoke(
            Some(&manager.agent_key_plaintext),
            "company.project",
            json!({
                "action": "get",
                "company_id": company.company.id,
                "project_id": "5"
            }),
        )
        .expect("the only visible project should be a safe get fallback");
    assert_eq!(
        only_visible.output["project"]["project"]["id"],
        first.project.id.to_string()
    );

    let second = app
        .create_company_project_for_human(CreateCompanyProjectForHumanInput {
            human_user_id: human.id,
            company_id: company.company.id,
            owner_agent_id: manager.agent_profile.id,
            name: "第二项目".into(),
            description: None,
            member_agent_ids: Vec::new(),
            project_type: Some("web_application".into()),
            project_type_source: Some(PROJECT_TYPE_SOURCE_HUMAN.into()),
            project_type_confidence: Some(100),
            project_type_evidence: Vec::new(),
            project_id: None,
        })
        .expect("second project should be created");
    let error = gateway
        .invoke(
            Some(&manager.agent_key_plaintext),
            "company.project",
            json!({
                "action": "get",
                "company_id": company.company.id,
                "project_id": "5"
            }),
        )
        .expect_err("ambiguous list position should not resolve");
    let message = error.to_string();
    assert!(message.contains("do not use a list position"));
    assert!(message.contains(&first.project.id.to_string()));
    assert!(message.contains(&second.project.id.to_string()));

    let logs = app
        .list_agent_action_logs(manager.agent_profile.id, 20)
        .expect("logs");
    assert!(logs.iter().any(|log| {
        log.action_type == "diagnostic.company.project.get"
            && matches!(log.status, AgentActionStatus::Failed)
            && log.request_payload["project_id"] == "5"
    }));
}
