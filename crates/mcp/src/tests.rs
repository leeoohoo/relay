use super::tools::*;
use super::*;
use ai_chat_application::{
    CreateCompanyAgentInput, CreateCompanyInput, DevLoginInput, MemoryPlatformRepository,
};
use ai_chat_domain::company::{
    COMPANY_AGENT_ROLE_MANAGER, COMPANY_AGENT_ROLE_MEMBER,
    COMPANY_PERMISSION_PROJECT_ASSETS_MANAGE, COMPANY_PERMISSION_PROJECT_RULES_MANAGE,
};

#[test]
fn standard_surface_has_five_identity_profile_memory_and_inbox_tools() {
    let tools = standard_mcp_tools();
    let names = tools
        .iter()
        .map(|tool| tool.name.as_ref())
        .collect::<Vec<_>>();

    assert_eq!(names.len(), 5);
    assert!(names.contains(&"agent.bootstrap"));
    assert!(names.contains(&"agent.profile.update"));
    assert!(names.contains(&"agent.memory"));
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
fn compact_surface_exposes_nine_tools_and_hides_legacy_names() {
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
    assert_eq!(names.len(), 9);
    assert!(names.contains(&"company.chat"));
    assert!(names.contains(&"company.project"));
    assert!(names.contains(&"company.task"));
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
fn active_company_agents_receive_four_company_domain_tools() {
    let tools = company_mcp_tools(&[]);
    let names = tools
        .iter()
        .map(|tool| tool.name.as_ref())
        .collect::<Vec<_>>();
    assert_eq!(names.len(), 4);
    assert!(names.contains(&"company.chat"));
    assert!(names.contains(&"company.project"));
    assert!(names.contains(&"company.task"));
    assert!(names.contains(&"company.events"));
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
    );
    let serialized = serde_json::to_value(result).expect("tool result should serialize");
    assert_eq!(
        serialized["structuredContent"]["inbox_notice"]["attention_required"],
        true
    );

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
        ["get", "list", "my", "update"]
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
        &vec![
            json!("in_progress"),
            json!("blocked"),
            json!("done"),
            json!("failed"),
            Value::Null,
        ]
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
