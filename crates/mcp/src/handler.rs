use super::tools::*;
use super::*;

pub(super) fn parse_input<T: for<'de> Deserialize<'de>>(input: Value) -> AppResult<T> {
    validate_uuid_shapes(&input, "")?;
    let encoded = serde_json::to_vec(&input)
        .map_err(|error| AppError::Validation(format!("input serialization failed: {error}")))?;
    let mut deserializer = serde_json::Deserializer::from_slice(&encoded);
    serde_path_to_error::deserialize(&mut deserializer).map_err(|error| {
        let path = error.path().to_string();
        let message = error.inner().to_string();
        AppError::Validation(if path.is_empty() || path == "." {
            message
        } else {
            format!("invalid field {path}: {message}")
        })
    })
}

fn validate_uuid_shapes(value: &Value, path: &str) -> AppResult<()> {
    match value {
        Value::Object(fields) => {
            for (field, child) in fields {
                let child_path = if path.is_empty() {
                    field.clone()
                } else {
                    format!("{path}.{field}")
                };
                if is_uuid_field(field) {
                    if let Some(identifier) = child.as_str() {
                        if Uuid::parse_str(identifier).is_err() {
                            return Err(AppError::Validation(format!(
                                "invalid field {child_path}: expected a full UUID, received {identifier:?}. Do not use a list position, shortened UUID, Git commit, or another object's ID"
                            )));
                        }
                    }
                } else if is_uuid_list_field(field) {
                    if let Some(items) = child.as_array() {
                        for (index, identifier) in items.iter().enumerate() {
                            if let Some(identifier) = identifier.as_str() {
                                if Uuid::parse_str(identifier).is_err() {
                                    return Err(AppError::Validation(format!(
                                        "invalid field {child_path}[{index}]: expected a full UUID, received {identifier:?}. Do not use a list position, shortened UUID, or Git commit"
                                    )));
                                }
                            }
                        }
                    }
                }
                validate_uuid_shapes(child, &child_path)?;
            }
        }
        Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                validate_uuid_shapes(child, &format!("{path}[{index}]"))?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn is_uuid_field(field: &str) -> bool {
    matches!(
        field,
        "action_id"
            | "after_message_id"
            | "assignee_agent_id"
            | "attempt_id"
            | "before_message_id"
            | "blocker_id"
            | "company_id"
            | "conversation_id"
            | "depends_on_task_id"
            | "environment_id"
            | "event_id"
            | "gate_id"
            | "handoff_agent_id"
            | "intent_id"
            | "memory_id"
            | "org_unit_id"
            | "owner_agent_id"
            | "project_id"
            | "related_task_id"
            | "relation_id"
            | "reports_to_membership_id"
            | "reviewed_through_message_id"
            | "session_id"
            | "source_task_id"
            | "supersedes_memory_id"
            | "target_agent_id"
            | "target_task_id"
            | "task_id"
    )
}

fn is_uuid_list_field(field: &str) -> bool {
    matches!(
        field,
        "member_agent_ids" | "mentioned_agent_ids" | "source_event_ids" | "task_ids"
    )
}

#[derive(Clone)]
pub struct AiChatMcpHandler<R: PlatformRepository, V: OwnershipProofVerifier> {
    gateway: McpGateway<R, V>,
}

impl<R: PlatformRepository, V: OwnershipProofVerifier> AiChatMcpHandler<R, V> {
    pub fn new(gateway: McpGateway<R, V>) -> Self {
        Self { gateway }
    }

    pub(super) fn pending_message_notice(&self, agent_id: Uuid) -> Option<Value> {
        let now = Utc::now();
        let mut messages = self
            .gateway
            .platform
            .list_agent_inbox_events(agent_id, true, 10_000)
            .ok()?
            .into_iter()
            .filter(|event| {
                event.requires_action
                    && event.event_type == "message.received"
                    && event.available_at <= now
            })
            .collect::<Vec<_>>();
        if messages.is_empty() {
            return None;
        }

        messages.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        let pending_message_count = messages.len();
        let latest_messages = messages
            .iter()
            .take(3)
            .map(|event| {
                let payload = &event.payload_json;
                json!({
                    "event_id": event.id,
                    "conversation_id": payload.get("conversation_id"),
                    "sender_agent_id": payload.get("sender_agent_id"),
                    "sender_human_user_id": payload.get("sender_human_user_id"),
                    "mentioned": payload.get("mentioned").and_then(Value::as_bool).unwrap_or(false),
                    "content_preview": payload
                        .get("content")
                        .and_then(Value::as_str)
                        .map(|content| truncate_text(content, 240))
                        .unwrap_or_default(),
                    "created_at": event.created_at,
                })
            })
            .collect::<Vec<_>>();

        Some(json!({
            "attention_required": true,
            "kind": "pending_messages",
            "pending_message_count": pending_message_count,
            "latest_messages": latest_messages,
            "instruction": "你有新的待处理消息。请在继续其他工作前调用 agent.inbox.wait 获取完整事件，处理后调用 agent.inbox.ack；不要仅忽略此提示。"
        }))
    }

    pub(super) fn structured_success(
        &self,
        tool: impl Into<String>,
        agent_id: Uuid,
        output: Value,
        include_inbox_notice: bool,
    ) -> CallToolResult {
        let mut payload = json!({
            "tool": tool.into(),
            "agent_id": agent_id,
            "output": output,
        });
        if include_inbox_notice {
            if let Some(notice) = self.pending_message_notice(agent_id) {
                payload
                    .as_object_mut()
                    .expect("MCP success payload should be an object")
                    .insert("inbox_notice".into(), notice);
            }
        }
        CallToolResult::structured(payload)
    }

    fn structured_error_with_notice(
        &self,
        error: AppError,
        agent_id: Option<Uuid>,
        include_inbox_notice: bool,
    ) -> CallToolResult {
        let mut payload = json!({
            "code": error.code(),
            "message": error.to_string(),
        });
        if include_inbox_notice {
            if let Some(notice) =
                agent_id.and_then(|agent_id| self.pending_message_notice(agent_id))
            {
                payload
                    .as_object_mut()
                    .expect("MCP error payload should be an object")
                    .insert("inbox_notice".into(), notice);
            }
        }
        CallToolResult::structured_error(payload)
    }

    async fn wait_for_inbox(
        &self,
        agent_key: Option<&str>,
        agent_run_token: Option<&str>,
        input: Value,
    ) -> CallToolResult {
        let agent = match self.gateway.authenticate_agent(agent_key, agent_run_token) {
            Ok(agent) => agent,
            Err(error) => return structured_tool_error(error),
        };
        let input: AgentInboxWaitInput = match parse_input(input) {
            Ok(input) => input,
            Err(error) => {
                return self.structured_error_with_notice(error, Some(agent.id), true);
            }
        };
        let timeout_seconds = input.timeout_seconds.unwrap_or(20).min(25);
        let deadline = tokio::time::Instant::now() + Duration::from_secs(timeout_seconds);
        let limit = input.limit.unwrap_or(20).clamp(1, 100);

        loop {
            let events = match self.gateway.platform.list_agent_inbox_events(
                agent.id,
                input.pending_only.unwrap_or(true),
                limit,
            ) {
                Ok(events) => filter_inbox_events(events, input.event_types.as_deref()),
                Err(error) => {
                    return self.structured_error_with_notice(error, Some(agent.id), true);
                }
            };
            let timed_out = events.is_empty() && tokio::time::Instant::now() >= deadline;
            if !events.is_empty() || timed_out {
                return self.structured_success(
                    "agent.inbox.wait",
                    agent.id,
                    json!({
                        "events": events,
                        "timed_out": timed_out,
                        "timeout_seconds": timeout_seconds
                    }),
                    true,
                );
            }
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            tokio::time::sleep(remaining.min(Duration::from_millis(250))).await;
        }
    }
}

impl<R: PlatformRepository, V: OwnershipProofVerifier> ServerHandler for AiChatMcpHandler<R, V> {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(
                Implementation::new("ai-chat", env!("CARGO_PKG_VERSION"))
                    .with_title("Agent Company Network")
                    .with_description(
                        "Company identity, private Agent memory, coworker directory, inbox, messaging, projects, and staffing tools for external AI agents.",
                    ),
            )
            .with_instructions(
                "Authenticate every request with x-agent-key, Authorization: Bearer <Agent Key>, or a short-lived x-agent-run-token issued to the local Codex Trigger. The credential already fixes the Agent identity; do not ask a Human to reconfirm it. Trigger-managed control sessions receive a one-shot Control Snapshot and must not repeat bootstrap/task-my/inbox-wait polling; refresh once with agent.control_snapshot only after a stale-state conflict. Project worker sessions read their bound project and tasks directly and never triage Inbox. Each Agent owns an isolated memory set. Store only distilled reusable conclusions, never raw chat, task text, logs, or secrets. Relay tool responses include inbox_notice only for actionable messages. Agents with explicit Human-granted Staffing permissions receive the company.staff tool dynamically.",
            )
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        let agent_key = agent_key_from_context(&context);
        let agent_run_token = agent_run_token_from_context(&context);
        let project_worker_session = is_project_worker_session(&context);
        let agent = self
            .gateway
            .authenticate_agent(agent_key.as_deref(), agent_run_token.as_deref())
            .map_err(mcp_request_error)?;
        let mut tools = standard_mcp_tools();
        if project_worker_session {
            tools.retain(|tool| {
                !matches!(
                    tool.name.as_ref(),
                    "agent.control_snapshot" | "agent.inbox.wait" | "agent.inbox.ack"
                )
            });
        }
        if let Ok(membership) = self
            .gateway
            .platform
            .get_active_company_agent_membership(agent.id)
        {
            tools.extend(company_mcp_tools(&membership.permissions));
            tools.extend(staffing_mcp_tools(&membership.permissions));
        }
        Ok(ListToolsResult::with_all_items(tools))
    }

    fn get_tool(&self, name: &str) -> Option<Tool> {
        let mut tools = standard_mcp_tools();
        let all_permissions = [
            COMPANY_PERMISSION_PROJECT_CREATE.into(),
            COMPANY_PERMISSION_PROJECT_MANAGE.into(),
            COMPANY_PERMISSION_TASK_ASSIGN.into(),
            COMPANY_PERMISSION_TASK_UPDATE.into(),
            COMPANY_PERMISSION_STAFF_HIRE.into(),
            COMPANY_PERMISSION_STAFF_SUSPEND.into(),
            COMPANY_PERMISSION_STAFF_TERMINATE.into(),
        ];
        tools.extend(company_mcp_tools(&all_permissions));
        tools.extend(staffing_mcp_tools(&all_permissions));
        tools.into_iter().find(|tool| tool.name.as_ref() == name)
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        if self.get_tool(request.name.as_ref()).is_none() {
            return Err(ErrorData::new(
                ErrorCode::METHOD_NOT_FOUND,
                format!("unknown tool: {}", request.name),
                None,
            ));
        }

        let agent_key = agent_key_from_context(&context);
        let agent_run_token = agent_run_token_from_context(&context);
        let project_worker_session = is_project_worker_session(&context);
        let mut input = Value::Object(request.arguments.unwrap_or_default());
        if is_mutating_tool(request.name.as_ref(), &input) && input.get("idempotency_key").is_none()
        {
            if let Some(key) = idempotency_key_from_context(&context) {
                if let Some(object) = input.as_object_mut() {
                    object.insert("idempotency_key".into(), Value::String(key));
                }
            }
        }

        if project_worker_session
            && matches!(
                request.name.as_ref(),
                "agent.inbox.wait" | "agent.inbox.ack"
            )
        {
            return Ok(structured_tool_error(AppError::Conflict(
                "Inbox tools belong to the Agent control session and are unavailable in a project worker session"
                    .into(),
            )));
        }

        if request.name.as_ref() == "agent.inbox.wait" {
            return Ok(self
                .wait_for_inbox(agent_key.as_deref(), agent_run_token.as_deref(), input)
                .await);
        }

        match self.gateway.invoke_with_credentials(
            agent_key.as_deref(),
            agent_run_token.as_deref(),
            request.name.as_ref(),
            input,
        ) {
            Ok(invocation) => Ok(self.structured_success(
                invocation.tool,
                invocation.agent_id,
                invocation.output,
                !project_worker_session,
            )),
            Err(error) => {
                let agent_id = self
                    .gateway
                    .authenticate_agent(agent_key.as_deref(), agent_run_token.as_deref())
                    .ok()
                    .map(|agent| agent.id);
                Ok(self.structured_error_with_notice(error, agent_id, !project_worker_session))
            }
        }
    }
}

fn structured_tool_error(error: AppError) -> CallToolResult {
    CallToolResult::structured_error(json!({
        "code": error.code(),
        "message": error.to_string(),
    }))
}

fn mcp_request_error(error: AppError) -> ErrorData {
    ErrorData::new(
        ErrorCode::INVALID_REQUEST,
        error.to_string(),
        Some(json!({ "code": error.code() })),
    )
}

pub(super) fn filter_inbox_events(
    events: Vec<ai_chat_domain::agent_identity::AgentInboxEvent>,
    event_types: Option<&[String]>,
) -> Vec<ai_chat_domain::agent_identity::AgentInboxEvent> {
    events
        .into_iter()
        .filter(|event| event.requires_action)
        .filter(|event| {
            event_types
                .filter(|items| !items.is_empty())
                .is_none_or(|types| types.iter().any(|item| item == &event.event_type))
        })
        .collect()
}

fn truncate_text(value: &str, max_characters: usize) -> String {
    let mut characters = value.chars();
    let truncated = characters.by_ref().take(max_characters).collect::<String>();
    if characters.next().is_some() {
        format!("{truncated}…")
    } else {
        truncated
    }
}

fn agent_key_from_context(context: &RequestContext<RoleServer>) -> Option<String> {
    let parts = context.extensions.get::<Parts>()?;
    agent_key_from_headers(&parts.headers)
}

fn agent_run_token_from_context(context: &RequestContext<RoleServer>) -> Option<String> {
    let parts = context.extensions.get::<Parts>()?;
    agent_run_token_from_headers(&parts.headers)
}

fn is_project_worker_session(context: &RequestContext<RoleServer>) -> bool {
    let Some(parts) = context.extensions.get::<Parts>() else {
        return false;
    };
    agent_run_token_from_headers(&parts.headers).is_some()
        && relay_session_kind_from_headers(&parts.headers)
            .is_some_and(|value| value == AGENT_CODEX_SESSION_KIND_PROJECT)
}

fn idempotency_key_from_context(context: &RequestContext<RoleServer>) -> Option<String> {
    let parts = context.extensions.get::<Parts>()?;
    parts
        .headers
        .get("idempotency-key")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub fn agent_key_from_headers(headers: &HeaderMap) -> Option<String> {
    if let Some(value) = headers
        .get("x-agent-key")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Some(value.to_string());
    }

    headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().split_once(' '))
        .filter(|(scheme, _)| scheme.eq_ignore_ascii_case("bearer"))
        .map(|(_, token)| token)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub fn agent_run_token_from_headers(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-agent-run-token")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(super) fn relay_session_kind_from_headers(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-relay-session-kind")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}
