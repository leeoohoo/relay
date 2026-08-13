use super::tools::*;
use super::*;
use ai_chat_application::{
    CreateCompanyAgentInput, CreateCompanyInput, CreateCompanyProjectForHumanInput, DevLoginInput,
    MemoryPlatformRepository,
};
use ai_chat_domain::company::{
    COMPANY_AGENT_ROLE_MANAGER, COMPANY_AGENT_ROLE_MEMBER,
    COMPANY_PERMISSION_PROJECT_ASSETS_MANAGE, COMPANY_PERMISSION_PROJECT_RULES_MANAGE,
    PROJECT_TYPE_SOURCE_HUMAN,
};

#[test]
fn standard_surface_has_seven_identity_memory_session_snapshot_and_inbox_tools() {
    let tools = standard_mcp_tools();
    let names = tools
        .iter()
        .map(|tool| tool.name.as_ref())
        .collect::<Vec<_>>();

    assert_eq!(names.len(), 7);
    assert!(names.contains(&"agent.bootstrap"));
    assert!(names.contains(&"agent.control_snapshot"));
    assert!(names.contains(&"agent.profile.update"));
    assert!(names.contains(&"agent.memory"));
    assert!(names.contains(&"agent.work_session"));
    assert!(names.contains(&"agent.inbox.wait"));
    assert!(names.contains(&"agent.inbox.ack"));
    assert!(tools.iter().all(|tool| {
        tool.input_schema
            .get("type")
            .and_then(Value::as_str)
            .is_some_and(|value| value == "object")
    }));
}

#[test]
fn memory_source_refs_accept_git_commit_ids_and_describe_identifier_rules() {
    let commit_sha = "7ed28bec6af9d8cbd8b438f371bd70299a0c4858";
    let input: AgentMemoryToolInput = handler::parse_input(json!({
        "action": "remember",
        "company_id": Uuid::new_v4(),
        "project_id": Uuid::new_v4(),
        "scope": "project",
        "memory_tier": "long_term",
        "memory_type": "handoff",
        "topic_key": "qa-rerun-result",
        "title": "QA rerun result",
        "summary": "The focused QA rerun found a release-blocking Web runtime failure.",
        "source_refs": [{
            "source_type": "git_commit",
            "source_id": commit_sha,
            "label": "QA evidence commit"
        }]
    }))
    .expect("a Git commit source reference should parse");

    let AgentMemoryOperation::Remember { source_refs, .. } = input.operation else {
        panic!("remember input should select the remember operation");
    };
    assert_eq!(source_refs.len(), 1);
    assert_eq!(source_refs[0].source_id, commit_sha);

    let memory_tool = standard_mcp_tools()
        .into_iter()
        .find(|tool| tool.name.as_ref() == "agent.memory")
        .expect("memory tool should exist");
    let schema =
        serde_json::to_string(&memory_tool.input_schema).expect("memory schema should serialize");
    assert!(schema.contains("git_commit"));
    assert!(schema.contains("Do not concatenate labels or prefixes"));
}

#[test]
fn compact_surface_exposes_thirteen_tools_and_hides_legacy_names() {
    let mut tools = standard_mcp_tools();
    tools.extend(company_mcp_tools(&[
        COMPANY_PERMISSION_PROJECT_CREATE.into(),
        COMPANY_PERMISSION_PROJECT_MANAGE.into(),
        COMPANY_PERMISSION_TASK_ASSIGN.into(),
        COMPANY_PERMISSION_TASK_UPDATE.into(),
    ]));
    let names = tools
        .iter()
        .map(|tool| tool.name.as_ref())
        .collect::<Vec<_>>();
    assert_eq!(names.len(), 13);
    assert!(names.contains(&"company.chat"));
    assert!(names.contains(&"company.project"));
    assert!(names.contains(&"company.task"));
    assert!(names.contains(&"company.gate"));
    assert!(names.contains(&"company.environment"));
    assert!(names.contains(&"company.events"));
    assert!(!is_public_tool_name("agent.get_profile"));
    assert!(!is_public_tool_name("company.chat.message.send"));
    assert!(!is_public_tool_name("company.project.task.update"));
    assert!(!is_public_tool_name("company.staff.hire"));
    for name in [
        "agent.profile.update",
        "company.chat",
        "company.project",
        "company.task",
    ] {
        let tool = tools
            .iter()
            .find(|tool| tool.name.as_ref() == name)
            .expect("compact action tool should exist");
        assert!(serde_json::to_string(&tool.input_schema)
            .expect("tool schema should serialize")
            .contains("idempotency_key"));
    }
}

#[test]
fn staffing_tools_are_only_added_for_explicit_permissions() {
    assert!(staffing_mcp_tools(&[]).is_empty());

    let hire_only = staffing_mcp_tools(&[COMPANY_PERMISSION_STAFF_HIRE.into()]);
    let hire_names = hire_only
        .iter()
        .map(|tool| tool.name.as_ref())
        .collect::<Vec<_>>();
    assert_eq!(hire_names, vec!["company.staff"]);
    let hire_actions = tool_schema_actions(&hire_only[0]);
    assert!(hire_actions.contains(&"hire".into()));
    assert!(hire_actions.contains(&"action_get".into()));
    assert!(hire_actions.contains(&"action_list".into()));
    assert!(!hire_actions.contains(&"suspend".into()));
    assert!(!hire_actions.contains(&"terminate".into()));

    let all = staffing_mcp_tools(&[
        COMPANY_PERMISSION_STAFF_HIRE.into(),
        COMPANY_PERMISSION_STAFF_SUSPEND.into(),
        COMPANY_PERMISSION_STAFF_TERMINATE.into(),
    ]);
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].name.as_ref(), "company.staff");
    let schema =
        serde_json::to_string(&all[0].input_schema).expect("staffing schema should serialize");
    assert!(schema.contains("hire"));
    assert!(schema.contains("terminate"));
    assert!(schema.contains("action_list"));
}

#[test]
fn staffing_profession_keys_accept_common_aliases_and_expose_full_catalog() {
    let engineering: CompanyProfessionKeyInput = serde_json::from_str("\"engineering_manager\"")
        .expect("engineering_manager should map to technical_manager");
    assert_eq!(engineering.as_str(), "technical_manager");
    let quality: CompanyProfessionKeyInput = serde_json::from_str("\"quality_assurance\"")
        .expect("quality_assurance should map to qa_engineer");
    assert_eq!(quality.as_str(), "qa_engineer");

    let schema = schemars::schema_for!(CompanyProfessionKeyInput);
    let serialized = serde_json::to_string(&schema).expect("profession schema should serialize");
    for key in [
        "technical_manager",
        "qa_engineer",
        "game_engineer",
        "database_engineer",
        "erp_consultant",
        "wms_consultant",
    ] {
        assert!(serialized.contains(key), "schema should expose {key}");
    }
}

#[test]
fn task_execution_schema_exposes_all_fixed_value_enums() {
    let tool = company_mcp_tools(&[
        COMPANY_PERMISSION_TASK_ASSIGN.into(),
        COMPANY_PERMISSION_TASK_UPDATE.into(),
    ])
    .into_iter()
    .find(|tool| tool.name.as_ref() == "company.task")
    .expect("company.task tool");
    let serialized =
        serde_json::to_string(&tool.input_schema).expect("task schema should serialize");
    for value in [
        "execution",
        "environment_check",
        "succeeded",
        "interrupted",
        "dependency",
        "environment",
        "resolved",
        "waived",
        "retry_of",
        "report",
        "artifact",
        "informational",
    ] {
        assert!(serialized.contains(value), "schema should expose {value}");
    }
    let schema = serde_json::to_value(&tool.input_schema).expect("task schema should serialize");
    assert!(schema
        .pointer("/required")
        .and_then(Value::as_array)
        .is_some_and(|fields| fields.iter().any(|field| field == "action")));
    let action_values = schema
        .pointer("/properties/action/enum")
        .and_then(Value::as_array)
        .expect("task schema should expose a top-level action discriminator");
    assert!(action_values
        .iter()
        .any(|action| action == "evidence_create"));
}

#[test]
fn project_asset_schema_matches_application_validation_contract() {
    let tool = company_mcp_tools(&[COMPANY_PERMISSION_PROJECT_ASSETS_MANAGE.into()])
        .into_iter()
        .find(|tool| tool.name.as_ref() == "company.project")
        .expect("company.project tool");
    let schema = Value::Object((*tool.input_schema).clone());

    let metadata = schema
        .pointer("/$defs/CompanyProjectAssetToolInput/properties/metadata")
        .expect("asset metadata schema");
    assert!(schema_contains_type(metadata, "object"));
    assert!(!schema_contains_type(metadata, "string"));

    let status = schema
        .pointer("/$defs/CompanyProjectAssetStatusInput/enum")
        .and_then(Value::as_array)
        .expect("asset status enum");
    assert_eq!(
        status,
        &vec![
            json!("active"),
            json!("missing"),
            json!("deprecated"),
            json!("unknown"),
        ]
    );

    let input: CompanyProjectToolInput = handler::parse_input(json!({
        "action": "assets_replace",
        "company_id": Uuid::nil(),
        "project_id": Uuid::nil(),
        "assets": [{
            "name": "HTTP API",
            "asset_type": "service",
            "locator": "apps/server",
            "status": "active",
            "metadata": { "language": "Rust" }
        }]
    }))
    .expect("valid structured asset input should parse");
    let CompanyProjectOperation::AssetsReplace { assets, .. } = input.operation else {
        panic!("expected assets_replace operation");
    };
    assert_eq!(
        assets[0].status.as_ref().map(|value| value.as_str()),
        Some("active")
    );
    assert_eq!(
        assets[0]
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.get("language")),
        Some(&json!("Rust"))
    );

    let invalid_status = handler::parse_input::<CompanyProjectToolInput>(json!({
        "action": "assets_replace",
        "company_id": Uuid::nil(),
        "project_id": Uuid::nil(),
        "assets": [{
            "name": "HTTP API",
            "asset_type": "service",
            "locator": "apps/server",
            "status": "ready"
        }]
    }));
    assert!(invalid_status.is_err());

    for status in ["active", "missing", "deprecated", "unknown"] {
        handler::parse_input::<CompanyProjectToolInput>(json!({
            "action": "assets_replace",
            "company_id": Uuid::nil(),
            "project_id": Uuid::nil(),
            "assets": [{
                "name": "HTTP API",
                "asset_type": "service",
                "locator": "apps/server",
                "status": status
            }]
        }))
        .unwrap_or_else(|error| panic!("documented asset status {status} should parse: {error}"));
    }

    let legacy_string_metadata: CompanyProjectToolInput = handler::parse_input(json!({
        "action": "assets_replace",
        "company_id": Uuid::nil(),
        "project_id": Uuid::nil(),
        "assets": [{
            "name": "HTTP API",
            "asset_type": "service",
            "locator": "apps/server",
            "status": "planned",
            "metadata": "{\"language\":\"Rust\"}"
        }]
    }))
    .expect("legacy cached schemas should remain compatible during rollout");
    let CompanyProjectOperation::AssetsReplace { assets, .. } = legacy_string_metadata.operation
    else {
        panic!("expected assets_replace operation");
    };
    assert_eq!(
        assets[0].status.as_ref().map(|status| status.as_str()),
        Some("missing")
    );
    assert_eq!(
        assets[0]
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.get("language")),
        Some(&json!("Rust"))
    );

    let invalid_string_metadata = handler::parse_input::<CompanyProjectToolInput>(json!({
        "action": "assets_replace",
        "company_id": Uuid::nil(),
        "project_id": Uuid::nil(),
        "assets": [{
            "name": "HTTP API",
            "asset_type": "service",
            "locator": "apps/server",
            "metadata": "not-json"
        }]
    }));
    assert!(invalid_string_metadata.is_err());
}

#[test]
fn environment_observation_schema_uses_structured_objects_and_status_enums() {
    let tool = company_mcp_tools(&[COMPANY_PERMISSION_TASK_ASSIGN.into()])
        .into_iter()
        .find(|tool| tool.name.as_ref() == "company.environment")
        .expect("company.environment tool");
    let schema = Value::Object((*tool.input_schema).clone());

    let health_summary = schema_action_property(&schema, "observe", "health_summary")
        .expect("health_summary schema");
    assert!(schema_contains_type(health_summary, "object"));
    assert!(!schema_contains_type(health_summary, "string"));

    let health_details = schema
        .pointer("/$defs/ProjectEnvironmentServiceObservationToolInput/properties/health_details")
        .expect("service health_details schema");
    assert!(schema_contains_type(health_details, "object"));
    assert!(!schema_contains_type(health_details, "string"));

    for (definition, expected) in [
        (
            "ProjectEnvironmentStatusInput",
            vec!["unknown", "provisioning", "ready", "degraded", "offline"],
        ),
        (
            "ProjectEnvironmentServiceHealthStatusInput",
            vec!["unknown", "healthy", "unhealthy"],
        ),
    ] {
        let values = schema
            .pointer(&format!("/$defs/{definition}/enum"))
            .and_then(Value::as_array)
            .expect("environment status enum");
        assert_eq!(
            values,
            &expected.into_iter().map(Value::from).collect::<Vec<_>>()
        );
    }

    handler::parse_input::<dispatch_environment::CompanyEnvironmentToolInput>(json!({
        "action": "observe",
        "company_id": Uuid::nil(),
        "project_id": Uuid::nil(),
        "environment_id": Uuid::nil(),
        "status": "ready",
        "health_summary": { "message": "healthy" },
        "services": [{
            "service_key": "web",
            "health_status": "healthy",
            "health_details": { "status_code": 200 }
        }]
    }))
    .expect("valid environment observation should parse");

    handler::parse_input::<dispatch_environment::CompanyEnvironmentToolInput>(json!({
        "action": "observe",
        "company_id": Uuid::nil(),
        "project_id": Uuid::nil(),
        "environment_id": Uuid::nil(),
        "status": "ready",
        "health_summary": "{\"message\":\"healthy\"}",
        "services": [{
            "service_key": "web",
            "health_status": "healthy",
            "health_details": "{\"status_code\":200}"
        }]
    }))
    .expect("legacy string-encoded environment objects should parse during rollout");

    let invalid_string_summary =
        handler::parse_input::<dispatch_environment::CompanyEnvironmentToolInput>(json!({
            "action": "observe",
            "company_id": Uuid::nil(),
            "project_id": Uuid::nil(),
            "environment_id": Uuid::nil(),
            "status": "ready",
            "health_summary": "not-json"
        }));
    assert!(invalid_string_summary.is_err());
}

#[test]
fn evidence_metrics_default_to_an_empty_object() {
    let input: CompanyTaskToolInput = serde_json::from_value(json!({
        "action": "evidence_create",
        "company_id": Uuid::nil(),
        "project_id": Uuid::nil(),
        "evidence_type": "report",
        "title": "Review",
        "summary": "Reviewed",
        "result": "informational",
        "artifact_refs": []
    }))
    .expect("evidence input should accept omitted metrics");

    let CompanyTaskOperation::EvidenceCreate { metrics, .. } = input.operation else {
        panic!("expected evidence_create operation");
    };
    assert!(metrics.is_empty());
}

#[test]
fn evidence_input_accepts_artifact_as_a_first_class_type() {
    let input: CompanyTaskToolInput = serde_json::from_value(json!({
        "action": "evidence_create",
        "company_id": Uuid::nil(),
        "project_id": Uuid::nil(),
        "evidence_type": "artifact",
        "title": "Delivery artifact",
        "summary": "Committed project deliverable",
        "result": "informational",
        "artifact_refs": []
    }))
    .expect("evidence input should accept artifact evidence");

    let CompanyTaskOperation::EvidenceCreate { evidence_type, .. } = input.operation else {
        panic!("expected evidence_create operation");
    };
    assert_eq!(evidence_type.as_str(), "artifact");
}

#[test]
fn active_company_agents_receive_six_company_domain_tools() {
    let tools = company_mcp_tools(&[]);
    let names = tools
        .iter()
        .map(|tool| tool.name.as_ref())
        .collect::<Vec<_>>();
    assert_eq!(names.len(), 6);
    assert!(names.contains(&"company.chat"));
    assert!(names.contains(&"company.project"));
    assert!(names.contains(&"company.task"));
    assert!(names.contains(&"company.gate"));
    assert!(names.contains(&"company.environment"));
    assert!(names.contains(&"company.events"));
}

#[test]
fn repeated_work_session_dispatch_returns_the_existing_intent() {
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
}

#[test]
fn every_tool_result_surfaces_pending_message_notice_until_acknowledged() {
    let app = PlatformApp::new(MemoryPlatformRepository::default());
    let owner = app
        .dev_login(DevLoginInput {
            email: "mcp-inbox-notice-owner@example.com".into(),
            display_name: "MCP Inbox Notice Owner".into(),
        })
        .expect("owner should be created");
    let company = app
        .create_company(CreateCompanyInput {
            human_user_id: owner.id,
            name: "MCP Inbox Notice Company".into(),
            slug: Some("mcp-inbox-notice-company".into()),
            description: None,
        })
        .expect("company should be created");
    let manager = app
        .create_company_agent(CreateCompanyAgentInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            display_name: "Notice Manager".into(),
            handle: "notice-manager".into(),
            persona: "负责发送消息".into(),
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
            display_name: "Notice Engineer".into(),
            handle: "notice-engineer".into(),
            persona: "负责处理消息".into(),
            org_unit_id: None,
            job_title: None,
            role_key: Some(COMPANY_AGENT_ROLE_MEMBER.into()),
            reports_to_membership_id: Some(manager.membership.id),
        })
        .expect("engineer should be created");
    let group_id = company.conversations[0].preview.id;
    let gateway = McpGateway::new(app.clone(), None);
    gateway
        .invoke(
            Some(&manager.agent_key_plaintext),
            "company.chat",
            json!({
                "action": "send",
                "company_id": company.company.id,
                "conversation_id": group_id,
                "content": "请立即处理这条新的项目消息。",
                "mentioned_agent_ids": [engineer.agent_profile.id],
                "mention_all": false,
                "idempotency_key": "mcp-inbox-notice-message"
            }),
        )
        .expect("manager should send a message");

    let handler = AiChatMcpHandler::new(gateway.clone());
    let notice = handler
        .pending_message_notice(engineer.agent_profile.id)
        .expect("pending message should produce a notice");
    assert_eq!(notice["attention_required"], true);
    assert_eq!(notice["pending_message_count"], 1);
    assert_eq!(
        notice["latest_messages"][0]["content_preview"],
        "请立即处理这条新的项目消息。"
    );

    let result = handler.structured_success(
        "company.project",
        engineer.agent_profile.id,
        json!({ "projects": [] }),
        true,
    );
    let serialized = serde_json::to_value(result).expect("tool result should serialize");
    assert_eq!(
        serialized["structuredContent"]["inbox_notice"]["attention_required"],
        true
    );

    let worker_result = handler.structured_success(
        "company.project",
        engineer.agent_profile.id,
        json!({ "projects": [] }),
        false,
    );
    let worker_serialized =
        serde_json::to_value(worker_result).expect("worker tool result should serialize");
    assert!(worker_serialized["structuredContent"]
        .get("inbox_notice")
        .is_none());

    let event_id = app
        .list_agent_inbox_events(engineer.agent_profile.id, true, 10)
        .expect("engineer inbox should list")
        .into_iter()
        .find(|event| event.event_type == "message.received")
        .expect("message event should exist")
        .id;
    gateway
        .invoke(
            Some(&engineer.agent_key_plaintext),
            "agent.inbox.ack",
            json!({ "event_id": event_id }),
        )
        .expect("engineer should acknowledge the message");
    assert!(handler
        .pending_message_notice(engineer.agent_profile.id)
        .is_none());
}

#[test]
fn relay_session_kind_header_is_parsed_for_trigger_requests() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-relay-session-kind",
        "project".parse().expect("valid header"),
    );
    assert_eq!(
        handler::relay_session_kind_from_headers(&headers).as_deref(),
        Some("project")
    );
}

#[test]
fn company_action_schemas_keep_hot_grant_project_actions_visible() {
    let member_tools = company_mcp_tools(&[COMPANY_PERMISSION_TASK_UPDATE.into()]);
    let member_project = member_tools
        .iter()
        .find(|tool| tool.name.as_ref() == "company.project")
        .expect("member project tool");
    let member_project_actions = tool_schema_actions(member_project);
    assert_eq!(
        member_project_actions,
        [
            "assets_replace",
            "get",
            "list",
            "rule_update",
            "status_update",
        ]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>()
    );
    let member_project_schema = serde_json::to_string(&member_project.input_schema)
        .expect("member project schema should serialize");
    assert!(!member_project_schema.contains("project_status"));

    let member_task = member_tools
        .iter()
        .find(|tool| tool.name.as_ref() == "company.task")
        .expect("member task tool");
    assert_eq!(
        tool_schema_actions(member_task),
        [
            "attempt_finish",
            "attempt_start",
            "blocker_open",
            "blocker_resolve",
            "evidence_create",
            "execution_get",
            "get",
            "list",
            "my",
            "update",
        ]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>()
    );
    let member_task_schema = serde_json::to_string(&member_task.input_schema)
        .expect("member task schema should serialize");
    assert!(member_task_schema.contains("\"status\""));
    let member_task_schema_value = Value::Object((*member_task.input_schema).clone());
    let status_schema = schema_action_property(&member_task_schema_value, "update", "status")
        .expect("member update status schema should exist");
    assert_eq!(
        status_schema
            .get("enum")
            .and_then(Value::as_array)
            .expect("member status should be an enum"),
        &vec![json!("in_progress"), json!("done"), Value::Null,]
    );
    for hidden_field in [
        "title",
        "description",
        "priority",
        "assignee_agent_id",
        "due_at",
    ] {
        assert!(!schema_action_has_property(
            &member_task_schema_value,
            "update",
            hidden_field,
        ));
    }

    let manager_tools = company_mcp_tools(&[
        COMPANY_PERMISSION_PROJECT_CREATE.into(),
        COMPANY_PERMISSION_PROJECT_MANAGE.into(),
        COMPANY_PERMISSION_PROJECT_RULES_MANAGE.into(),
        COMPANY_PERMISSION_PROJECT_ASSETS_MANAGE.into(),
        COMPANY_PERMISSION_TASK_ASSIGN.into(),
        COMPANY_PERMISSION_TASK_UPDATE.into(),
    ]);
    let manager_project = manager_tools
        .iter()
        .find(|tool| tool.name.as_ref() == "company.project")
        .expect("manager project tool");
    let manager_project_actions = tool_schema_actions(manager_project);
    for action in [
        "create",
        "git_provision",
        "update",
        "member_add",
        "member_remove",
        "owner_transfer",
        "rule_update",
        "assets_replace",
    ] {
        assert!(manager_project_actions.contains(&action.to_string()));
    }
    let manager_task = manager_tools
        .iter()
        .find(|tool| tool.name.as_ref() == "company.task")
        .expect("manager task tool");
    let manager_task_actions = tool_schema_actions(manager_task);
    for action in [
        "get",
        "list",
        "my",
        "create",
        "update",
        "batch_update",
        "dependency_add",
        "dependency_remove",
    ] {
        assert!(manager_task_actions.contains(&action.to_string()));
    }
}

#[test]
fn task_reads_do_not_require_idempotency_or_mutation_auditing() {
    for action in ["get", "list", "my"] {
        assert!(!is_mutating_tool(
            "company.task",
            &json!({ "action": action })
        ));
    }
    assert!(is_mutating_tool(
        "company.task",
        &json!({ "action": "update" })
    ));
}

#[test]
fn assigned_agent_can_read_tasks_through_get_list_and_my_actions() {
    let app = PlatformApp::new(MemoryPlatformRepository::default());
    let owner = app
        .dev_login(DevLoginInput {
            email: "mcp-task-owner@example.com".into(),
            display_name: "MCP Task Owner".into(),
        })
        .expect("owner should be created");
    let company = app
        .create_company(CreateCompanyInput {
            human_user_id: owner.id,
            name: "MCP Task Company".into(),
            slug: Some("mcp-task-company".into()),
            description: None,
        })
        .expect("company should be created");
    let manager = app
        .create_company_agent(CreateCompanyAgentInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            display_name: "MCP Task Manager".into(),
            handle: "mcp-task-manager".into(),
            persona: "负责项目任务".into(),
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
            display_name: "MCP Task Engineer".into(),
            handle: "mcp-task-engineer".into(),
            persona: "负责执行任务".into(),
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
            name: "MCP Task Project".into(),
            description: None,
            member_agent_ids: vec![engineer.agent_profile.id],
        })
        .expect("project should be created");
    let prerequisite = app
        .create_company_project_task(CreateCompanyProjectTaskInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            project_id: project.project.id,
            title: "准备 MCP 测试数据".into(),
            description: None,
            priority: Some("high".into()),
            assignee_agent_id: Some(manager.agent_profile.id),
            due_at: None,
        })
        .expect("prerequisite should be created");
    let task = app
        .create_company_project_task(CreateCompanyProjectTaskInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            project_id: project.project.id,
            title: "验证任务读取 MCP".into(),
            description: None,
            priority: Some("high".into()),
            assignee_agent_id: Some(engineer.agent_profile.id),
            due_at: None,
        })
        .expect("task should be created");
    app.add_company_project_task_dependency(ChangeCompanyProjectTaskDependencyInput {
        actor_agent_id: manager.agent_profile.id,
        company_id: company.company.id,
        project_id: project.project.id,
        task_id: task.id,
        depends_on_task_id: prerequisite.id,
        dependency_condition: None,
    })
    .expect("task dependency should be created");
    let gateway = McpGateway::new(app.clone(), None);

    let mine = gateway
        .invoke(
            Some(&engineer.agent_key_plaintext),
            "company.task",
            json!({ "action": "my", "company_id": company.company.id }),
        )
        .expect("assigned Agent should list its tasks");
    assert_eq!(
        mine.output["assignments"][0]["task"]["id"],
        task.id.to_string()
    );
    assert_eq!(mine.output["waiting_count"], 1);
    assert_eq!(
        mine.output["assignments"][0]["readiness"],
        "waiting_for_dependencies"
    );
    assert_eq!(mine.output["assignments"][0]["can_start"], false);
    assert_eq!(
        mine.output["assignments"][0]["unresolved_dependencies"][0]["task_id"],
        prerequisite.id.to_string()
    );

    app.update_company_project_task(UpdateCompanyProjectTaskInput {
        actor_agent_id: manager.agent_profile.id,
        company_id: company.company.id,
        project_id: project.project.id,
        task_id: prerequisite.id,
        title: None,
        description: None,
        status: Some("done".into()),
        priority: None,
        assignee_agent_id: None,
        due_at: None,
    })
    .expect("prerequisite should complete");
    let ready = gateway
        .invoke(
            Some(&engineer.agent_key_plaintext),
            "company.task",
            json!({ "action": "my", "company_id": company.company.id }),
        )
        .expect("assigned Agent should re-check task readiness");
    assert_eq!(ready.output["ready_count"], 1);
    assert_eq!(ready.output["assignments"][0]["readiness"], "ready");
    assert_eq!(ready.output["assignments"][0]["can_start"], true);

    let listed = gateway
        .invoke(
            Some(&engineer.agent_key_plaintext),
            "company.task",
            json!({
                "action": "list",
                "company_id": company.company.id,
                "project_id": project.project.id,
                "assignee_agent_id": engineer.agent_profile.id,
                "status": "todo"
            }),
        )
        .expect("project member should filter project tasks");
    assert_eq!(listed.output["tasks"][0]["id"], task.id.to_string());

    let fetched = gateway
        .invoke(
            Some(&engineer.agent_key_plaintext),
            "company.task",
            json!({
                "action": "get",
                "company_id": company.company.id,
                "project_id": project.project.id,
                "task_id": task.id
            }),
        )
        .expect("project member should fetch one task");
    assert_eq!(fetched.output["task"]["id"], task.id.to_string());
}

fn tool_schema_actions(tool: &Tool) -> Vec<String> {
    fn collect(value: &Value, actions: &mut Vec<String>) {
        if let Some(action) = schema_action_name(value) {
            if !actions.contains(&action) {
                actions.push(action);
            }
        }
        match value {
            Value::Object(object) => {
                for child in object.values() {
                    collect(child, actions);
                }
            }
            Value::Array(items) => {
                for item in items {
                    collect(item, actions);
                }
            }
            _ => {}
        }
    }

    let mut actions = Vec::new();
    collect(&Value::Object((*tool.input_schema).clone()), &mut actions);
    actions.sort();
    actions
}

fn schema_action_has_property(value: &Value, action: &str, property: &str) -> bool {
    if schema_action_name(value).as_deref() == Some(action)
        && value
            .get("properties")
            .and_then(Value::as_object)
            .is_some_and(|properties| properties.contains_key(property))
    {
        return true;
    }
    match value {
        Value::Object(object) => object
            .values()
            .any(|child| schema_action_has_property(child, action, property)),
        Value::Array(items) => items
            .iter()
            .any(|item| schema_action_has_property(item, action, property)),
        _ => false,
    }
}

fn schema_action_property<'a>(value: &'a Value, action: &str, property: &str) -> Option<&'a Value> {
    if schema_action_name(value).as_deref() == Some(action) {
        if let Some(field) = value
            .get("properties")
            .and_then(Value::as_object)
            .and_then(|properties| properties.get(property))
        {
            return Some(field);
        }
    }
    match value {
        Value::Object(object) => object
            .values()
            .find_map(|child| schema_action_property(child, action, property)),
        Value::Array(items) => items
            .iter()
            .find_map(|item| schema_action_property(item, action, property)),
        _ => None,
    }
}

fn schema_contains_type(value: &Value, expected: &str) -> bool {
    match value {
        Value::Object(object) => {
            object.get("type").is_some_and(|value| match value {
                Value::String(value) => value == expected,
                Value::Array(values) => values.iter().any(|value| value == expected),
                _ => false,
            }) || object
                .values()
                .any(|child| schema_contains_type(child, expected))
        }
        Value::Array(items) => items
            .iter()
            .any(|item| schema_contains_type(item, expected)),
        _ => false,
    }
}

#[test]
fn agent_key_supports_custom_header_and_bearer_token() {
    let mut headers = HeaderMap::new();
    headers.insert("x-agent-key", "agk_custom".parse().unwrap());
    assert_eq!(
        agent_key_from_headers(&headers).as_deref(),
        Some("agk_custom")
    );

    headers.remove("x-agent-key");
    headers.insert(AUTHORIZATION, "bearer agk_bearer".parse().unwrap());
    assert_eq!(
        agent_key_from_headers(&headers).as_deref(),
        Some("agk_bearer")
    );
}
