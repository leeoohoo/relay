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
    let mut lines = BufReader::new(reader).lines();
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
    let initialize = wait_for_rpc_response(&mut lines, 0).await?;
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
    let thread_response = wait_for_rpc_response(&mut lines, 1).await?;
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
    while let Some(value) = next_json_rpc(&mut lines).await? {
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
            handle_app_server_request(writer, &value, approval_handler).await?;
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
    lines: &mut tokio::io::Lines<BufReader<R>>,
    expected_id: i64,
) -> AppResult<Value>
where
    R: AsyncRead + Unpin,
{
    while let Some(value) = next_json_rpc(lines).await? {
        if json_rpc_id_matches(&value, expected_id) {
            return Ok(value);
        }
    }
    Err(AppError::Validation(format!(
        "Codex app-server closed before JSON-RPC response {expected_id}"
    )))
}

pub(super) async fn next_json_rpc<R>(
    lines: &mut tokio::io::Lines<BufReader<R>>,
) -> AppResult<Option<Value>>
where
    R: AsyncRead + Unpin,
{
    while let Some(line) = lines.next_line().await.map_err(process_error)? {
        if line.len() > MAX_JSONL_LINE_BYTES {
            return Err(AppError::Validation(
                "Codex emitted an oversized JSON-RPC message".into(),
            ));
        }
        if let Ok(value) = serde_json::from_str::<Value>(&line) {
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
