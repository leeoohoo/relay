use super::*;

pub fn standard_mcp_tools() -> Vec<Tool> {
    vec![
        read_only_tool::<EmptyInput>(
            "agent.bootstrap",
            "Start here. Return the authenticated Agent's identity, company, organization, coworkers, permissions, conversations, projects, pending inbox, and suggested next tools.",
        ),
        action_tool::<AgentProfileUpdateToolInput>(
            "agent.profile.update",
            "Update the authenticated Agent's structured responsibilities, skills, current focus, or collaboration preference so coworkers can discover what this Agent does.",
        ),
        action_tool::<AgentMemoryToolInput>(
            "agent.memory",
            "Maintain this Agent's isolated two-tier memory. long_term memories are injected into the Agent-specific Skill on every Codex wake-up; short_term memories are searched on demand. Available actions: overview, search, get, remember, update, archive, supersede, pin, and forget. Store concise reusable conclusions, never raw chat, task text, logs, or secrets.",
        ),
        action_tool::<AgentWorkSessionToolInput>(
            "agent.work_session",
            "List this Agent's control/project Codex sessions, inspect a session checkpoint, or dispatch structured work to a project-bound worker session. Use dispatch only when project execution is necessary; the Relay backend resolves the actual thread from agent_id plus project_id.",
        ),
        read_only_tool::<AgentInboxWaitInput>(
            "agent.inbox.wait",
            "List or wait up to 25 seconds for inbox events. Set timeout_seconds to 0 for an immediate query and pending_only to false for history.",
        ),
        mutating_tool::<AgentInboxProcessInput>(
            "agent.inbox.ack",
            "Mark one inbox event as processed after the agent has handled it.",
            true,
        ),
    ]
}

pub(super) fn company_mcp_tools(permissions: &[String]) -> Vec<Tool> {
    // Keep delegated project-content actions in the schema even before the
    // permission is granted. A Human can grant these permissions while a
    // Codex turn is already running, but Codex does not refresh the MCP tool
    // schema in the middle of that turn. Execution still rechecks the live
    // permission in PlatformApp, so visibility here does not grant access.
    let mut project_actions = vec![
        "get",
        "list",
        "status_update",
        "rule_update",
        "assets_replace",
    ];
    if permissions
        .iter()
        .any(|permission| permission == COMPANY_PERMISSION_PROJECT_CREATE)
    {
        project_actions.extend(["create", "git_provision"]);
    }
    let can_manage_projects = permissions
        .iter()
        .any(|permission| permission == COMPANY_PERMISSION_PROJECT_MANAGE);
    if can_manage_projects {
        project_actions.extend(["update", "member_add", "member_remove", "owner_transfer"]);
        if !project_actions.contains(&"git_provision") {
            project_actions.push("git_provision");
        }
    }
    let project_schema = tailored_action_schema::<CompanyProjectToolInput>(
        &project_actions,
        if can_manage_projects {
            &[]
        } else {
            &[("status_update", &["project_status"] as &[&str])]
        },
    );

    let can_assign_tasks = permissions
        .iter()
        .any(|permission| permission == COMPANY_PERMISSION_TASK_ASSIGN);
    let can_update_tasks = permissions
        .iter()
        .any(|permission| permission == COMPANY_PERMISSION_TASK_UPDATE);
    let mut task_actions = vec!["get", "list", "my"];
    if can_assign_tasks {
        task_actions.extend([
            "create",
            "update",
            "batch_update",
            "dependency_add",
            "dependency_remove",
        ]);
    } else if can_update_tasks {
        task_actions.push("update");
    }
    let task_schema = tailored_action_schema::<CompanyTaskToolInput>(
        &task_actions,
        if can_assign_tasks {
            &[]
        } else {
            &[(
                "update",
                &[
                    "title",
                    "description",
                    "priority",
                    "assignee_agent_id",
                    "due_at",
                ] as &[&str],
            )]
        },
    );
    let task_schema = if can_assign_tasks {
        task_schema
    } else {
        restrict_action_field_to_nullable_enum(
            task_schema,
            "update",
            "status",
            &["in_progress", "blocked", "done", "failed"],
            "Assigned Agents may only update their own task to in_progress, blocked, done, or failed.",
        )
    };

    let mut tools = vec![
        action_tool::<CompanyChatToolInput>(
            "company.chat",
            "Company messaging actions: direct_open, group_create, send, reply, history, unread, and mark_read. During an active Codex run, mark_read never acknowledges messages that arrived after the run started; inspect them with agent.inbox.wait and acknowledge each handled event with agent.inbox.ack.",
        ),
        action_tool_with_schema(
            "company.project",
            format!(
                "Project actions visible to this Agent: {}. rule_update and assets_replace remain visible so a Human can grant their permissions during an active turn; every call is authorized against the Agent's current live permissions.",
                project_actions.join(", ")
            ),
            project_schema,
        ),
        read_only_tool::<CompanyEventsToolInput>(
            "company.events",
            "Catch up on durable company realtime events after a sequence id; use the Agent SSE endpoint for push delivery.",
        ),
    ];
    tools.insert(
        2,
        action_tool_with_schema(
            "company.task",
            format!(
                "Project task actions available to this Agent: {}.",
                task_actions.join(", ")
            ),
            task_schema,
        ),
    );
    tools
}

pub(super) fn staffing_mcp_tools(permissions: &[String]) -> Vec<Tool> {
    let mut actions = vec!["action_get", "action_list"];
    for (permission, action) in [
        (COMPANY_PERMISSION_STAFF_HIRE, "hire"),
        (COMPANY_PERMISSION_STAFF_SUSPEND, "suspend"),
        (COMPANY_PERMISSION_STAFF_TERMINATE, "terminate"),
    ] {
        if permissions.iter().any(|value| value == permission) {
            actions.push(action);
        }
    }
    (actions.len() > 2)
        .then(|| {
            action_tool_with_schema(
                "company.staff",
                format!(
                    "Staffing actions available to this Agent: {}.",
                    actions.join(", ")
                ),
                tailored_action_schema::<CompanyStaffToolInput>(&actions, &[]),
            )
        })
        .into_iter()
        .collect()
}

pub(super) fn read_only_tool<T: JsonSchema + 'static>(
    name: &'static str,
    description: &'static str,
) -> Tool {
    Tool::new(name, description, schema_for::<T>()).with_annotations(
        ToolAnnotations::new()
            .read_only(true)
            .destructive(false)
            .idempotent(true)
            .open_world(false),
    )
}

pub(super) fn mutating_tool<T: JsonSchema + 'static>(
    name: &'static str,
    description: &'static str,
    idempotent: bool,
) -> Tool {
    let mut input_schema = (*schema_for::<T>()).clone();
    if let Some(Value::Object(properties)) = input_schema.get_mut("properties") {
        properties.insert(
            "idempotency_key".into(),
            json!({
                "type": "string",
                "maxLength": 160,
                "description": "Optional retry key. Reusing it with the same input replays the previous result for 24 hours."
            }),
        );
    }
    Tool::new(name, description, Arc::new(input_schema)).with_annotations(
        ToolAnnotations::new()
            .read_only(false)
            .destructive(false)
            .idempotent(idempotent)
            .open_world(false),
    )
}

pub(super) fn action_tool<T: JsonSchema + 'static>(
    name: &'static str,
    description: &'static str,
) -> Tool {
    Tool::new(name, description, schema_for::<T>()).with_annotations(
        ToolAnnotations::new()
            .read_only(false)
            .destructive(false)
            .idempotent(true)
            .open_world(false),
    )
}

pub(super) fn action_tool_with_schema(
    name: &'static str,
    description: String,
    input_schema: Arc<JsonObject>,
) -> Tool {
    Tool::new(name, description, input_schema).with_annotations(
        ToolAnnotations::new()
            .read_only(false)
            .destructive(false)
            .idempotent(true)
            .open_world(false),
    )
}

pub(super) fn tailored_action_schema<T: JsonSchema + 'static>(
    allowed_actions: &[&str],
    hidden_fields: &[(&str, &[&str])],
) -> Arc<JsonObject> {
    let mut schema = Value::Object((*schema_for::<T>()).clone());
    tailor_action_schema_value(&mut schema, allowed_actions, hidden_fields);
    match schema {
        Value::Object(object) => Arc::new(object),
        _ => unreachable!("MCP input schema root must be an object"),
    }
}

pub(super) fn tailor_action_schema_value(
    value: &mut Value,
    allowed_actions: &[&str],
    hidden_fields: &[(&str, &[&str])],
) {
    match value {
        Value::Object(object) => {
            if let Some(Value::Array(variants)) = object.get_mut("oneOf") {
                if variants
                    .iter()
                    .any(|variant| schema_action_name(variant).is_some())
                {
                    variants.retain(|variant| {
                        schema_action_name(variant)
                            .is_none_or(|action| allowed_actions.contains(&action.as_str()))
                    });
                }
            }
            for child in object.values_mut() {
                tailor_action_schema_value(child, allowed_actions, hidden_fields);
            }
            if let Some(action) = schema_action_name(value) {
                if let Some((_, fields)) = hidden_fields
                    .iter()
                    .find(|(candidate, _)| *candidate == action)
                {
                    hide_schema_fields(value, fields);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                tailor_action_schema_value(item, allowed_actions, hidden_fields);
            }
        }
        _ => {}
    }
}

pub(super) fn schema_action_name(value: &Value) -> Option<String> {
    if let Some(action_schema) = value
        .get("properties")
        .and_then(|properties| properties.get("action"))
    {
        if let Some(action) = action_schema.get("const").and_then(Value::as_str) {
            return Some(action.to_string());
        }
        if let Some(action) = action_schema
            .get("enum")
            .and_then(Value::as_array)
            .and_then(|values| values.first())
            .and_then(Value::as_str)
        {
            return Some(action.to_string());
        }
    }
    value
        .get("allOf")
        .or_else(|| value.get("anyOf"))
        .and_then(Value::as_array)
        .and_then(|items| items.iter().find_map(schema_action_name))
}

pub(super) fn hide_schema_fields(value: &mut Value, fields: &[&str]) {
    match value {
        Value::Object(object) => {
            if let Some(Value::Object(properties)) = object.get_mut("properties") {
                for field in fields {
                    properties.remove(*field);
                }
            }
            if let Some(Value::Array(required)) = object.get_mut("required") {
                required
                    .retain(|field| field.as_str().is_none_or(|field| !fields.contains(&field)));
            }
            for child in object.values_mut() {
                hide_schema_fields(child, fields);
            }
        }
        Value::Array(items) => {
            for item in items {
                hide_schema_fields(item, fields);
            }
        }
        _ => {}
    }
}

pub(super) fn restrict_action_field_to_nullable_enum(
    schema: Arc<JsonObject>,
    action: &str,
    field: &str,
    values: &[&str],
    description: &str,
) -> Arc<JsonObject> {
    let mut schema = Value::Object((*schema).clone());
    restrict_action_field_to_nullable_enum_value(&mut schema, action, field, values, description);
    match schema {
        Value::Object(object) => Arc::new(object),
        _ => unreachable!("MCP input schema root must be an object"),
    }
}

pub(super) fn restrict_action_field_to_nullable_enum_value(
    value: &mut Value,
    action: &str,
    field: &str,
    values: &[&str],
    description: &str,
) {
    if schema_action_name(value).as_deref() == Some(action) {
        if let Some(Value::Object(properties)) = value.get_mut("properties") {
            let mut allowed = values.iter().map(|value| json!(value)).collect::<Vec<_>>();
            allowed.push(Value::Null);
            properties.insert(
                field.to_string(),
                json!({
                    "enum": allowed,
                    "description": description
                }),
            );
        }
    }
    match value {
        Value::Object(object) => {
            for child in object.values_mut() {
                restrict_action_field_to_nullable_enum_value(
                    child,
                    action,
                    field,
                    values,
                    description,
                );
            }
        }
        Value::Array(items) => {
            for item in items {
                restrict_action_field_to_nullable_enum_value(
                    item,
                    action,
                    field,
                    values,
                    description,
                );
            }
        }
        _ => {}
    }
}

pub(super) fn schema_for<T: JsonSchema + 'static>() -> Arc<JsonObject> {
    rmcp::handler::server::tool::schema_for_input::<T>()
        .unwrap_or_else(|error| panic!("invalid MCP input schema: {error}"))
}

pub(super) fn is_public_tool_name(tool: &str) -> bool {
    matches!(
        tool,
        "agent.bootstrap"
            | "agent.profile.update"
            | "agent.memory"
            | "agent.work_session"
            | "agent.inbox.wait"
            | "agent.inbox.ack"
            | "company.chat"
            | "company.project"
            | "company.task"
            | "company.events"
            | "company.staff"
    )
}

pub(super) fn input_action(input: &Value) -> Option<&str> {
    input.get("action").and_then(Value::as_str)
}

pub(super) fn is_mutating_tool(tool: &str, input: &Value) -> bool {
    match tool {
        "agent.profile.update" | "agent.inbox.ack" => true,
        "agent.memory" => !matches!(input_action(input), Some("overview" | "search" | "get")),
        "agent.work_session" => matches!(input_action(input), Some("dispatch")),
        "company.chat" => !matches!(input_action(input), Some("history" | "unread")),
        "company.project" => !matches!(input_action(input), Some("get" | "list")),
        "company.task" => !matches!(input_action(input), Some("get" | "list" | "my")),
        "company.staff" => !matches!(input_action(input), Some("action_get" | "action_list")),
        _ => false,
    }
}

pub(super) fn audit_action_name(tool: &str, input: &Value) -> String {
    input_action(input)
        .map(|action| format!("{tool}.{action}"))
        .unwrap_or_else(|| tool.to_string())
}

pub(super) fn success_target_ref(tool: &str, input: &Value, output: &Value) -> Option<String> {
    match tool {
        "agent.profile.update" => nested_id(output, &["work_profile", "agent_profile", "id"])
            .map(|value| format!("agent:{value}")),
        "agent.inbox.ack" => {
            nested_id(output, &["event", "id"]).map(|value| format!("agent_inbox:{value}"))
        }
        "agent.memory" => nested_id(output, &["memory", "id"])
            .or_else(|| nested_id(input, &["memory_id"]))
            .map(|value| format!("agent_memory:{value}")),
        "agent.work_session" => nested_id(output, &["intent", "project_id"])
            .or_else(|| nested_id(input, &["project_id"]))
            .map(|value| format!("project:{value}")),
        "company.chat" => match input_action(input) {
            Some("direct_open" | "group_create") => {
                nested_id(output, &["conversation", "preview", "id"])
                    .map(|value| format!("conversation:{value}"))
            }
            Some("send" | "reply") => nested_id(output, &["message", "conversation_id"])
                .map(|value| format!("conversation:{value}")),
            Some("mark_read") => nested_id(output, &["result", "conversation_id"])
                .map(|value| format!("conversation:{value}")),
            _ => None,
        },
        "company.project" => match input_action(input) {
            Some("create" | "update" | "member_add" | "member_remove" | "owner_transfer") => {
                nested_id(output, &["project", "project", "id"])
                    .map(|value| format!("project:{value}"))
            }
            Some("status_update") => nested_id(output, &["status_update", "project_id"])
                .map(|value| format!("project:{value}")),
            Some("rule_update" | "assets_replace" | "git_provision") => {
                nested_id(input, &["project_id"]).map(|value| format!("project:{value}"))
            }
            _ => None,
        },
        "company.task" => match input_action(input) {
            Some("create" | "update") => {
                nested_id(output, &["task", "project_id"]).map(|value| format!("project:{value}"))
            }
            Some("batch_update") => output
                .get("tasks")
                .and_then(Value::as_array)
                .and_then(|tasks| tasks.first())
                .and_then(|task| nested_id(task, &["project_id"]))
                .map(|value| format!("project:{value}")),
            Some("dependency_add") => nested_id(output, &["dependency", "project_id"])
                .map(|value| format!("project:{value}")),
            Some("dependency_remove") => {
                nested_id(output, &["project_id"]).map(|value| format!("project:{value}"))
            }
            _ => None,
        },
        "company.staff" => nested_id(output, &["result", "agent_profile", "id"])
            .map(|value| format!("agent:{value}")),
        _ => None,
    }
}

pub(super) fn failure_target_ref(tool: &str, input: &Value) -> Option<String> {
    match tool {
        "agent.profile.update" => None,
        "agent.inbox.ack" => {
            nested_id(input, &["event_id"]).map(|value| format!("agent_inbox:{value}"))
        }
        "agent.memory" => nested_id(input, &["memory_id"])
            .map(|value| format!("agent_memory:{value}"))
            .or_else(|| nested_id(input, &["company_id"]).map(|value| format!("company:{value}"))),
        "agent.work_session" => nested_id(input, &["project_id"])
            .map(|value| format!("project:{value}"))
            .or_else(|| {
                nested_id(input, &["session_id"]).map(|value| format!("codex_session:{value}"))
            }),
        "company.chat" => match input_action(input) {
            Some("direct_open") => {
                nested_id(input, &["target_agent_id"]).map(|value| format!("agent:{value}"))
            }
            Some("group_create") => {
                nested_id(input, &["company_id"]).map(|value| format!("company:{value}"))
            }
            Some("send" | "mark_read") => {
                nested_id(input, &["conversation_id"]).map(|value| format!("conversation:{value}"))
            }
            Some("reply") => {
                nested_id(input, &["event_id"]).map(|value| format!("agent_inbox:{value}"))
            }
            _ => None,
        },
        "company.project" => match input_action(input) {
            Some("create") => {
                nested_id(input, &["company_id"]).map(|value| format!("company:{value}"))
            }
            _ => nested_id(input, &["project_id"]).map(|value| format!("project:{value}")),
        },
        "company.task" => nested_id(input, &["project_id"]).map(|value| format!("project:{value}")),
        "company.staff" => match input_action(input) {
            Some("hire") => {
                nested_id(input, &["company_id"]).map(|value| format!("company:{value}"))
            }
            Some("suspend" | "terminate") => {
                nested_id(input, &["target_agent_id"]).map(|value| format!("agent:{value}"))
            }
            _ => None,
        },
        _ => None,
    }
}

pub(super) fn nested_id(value: &Value, path: &[&str]) -> Option<String> {
    path.iter()
        .try_fold(value, |current, key| current.get(*key))
        .and_then(Value::as_str)
        .map(str::to_string)
}

pub(super) fn idempotency_key_from_input(input: &Value) -> Option<String> {
    input
        .get("idempotency_key")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(super) fn input_without_idempotency(mut input: Value) -> Value {
    if let Some(object) = input.as_object_mut() {
        object.remove("idempotency_key");
    }
    input
}

pub(super) fn action_status_for_error(error: &AppError) -> AgentActionStatus {
    match error {
        AppError::Conflict(_) | AppError::Unauthorized(_) | AppError::RateLimited(_) => {
            AgentActionStatus::Blocked
        }
        AppError::Validation(_) | AppError::NotFound(_) | AppError::Internal(_) => {
            AgentActionStatus::Failed
        }
    }
}
