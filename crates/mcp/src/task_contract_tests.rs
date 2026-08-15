use super::tools::*;
use super::*;
use ai_chat_application::{
    CreateCompanyAgentInput, CreateCompanyInput, CreateCompanyProjectInput,
    CreateCompanyProjectTaskInput, DevLoginInput, MemoryPlatformRepository,
};
use ai_chat_domain::company::{COMPANY_AGENT_ROLE_MANAGER, COMPANY_AGENT_ROLE_MEMBER};

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
fn task_input_normalization_handles_common_agent_vocabulary() {
    let cases = [
        (
            json!({
                "action": "attempt_start",
                "company_id": Uuid::nil(),
                "project_id": Uuid::nil(),
                "task_id": Uuid::nil(),
                "attempt_type": "implementation",
                "objective": "Implement the assigned scope"
            }),
            "attempt_start",
        ),
        (
            json!({
                "action": "attempt_finish",
                "company_id": Uuid::nil(),
                "project_id": Uuid::nil(),
                "task_id": Uuid::nil(),
                "attempt_id": Uuid::nil(),
                "status": "success",
                "result_summary": "Delivered and verified"
            }),
            "attempt_finish",
        ),
        (
            json!({
                "action": "blocker_open",
                "company_id": Uuid::nil(),
                "project_id": Uuid::nil(),
                "task_id": Uuid::nil(),
                "blocker_type": "environment_git_permission",
                "summary": "Git index is read-only",
                "resolution_condition": "Restore repository write access"
            }),
            "blocker_open",
        ),
        (
            json!({
                "action": "evidence_create",
                "company_id": Uuid::nil(),
                "project_id": Uuid::nil(),
                "evidence_type": "integration_report",
                "summary": "Integrated all accepted changes and verified the stable branch.",
                "result": "pass",
                "metadata": { "commit": "abc123" },
                "locator": "relay/integration@abc123"
            }),
            "evidence_create",
        ),
    ];

    for (mut value, action) in cases {
        normalize_company_task_input(&mut value);
        handler::parse_input::<CompanyTaskToolInput>(value)
            .unwrap_or_else(|error| panic!("normalized {action} should parse: {error}"));
    }
}

#[test]
fn gateway_normalizes_cached_task_execution_calls_before_business_validation() {
    let app = PlatformApp::new(MemoryPlatformRepository::default());
    let owner = app
        .dev_login(DevLoginInput {
            email: "mcp-task-normalization-owner@example.com".into(),
            display_name: "MCP Task Normalization Owner".into(),
        })
        .expect("owner should be created");
    let company = app
        .create_company(CreateCompanyInput {
            human_user_id: owner.id,
            name: "MCP Task Normalization Company".into(),
            slug: Some("mcp-task-normalization-company".into()),
            description: None,
        })
        .expect("company should be created");
    let manager = app
        .create_company_agent(CreateCompanyAgentInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            display_name: "MCP Task Normalization Manager".into(),
            handle: "mcp-task-normalization-manager".into(),
            persona: "负责任务规划和分配".into(),
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
            display_name: "MCP Task Normalization Engineer".into(),
            handle: "mcp-task-normalization-engineer".into(),
            persona: "负责执行和验证项目任务".into(),
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
            name: "MCP Task Normalization Project".into(),
            description: None,
            member_agent_ids: vec![engineer.agent_profile.id],
        })
        .expect("project should be created");
    let task = app
        .create_company_project_task(CreateCompanyProjectTaskInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            project_id: project.project.id,
            title: "验证旧会话工具输入兼容".into(),
            description: None,
            priority: Some("high".into()),
            assignee_agent_id: Some(engineer.agent_profile.id),
            due_at: None,
        })
        .expect("task should be created");
    let gateway = McpGateway::new(app, None);

    let started = gateway
        .invoke(
            Some(&engineer.agent_key_plaintext),
            "company.task",
            json!({
                "action": "attempt_start",
                "company_id": company.company.id,
                "project_id": project.project.id,
                "task_id": task.id,
                "attempt_type": "implementation",
                "idempotency_key": "normalize-attempt-start"
            }),
        )
        .expect("cached attempt_start vocabulary should succeed through the gateway");
    assert_eq!(started.output["attempt"]["attempt_type"], "execution");
    assert_eq!(
        started.output["attempt"]["objective"],
        "Execute the assigned task and satisfy its acceptance criteria."
    );
    let attempt_id = started.output["attempt"]["id"]
        .as_str()
        .and_then(|value| Uuid::parse_str(value).ok())
        .expect("attempt id should be returned");

    let blocker = gateway
        .invoke(
            Some(&engineer.agent_key_plaintext),
            "company.task",
            json!({
                "action": "blocker_open",
                "company_id": company.company.id,
                "project_id": project.project.id,
                "task_id": task.id,
                "attempt_id": attempt_id,
                "blocker_type": "environment_git_permission",
                "summary": "Git credentials require repair",
                "resolution_condition": "Restore repository write access",
                "idempotency_key": "normalize-blocker-open"
            }),
        )
        .expect("natural blocker subtype should succeed through the gateway");
    assert_eq!(blocker.output["blocker"]["blocker_type"], "environment");

    let evidence = gateway
        .invoke(
            Some(&engineer.agent_key_plaintext),
            "company.task",
            json!({
                "action": "evidence_create",
                "company_id": company.company.id,
                "project_id": project.project.id,
                "task_id": task.id,
                "attempt_id": attempt_id,
                "evidence_type": "integration_report",
                "summary": "第7-9章推进批次已验收并集成。",
                "result": "pass",
                "metadata": { "files": 3 },
                "locator": "relay/integration@abc123",
                "idempotency_key": "normalize-evidence-create"
            }),
        )
        .expect("cached evidence call should succeed through the gateway");
    assert_eq!(evidence.output["evidence"]["evidence_type"], "report");
    assert_eq!(evidence.output["evidence"]["result"], "passed");
    assert_eq!(
        evidence.output["evidence"]["title"],
        "第7-9章推进批次已验收并集成。"
    );
    assert_eq!(
        evidence.output["evidence"]["metrics"],
        json!({ "files": 3 })
    );
    assert_eq!(
        evidence.output["evidence"]["artifact_refs"],
        json!(["relay/integration@abc123"])
    );
    let replayed_evidence = gateway
        .invoke(
            Some(&engineer.agent_key_plaintext),
            "company.task",
            json!({
                "action": "evidence_create",
                "company_id": company.company.id,
                "project_id": project.project.id,
                "task_id": task.id,
                "attempt_id": attempt_id,
                "evidence_type": "integration_report",
                "summary": "第7-9章推进批次已验收并集成。",
                "result": "passed",
                "metadata": { "files": 3 },
                "locator": "relay/integration@abc123",
                "idempotency_key": "normalize-evidence-create"
            }),
        )
        .expect("semantic aliases should share the same idempotent request identity");
    assert_eq!(
        replayed_evidence.output["evidence"]["id"],
        evidence.output["evidence"]["id"]
    );

    let finished = gateway
        .invoke(
            Some(&engineer.agent_key_plaintext),
            "company.task",
            json!({
                "action": "attempt_finish",
                "company_id": company.company.id,
                "project_id": project.project.id,
                "task_id": task.id,
                "attempt_id": attempt_id,
                "status": "success",
                "result_summary": "Delivered and verified",
                "idempotency_key": "normalize-attempt-finish"
            }),
        )
        .expect("natural terminal status should succeed through the gateway");
    assert_eq!(finished.output["attempt"]["status"], "succeeded");
}

#[test]
fn evidence_normalization_supplies_cached_client_defaults() {
    let summary = "第7-9章推进批次已验收并集成到 relay/integration。";
    let mut value = json!({
        "action": "evidence_create",
        "company_id": Uuid::nil(),
        "project_id": Uuid::nil(),
        "evidence_type": "git_commit",
        "summary": summary,
        "metadata": { "commit": "abc123" },
        "locator": "relay/integration@abc123"
    });
    normalize_company_task_input(&mut value);

    assert_eq!(value["evidence_type"], "artifact");
    assert_eq!(value["result"], "informational");
    assert_eq!(value["title"], summary);
    assert_eq!(value["metrics"], json!({ "commit": "abc123" }));
    assert_eq!(value["artifact_refs"], json!(["relay/integration@abc123"]));
    assert!(value.get("metadata").is_none());
    assert!(value.get("locator").is_none());
    handler::parse_input::<CompanyTaskToolInput>(value)
        .expect("cached evidence input should parse after normalization");

    let mut encoded_metrics = json!({
        "action": "evidence_create",
        "company_id": Uuid::nil(),
        "project_id": Uuid::nil(),
        "evidence_type": "document",
        "title": "Review",
        "summary": "Review complete",
        "result": "pass",
        "metrics": "{\"files\":3}",
        "artifact_refs": null
    });
    normalize_company_task_input(&mut encoded_metrics);
    assert_eq!(encoded_metrics["metrics"], json!({ "files": 3 }));
    assert_eq!(encoded_metrics["artifact_refs"], json!([]));
    handler::parse_input::<CompanyTaskToolInput>(encoded_metrics)
        .expect("legacy encoded evidence metrics should normalize");
}

#[test]
fn task_contract_validation_reports_all_problems_in_one_response() {
    let mut value = json!({
        "action": "attempt_finish",
        "company_id": Uuid::nil(),
        "status": "mystery"
    });
    normalize_company_task_input(&mut value);
    let error = validate_company_task_input(&value).expect_err("input should be incomplete");
    let message = error.to_string();

    for field in ["project_id", "task_id", "attempt_id", "result_summary"] {
        assert!(
            message.contains(field),
            "error should identify missing {field}"
        );
    }
    assert!(message.contains("status=\"mystery\""));
    assert!(message.contains("succeeded, failed, cancelled, interrupted"));
    assert!(message.contains("do not add fields one at a time"));
}

#[test]
fn task_schema_spells_out_execution_action_contracts() {
    let tool = company_mcp_tools(&[
        COMPANY_PERMISSION_TASK_ASSIGN.into(),
        COMPANY_PERMISSION_TASK_UPDATE.into(),
    ])
    .into_iter()
    .find(|tool| tool.name.as_ref() == "company.task")
    .expect("company.task tool");
    let schema = Value::Object((*tool.input_schema).clone());

    for (action, required_fields) in [
        (
            "attempt_start",
            &[
                "company_id",
                "project_id",
                "task_id",
                "attempt_type",
                "objective",
            ][..],
        ),
        (
            "attempt_finish",
            &[
                "company_id",
                "project_id",
                "task_id",
                "attempt_id",
                "status",
                "result_summary",
            ][..],
        ),
        (
            "blocker_open",
            &[
                "company_id",
                "project_id",
                "task_id",
                "blocker_type",
                "summary",
                "resolution_condition",
            ][..],
        ),
        (
            "evidence_create",
            &[
                "company_id",
                "project_id",
                "evidence_type",
                "title",
                "summary",
                "result",
            ][..],
        ),
    ] {
        let required = schema_action_required_fields(&schema, action);
        for field in required_fields {
            assert!(
                required.contains(&field.to_string()),
                "{action} schema should require {field}; got {required:?}"
            );
        }
    }
    assert!(tool.description.as_deref().is_some_and(
        |description| description.contains("Canonical evidence result values are passed")
    ));
}

fn schema_action_required_fields(value: &Value, action: &str) -> Vec<String> {
    if schema_action_name(value).as_deref() == Some(action) {
        if let Some(required) = value.get("required").and_then(Value::as_array) {
            return required
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect();
        }
    }
    match value {
        Value::Object(object) => object
            .values()
            .find_map(|child| {
                let fields = schema_action_required_fields(child, action);
                (!fields.is_empty()).then_some(fields)
            })
            .unwrap_or_default(),
        Value::Array(items) => items
            .iter()
            .find_map(|item| {
                let fields = schema_action_required_fields(item, action);
                (!fields.is_empty()).then_some(fields)
            })
            .unwrap_or_default(),
        _ => Vec::new(),
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
