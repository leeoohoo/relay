use super::*;

pub(super) async fn drive_app_server<W, R>(
    writer: &mut W,
    reader: R,
    request: &CodexRunRequest,
    resume_thread_id: Option<&str>,
    sandbox_mode: &str,
    approval_handler: &dyn CodexApprovalHandler,
) -> AppResult<ProcessOutcome>
where
    W: AsyncWrite + Unpin,
    R: AsyncRead + Unpin,
{
    let mut reader = BufReader::new(reader);
    send_json_rpc(
        writer,
        &json!({
            "method": "initialize",
            "id": 0,
            "params": {
                "clientInfo": {
                    "name": "relay_agent_trigger",
                    "title": "Relay Agent Trigger",
                    "version": env!("CARGO_PKG_VERSION")
                },
                "capabilities": { "experimentalApi": true }
            }
        }),
    )
    .await?;
    let initialize = wait_for_rpc_response(&mut reader, 0).await?;
    if let Some(error) = rpc_error_message(&initialize) {
        return Err(AppError::Validation(format!(
            "Codex app-server initialization failed: {error}"
        )));
    }
    send_json_rpc(writer, &json!({ "method": "initialized", "params": {} })).await?;

    let mut thread_params = json!({
        "cwd": request.cwd.to_string_lossy(),
        "sandbox": sandbox_mode,
        "approvalPolicy": request.approval_policy,
        "approvalsReviewer": "user"
    });
    if let Some(model) = request.model.as_deref() {
        thread_params["model"] = Value::String(model.to_string());
    }
    let thread_method = if let Some(thread_id) = resume_thread_id {
        thread_params["threadId"] = Value::String(thread_id.to_string());
        "thread/resume"
    } else {
        "thread/start"
    };
    send_json_rpc(
        writer,
        &json!({ "method": thread_method, "id": 1, "params": thread_params }),
    )
    .await?;
    let thread_response = wait_for_rpc_response(&mut reader, 1).await?;
    if let Some(error) = rpc_error_message(&thread_response) {
        return Ok(ProcessOutcome {
            status: CodexRunStatus::Failed,
            thread_id: resume_thread_id.map(str::to_string),
            exit_code: Some(1),
            final_message: None,
            error_message: Some(truncate(&sanitize_error(&error), 2_000)),
            turn_started: false,
        });
    }
    let thread_id = thread_response
        .pointer("/result/thread/id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| resume_thread_id.map(str::to_string))
        .ok_or_else(|| AppError::Validation("Codex app-server returned no thread ID".into()))?;
    report_progress(
        request.progress_handler.as_ref(),
        "session",
        "Codex 固定会话已连接",
        Some(&thread_id),
    );

    let mut turn_params = json!({
        "threadId": thread_id,
        "input": [{ "type": "text", "text": request.prompt }],
        "cwd": request.cwd.to_string_lossy(),
        "approvalPolicy": request.approval_policy,
        "approvalsReviewer": "user"
    });
    if let Some(model) = request.model.as_deref() {
        turn_params["model"] = Value::String(model.to_string());
    }
    if let Some(reasoning_effort) = request.reasoning_effort.as_deref() {
        turn_params["effort"] = Value::String(reasoning_effort.to_string());
    }
    send_json_rpc(
        writer,
        &json!({ "method": "turn/start", "id": 2, "params": turn_params }),
    )
    .await?;

    let mut turn_started = false;
    let mut final_message = None;
    let mut error_message = None;
    let mut active_mcp_items = HashMap::<String, Value>::new();
    while let Some(value) = next_json_rpc(&mut reader).await? {
        if json_rpc_id_matches(&value, 2) {
            if let Some(error) = rpc_error_message(&value) {
                return Ok(ProcessOutcome {
                    status: CodexRunStatus::Failed,
                    thread_id: Some(thread_id.clone()),
                    exit_code: Some(1),
                    final_message,
                    error_message: Some(truncate(&sanitize_error(&error), 2_000)),
                    turn_started,
                });
            }
            turn_started = true;
            continue;
        }
        if value.get("id").is_some() && value.get("method").is_some() {
            let item = value
                .pointer("/params/itemId")
                .and_then(Value::as_str)
                .and_then(|item_id| active_mcp_items.get(item_id));
            handle_app_server_request(writer, &value, approval_handler, item).await?;
            continue;
        }
        match value.get("method").and_then(Value::as_str) {
            Some("turn/started") => {
                turn_started = true;
                report_progress(
                    request.progress_handler.as_ref(),
                    "thinking",
                    "Codex 已开始分析待办",
                    Some(&thread_id),
                );
            }
            Some("item/started") => {
                if let Some(item) = value.pointer("/params/item") {
                    if item.get("type").and_then(Value::as_str) == Some("mcpToolCall") {
                        if let Some(item_id) = item.get("id").and_then(Value::as_str) {
                            active_mcp_items.insert(item_id.to_string(), item.clone());
                        }
                    }
                    if let Some((phase, summary)) = summarize_codex_item(item, false) {
                        report_progress(
                            request.progress_handler.as_ref(),
                            &phase,
                            &summary,
                            Some(&thread_id),
                        );
                    }
                }
            }
            Some("item/completed") => {
                if let Some(item) = value.pointer("/params/item") {
                    if let Some(item_id) = item.get("id").and_then(Value::as_str) {
                        active_mcp_items.remove(item_id);
                    }
                    if let Some(message) = extract_agent_message(item) {
                        final_message = Some(message);
                    }
                    if let Some((phase, summary)) = summarize_codex_item(item, true) {
                        report_progress(
                            request.progress_handler.as_ref(),
                            &phase,
                            &summary,
                            Some(&thread_id),
                        );
                    }
                }
            }
            Some("turn/completed") => {
                let turn = value.pointer("/params/turn");
                if final_message.is_none() {
                    final_message = turn.and_then(extract_final_turn_message);
                }
                let status = turn
                    .and_then(|turn| turn.get("status"))
                    .and_then(Value::as_str)
                    .unwrap_or("failed");
                let turn_error = turn
                    .and_then(|turn| turn.pointer("/error/message"))
                    .and_then(Value::as_str)
                    .map(str::to_string);
                let succeeded = status == "completed";
                return Ok(ProcessOutcome {
                    status: if succeeded {
                        CodexRunStatus::Succeeded
                    } else {
                        CodexRunStatus::Failed
                    },
                    thread_id: Some(thread_id),
                    exit_code: Some(if succeeded { 0 } else { 1 }),
                    final_message,
                    error_message: if succeeded {
                        None
                    } else {
                        turn_error
                            .or(error_message)
                            .or_else(|| Some(format!("Codex turn completed with status {status}")))
                    },
                    turn_started: true,
                });
            }
            Some("error") => {
                error_message = value
                    .pointer("/params/message")
                    .or_else(|| value.pointer("/params/error/message"))
                    .and_then(Value::as_str)
                    .map(|message| truncate(&sanitize_error(message), 2_000));
            }
            _ => {}
        }
    }
    if let Some(reasoning_summary) = request.reasoning_summary.as_deref() {
        if !matches!(reasoning_summary, "auto" | "concise" | "detailed" | "none") {
            return Err(AppError::Validation(
                "Codex reasoning summary must be auto, concise, detailed, or none".into(),
            ));
        }
    }
    if let Some(verbosity) = request.verbosity.as_deref() {
        if !matches!(verbosity, "low" | "medium" | "high") {
            return Err(AppError::Validation(
                "Codex verbosity must be low, medium, or high".into(),
            ));
        }
    }
    if let Some(personality) = request.personality.as_deref() {
        if !matches!(personality, "none" | "friendly" | "pragmatic") {
            return Err(AppError::Validation(
                "Codex personality must be none, friendly, or pragmatic".into(),
            ));
        }
    }
    if request
        .service_tier
        .as_deref()
        .is_some_and(|value| value != "fast")
    {
        return Err(AppError::Validation(
            "Codex service tier must be fast when configured".into(),
        ));
    }
    if !matches!(
        request.web_search.as_str(),
        "disabled" | "cached" | "indexed" | "live"
    ) {
        return Err(AppError::Validation(
            "Codex web search must be disabled, cached, indexed, or live".into(),
        ));
    }

    Ok(ProcessOutcome {
        status: CodexRunStatus::Failed,
        thread_id: Some(thread_id),
        exit_code: Some(1),
        final_message,
        error_message: error_message
            .or_else(|| Some("Codex app-server closed before turn/completed".into())),
        turn_started,
    })
}

pub(super) async fn handle_app_server_request<W>(
    writer: &mut W,
    value: &Value,
    approval_handler: &dyn CodexApprovalHandler,
    active_item: Option<&Value>,
) -> AppResult<()>
where
    W: AsyncWrite + Unpin,
{
    let rpc_id = value
        .get("id")
        .cloned()
        .ok_or_else(|| AppError::Validation("Codex server request had no ID".into()))?;
    let method = value
        .get("method")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::Validation("Codex server request had no method".into()))?;
    let params = value.get("params").cloned().unwrap_or_else(|| json!({}));
    if method == "item/tool/requestUserInput" {
        return handle_browser_tool_approval(
            writer,
            rpc_id,
            &params,
            active_item,
            approval_handler,
        )
        .await;
    }
    let (tool_name, default_reason) = match method {
        "item/commandExecution/requestApproval" | "execCommandApproval" => {
            (AGENT_CODEX_APPROVAL_TOOL_COMMAND, "Codex 请求执行受限命令")
        }
        "item/fileChange/requestApproval" | "applyPatchApproval" => (
            AGENT_CODEX_APPROVAL_TOOL_FILE_CHANGE,
            "Codex 请求修改受保护文件",
        ),
        "item/permissions/requestApproval" => (
            AGENT_CODEX_APPROVAL_TOOL_PERMISSIONS,
            "Codex 请求额外文件系统或网络权限",
        ),
        _ => {
            return send_json_rpc(
                writer,
                &json!({
                    "id": rpc_id,
                    "error": { "code": -32601, "message": format!("Unsupported server request: {method}") }
                }),
            )
            .await;
        }
    };
    let reason = params
        .get("reason")
        .and_then(Value::as_str)
        .filter(|reason| !reason.trim().is_empty())
        .unwrap_or(default_reason)
        .to_string();
    let approval = approval_handler
        .request_approval(CodexApprovalRequest {
            tool_name: tool_name.into(),
            risk_level: "high".into(),
            reason,
            arguments: json!({ "method": method, "params": params.clone() }),
        })
        .await;
    let decision = match approval {
        Ok(decision) => decision,
        Err(error) => {
            let _ = send_approval_response(
                writer,
                rpc_id.clone(),
                method,
                &params,
                CodexApprovalDecision::Decline,
            )
            .await;
            return Err(error);
        }
    };
    send_approval_response(writer, rpc_id, method, &params, decision).await
}

async fn handle_browser_tool_approval<W>(
    writer: &mut W,
    rpc_id: Value,
    params: &Value,
    active_item: Option<&Value>,
    approval_handler: &dyn CodexApprovalHandler,
) -> AppResult<()>
where
    W: AsyncWrite + Unpin,
{
    let Some(item) = active_item.filter(|item| is_managed_browser_approval_item(item)) else {
        return send_json_rpc(
            writer,
            &json!({
                "id": rpc_id,
                "error": { "code": -32601, "message": "Unsupported interactive tool request" }
            }),
        )
        .await;
    };
    let tool = item
        .get("tool")
        .and_then(Value::as_str)
        .unwrap_or("browser");
    let input = normalized_mcp_arguments(item.get("arguments"));
    let url = input.get("url").and_then(Value::as_str).map(str::to_string);
    let reason = match (tool, url.as_deref()) {
        ("navigate_page" | "new_page", Some(url)) => {
            format!("Agent 请求通过浏览器访问 {url}")
        }
        ("upload_file", _) => "Agent 请求向当前网站上传本地文件".into(),
        _ => "Agent 请求使用受审批保护的浏览器操作".into(),
    };
    let approval = approval_handler
        .request_approval(CodexApprovalRequest {
            tool_name: AGENT_CODEX_APPROVAL_TOOL_WEBSITE_ACCESS.into(),
            risk_level: if tool == "upload_file" {
                "high".into()
            } else {
                "medium".into()
            },
            reason,
            arguments: json!({
                "server": item.get("server").cloned().unwrap_or(Value::Null),
                "tool": tool,
                "url": url,
                "input": input,
                "prompt": params
            }),
        })
        .await;
    let decision = match approval {
        Ok(decision) => decision,
        Err(error) => {
            let _ = send_approval_response(
                writer,
                rpc_id.clone(),
                "item/tool/requestUserInput",
                params,
                CodexApprovalDecision::Decline,
            )
            .await;
            return Err(error);
        }
    };
    send_approval_response(
        writer,
        rpc_id,
        "item/tool/requestUserInput",
        params,
        decision,
    )
    .await
}

fn is_managed_browser_approval_item(item: &Value) -> bool {
    item.get("type").and_then(Value::as_str) == Some("mcpToolCall")
        && item.get("server").and_then(Value::as_str) == Some(MANAGED_BROWSER_MCP_NAME)
        && matches!(
            item.get("tool").and_then(Value::as_str),
            Some("navigate_page" | "new_page" | "upload_file")
        )
}

fn normalized_mcp_arguments(arguments: Option<&Value>) -> Value {
    match arguments {
        Some(Value::String(arguments)) => {
            serde_json::from_str(arguments).unwrap_or_else(|_| json!({ "raw": arguments }))
        }
        Some(arguments) => arguments.clone(),
        None => json!({}),
    }
}

pub(super) async fn send_approval_response<W>(
    writer: &mut W,
    rpc_id: Value,
    method: &str,
    params: &Value,
    decision: CodexApprovalDecision,
) -> AppResult<()>
where
    W: AsyncWrite + Unpin,
{
    let result = match method {
        "item/commandExecution/requestApproval" | "item/fileChange/requestApproval" => {
            json!({ "decision": if decision == CodexApprovalDecision::Accept { "accept" } else { "decline" } })
        }
        "item/permissions/requestApproval" => {
            let permissions = if decision == CodexApprovalDecision::Accept {
                params
                    .get("permissions")
                    .cloned()
                    .unwrap_or_else(|| json!({}))
            } else {
                json!({})
            };
            json!({ "permissions": permissions, "scope": "turn" })
        }
        "item/tool/requestUserInput" => tool_user_input_response(params, decision),
        "execCommandApproval" | "applyPatchApproval" => {
            if decision == CodexApprovalDecision::Accept {
                json!({ "decision": "approved" })
            } else {
                json!({ "decision": { "denied": { "rejection": "Rejected by Human" } } })
            }
        }
        _ => json!({}),
    };
    send_json_rpc(writer, &json!({ "id": rpc_id, "result": result })).await
}

fn tool_user_input_response(params: &Value, decision: CodexApprovalDecision) -> Value {
    let mut answers = serde_json::Map::new();
    for question in params
        .get("questions")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(id) = question.get("id").and_then(Value::as_str) else {
            continue;
        };
        let labels = question
            .get("options")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|option| option.get("label").and_then(Value::as_str))
            .collect::<Vec<_>>();
        let selected = select_approval_option(&labels, decision);
        answers.insert(id.to_string(), json!({ "answers": [selected] }));
    }
    json!({ "answers": answers })
}

fn select_approval_option(labels: &[&str], decision: CodexApprovalDecision) -> String {
    let preferred = if decision == CodexApprovalDecision::Accept {
        [
            "accept", "allow", "approve", "continue", "yes", "允许", "批准", "继续",
        ]
        .as_slice()
    } else {
        ["decline", "deny", "reject", "no", "cancel", "拒绝", "取消"].as_slice()
    };
    preferred
        .iter()
        .find_map(|preferred| {
            labels
                .iter()
                .find(|label| label.trim().eq_ignore_ascii_case(preferred))
        })
        .copied()
        .or_else(|| {
            if decision == CodexApprovalDecision::Accept {
                labels.first().copied()
            } else {
                labels.last().copied()
            }
        })
        .unwrap_or(if decision == CodexApprovalDecision::Accept {
            "Accept"
        } else {
            "Decline"
        })
        .to_string()
}

pub(super) async fn send_json_rpc<W>(writer: &mut W, value: &Value) -> AppResult<()>
where
    W: AsyncWrite + Unpin,
{
    let mut line = serde_json::to_vec(value).map_err(|error| {
        AppError::Validation(format!("failed to encode Codex JSON-RPC message: {error}"))
    })?;
    line.push(b'\n');
    writer.write_all(&line).await.map_err(process_error)?;
    writer.flush().await.map_err(process_error)
}

pub(super) async fn wait_for_rpc_response<R>(
    reader: &mut BufReader<R>,
    expected_id: i64,
) -> AppResult<Value>
where
    R: AsyncRead + Unpin,
{
    while let Some(value) = next_json_rpc(reader).await? {
        if json_rpc_id_matches(&value, expected_id) {
            return Ok(value);
        }
    }
    Err(AppError::Validation(format!(
        "Codex app-server closed before JSON-RPC response {expected_id}"
    )))
}

pub(super) async fn next_json_rpc<R>(reader: &mut BufReader<R>) -> AppResult<Option<Value>>
where
    R: AsyncRead + Unpin,
{
    while let Some(line) = read_bounded_line(reader, MAX_CODEX_JSON_MESSAGE_BYTES).await? {
        let BoundedLine::Message(line) = line else {
            continue;
        };
        if let Ok(value) = serde_json::from_slice::<Value>(&line) {
            return Ok(Some(value));
        }
    }
    Ok(None)
}

pub(super) fn json_rpc_id_matches(value: &Value, expected_id: i64) -> bool {
    value
        .get("id")
        .and_then(Value::as_i64)
        .is_some_and(|id| id == expected_id)
}

pub(super) fn rpc_error_message(value: &Value) -> Option<String> {
    value
        .pointer("/error/message")
        .and_then(Value::as_str)
        .map(str::to_string)
}

pub(super) fn extract_agent_message(item: &Value) -> Option<String> {
    (item.get("type").and_then(Value::as_str) == Some("agentMessage"))
        .then(|| item.get("text").and_then(Value::as_str))
        .flatten()
        .map(|message| truncate(message, 4_000))
}

pub(super) fn extract_final_turn_message(turn: &Value) -> Option<String> {
    turn.get("items")
        .and_then(Value::as_array)
        .and_then(|items| items.iter().rev().find_map(extract_agent_message))
}
