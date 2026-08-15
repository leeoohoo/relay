use super::*;
use ai_chat_application::{
    CreateCompanyAgentInput, CreateCompanyInput, CreateCompanyProjectForHumanInput, DevLoginInput,
    MemoryPlatformRepository,
};
use ai_chat_domain::company::{
    AGENT_EXECUTION_INTENT_ACTION_REPLACE_SESSION, COMPANY_AGENT_ROLE_MANAGER,
    PROJECT_TYPE_SOURCE_HUMAN,
};
use serde_json::json;
use uuid::Uuid;

#[test]
fn repeated_dispatch_capability_upgrade_and_replacement_preserve_intent_contract() {
    let app = PlatformApp::new(MemoryPlatformRepository::default());
    let human = app
        .dev_login(DevLoginInput {
            email: "mcp-work-session-dedup@example.com".into(),
            display_name: "MCP Work Session Dedup".into(),
        })
        .expect("human should be created");
    let company = app
        .create_company(CreateCompanyInput {
            human_user_id: human.id,
            name: "MCP Work Session Company".into(),
            slug: Some("mcp-work-session-company".into()),
            description: None,
        })
        .expect("company should be created");
    let manager = app
        .create_company_agent(CreateCompanyAgentInput {
            human_user_id: human.id,
            company_id: company.company.id,
            display_name: "MCP Work Session Manager".into(),
            handle: "mcp-work-session-manager".into(),
            persona: "负责派发工作".into(),
            org_unit_id: None,
            job_title: Some("项目经理".into()),
            role_key: Some(COMPANY_AGENT_ROLE_MANAGER.into()),
            reports_to_membership_id: None,
        })
        .expect("manager should be created");
    let project = app
        .create_company_project_for_human(CreateCompanyProjectForHumanInput {
            human_user_id: human.id,
            company_id: company.company.id,
            owner_agent_id: manager.agent_profile.id,
            name: "MCP Work Session Project".into(),
            description: Some("验证重复派发".into()),
            member_agent_ids: Vec::new(),
            project_type: Some("web_application".into()),
            project_type_source: Some(PROJECT_TYPE_SOURCE_HUMAN.into()),
            project_type_confidence: Some(100),
            project_type_evidence: Vec::new(),
            project_id: None,
        })
        .expect("project should be created");
    let gateway = McpGateway::new(app, None);
    let request = json!({
        "action": "dispatch",
        "company_id": company.company.id,
        "project_id": project.project.id,
        "objective": "完成同一个真实目标",
        "acceptance_criteria": ["有可验证结果"],
        "priority": "high",
        "dedupe_key": "mcp-same-logical-work"
    });

    let first = gateway
        .invoke(
            Some(&manager.agent_key_plaintext),
            "agent.work_session",
            request.clone(),
        )
        .expect("first dispatch should succeed");
    let repeated = gateway
        .invoke(
            Some(&manager.agent_key_plaintext),
            "agent.work_session",
            request,
        )
        .expect("repeated dispatch should return existing work");
    assert_eq!(first.output["deduplicated"], false);
    assert_eq!(repeated.output["deduplicated"], true);
    assert_eq!(
        first.output["intent"]["id"],
        repeated.output["intent"]["id"]
    );
    assert_eq!(first.output["intent"]["required_capabilities"], json!([]));

    let intent_id = serde_json::from_value::<Uuid>(first.output["intent"]["id"].clone())
        .expect("intent id should be a UUID");
    let upgraded = gateway
        .invoke(
            Some(&manager.agent_key_plaintext),
            "agent.work_session",
            json!({
                "action": "capability_request",
                "company_id": company.company.id,
                "intent_id": intent_id,
                "capability": "browser"
            }),
        )
        .expect("browser capability request should succeed");
    assert_eq!(
        upgraded.output["intent"]["required_capabilities"],
        json!(["browser"])
    );
    assert_eq!(upgraded.output["restart_required"], true);

    let replacement = gateway
        .invoke(
            Some(&manager.agent_key_plaintext),
            "agent.work_session",
            json!({
                "action": "dispatch",
                "company_id": company.company.id,
                "project_id": project.project.id,
                "objective": "从 checkpoint 建立新的工作会话代次",
                "acceptance_criteria": ["不复用旧 Codex thread"],
                "required_capabilities": ["browser"],
                "priority": "high",
                "dedupe_key": "mcp-replace-work-session",
                "replace_session": true
            }),
        )
        .expect("replacement dispatch should succeed");
    assert_eq!(replacement.output["replacement_requested"], true);
    assert_eq!(
        replacement.output["intent"]["action_type"],
        AGENT_EXECUTION_INTENT_ACTION_REPLACE_SESSION
    );
    assert_eq!(
        replacement.output["intent"]["required_capabilities"],
        json!(["browser"])
    );
}
