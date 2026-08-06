use super::*;

pub(super) async fn read_jsonl_events<R>(
    reader: R,
    progress_handler: Option<Arc<dyn CodexProgressHandler>>,
) -> AppResult<JsonlEvents>
where
    R: AsyncRead + Unpin,
{
    let mut lines = BufReader::new(reader).lines();
    let mut events = JsonlEvents::default();
    while let Some(line) = lines.next_line().await.map_err(process_error)? {
        if line.len() > MAX_JSONL_LINE_BYTES {
            return Err(AppError::Validation(
                "Codex emitted an oversized JSONL event".into(),
            ));
        }
        let Ok(value) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        match value.get("type").and_then(Value::as_str) {
            Some("thread.started") => {
                events.thread_id = value
                    .get("thread_id")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                report_progress(
                    progress_handler.as_ref(),
                    "session",
                    "Codex 固定会话已连接",
                    events.thread_id.as_deref(),
                );
            }
            Some("turn.started") => {
                events.turn_started = true;
                report_progress(
                    progress_handler.as_ref(),
                    "thinking",
                    "Codex 已开始分析待办",
                    events.thread_id.as_deref(),
                );
            }
            Some("turn.completed") => {
                events.turn_completed = true;
                report_progress(
                    progress_handler.as_ref(),
                    "finishing",
                    "Codex 正在整理本轮结果",
                    events.thread_id.as_deref(),
                );
            }
            Some("turn.failed") => {
                events.turn_failed = true;
                events.error_message = event_error_message(&value).or(events.error_message);
            }
            Some("error") => {
                events.error_message = event_error_message(&value).or(events.error_message);
            }
            Some("item.started") => {
                if let Some(item) = value.get("item") {
                    if let Some((phase, summary)) = summarize_codex_item(item, false) {
                        report_progress(
                            progress_handler.as_ref(),
                            &phase,
                            &summary,
                            events.thread_id.as_deref(),
                        );
                    }
                }
            }
            Some("item.completed") => {
                let item = value.get("item");
                if item
                    .and_then(|item| item.get("type"))
                    .and_then(Value::as_str)
                    == Some("agent_message")
                {
                    events.final_message = item
                        .and_then(|item| item.get("text"))
                        .and_then(Value::as_str)
                        .map(|text| truncate(text, 4_000));
                }
                if let Some(item) = item {
                    if let Some((phase, summary)) = summarize_codex_item(item, true) {
                        report_progress(
                            progress_handler.as_ref(),
                            &phase,
                            &summary,
                            events.thread_id.as_deref(),
                        );
                    }
                }
            }
            _ => {}
        }
    }
    Ok(events)
}

pub(super) fn report_progress(
    handler: Option<&Arc<dyn CodexProgressHandler>>,
    phase: &str,
    summary: &str,
    thread_id: Option<&str>,
) {
    if let Some(handler) = handler {
        handler.report(CodexProgressEvent {
            phase: phase.to_string(),
            summary: truncate(&sanitize_error(summary), 500),
            thread_id: thread_id.map(str::to_string),
        });
    }
}

pub(super) fn summarize_codex_item(item: &Value, completed: bool) -> Option<(String, String)> {
    let item_type = item.get("type").and_then(Value::as_str)?;
    let completion = if completed { "完成" } else { "正在" };
    let summary = match item_type {
        "command_execution" | "commandExecution" => {
            let command = item
                .get("command")
                .and_then(Value::as_str)
                .or_else(|| item.pointer("/command/text").and_then(Value::as_str))
                .unwrap_or("命令");
            format!("{completion}执行命令：{}", truncate(command, 240))
        }
        "mcp_tool_call" | "mcpToolCall" => {
            let server = item
                .get("server")
                .or_else(|| item.get("server_name"))
                .and_then(Value::as_str);
            let tool = item
                .get("tool")
                .or_else(|| item.get("tool_name"))
                .or_else(|| item.get("name"))
                .and_then(Value::as_str)
                .unwrap_or("MCP 工具");
            let label =
                server.map_or_else(|| tool.to_string(), |server| format!("{server}.{tool}"));
            let action = mcp_tool_action(item)
                .map(|action| format!("（action: {action}）"))
                .unwrap_or_default();
            let progress = if completed && codex_item_failed(item) {
                "工具调用失败："
            } else if completed {
                "完成调用工具："
            } else {
                "正在调用工具："
            };
            format!("{progress}{label}{action}")
        }
        "file_change" | "fileChange" => format!("{completion}修改项目文件"),
        "reasoning" => format!("{completion}分析问题和下一步"),
        "agent_message" | "agentMessage" => format!("{completion}整理回复和执行结果"),
        "web_search" | "webSearch" => format!("{completion}搜索资料"),
        "todo_list" | "todoList" => format!("{completion}更新执行计划"),
        _ => return None,
    };
    let phase = match item_type {
        "command_execution" | "commandExecution" => "command",
        "mcp_tool_call" | "mcpToolCall" => "tool",
        "file_change" | "fileChange" => "files",
        "reasoning" => "thinking",
        "agent_message" | "agentMessage" => "reporting",
        "web_search" | "webSearch" => "searching",
        "todo_list" | "todoList" => "planning",
        _ => "running",
    };
    Some((phase.into(), summary))
}

pub(super) fn mcp_tool_action(item: &Value) -> Option<String> {
    ["arguments", "input", "params", "arguments_json"]
        .into_iter()
        .filter_map(|key| item.get(key))
        .find_map(|arguments| {
            arguments
                .get("action")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| {
                    arguments.as_str().and_then(|text| {
                        serde_json::from_str::<Value>(text).ok().and_then(|value| {
                            value
                                .get("action")
                                .and_then(Value::as_str)
                                .map(str::to_string)
                        })
                    })
                })
        })
}

pub(super) fn codex_item_failed(item: &Value) -> bool {
    item.get("status")
        .and_then(Value::as_str)
        .is_some_and(|status| matches!(status, "failed" | "error"))
        || item
            .get("error")
            .is_some_and(|error| !error.is_null() && error.as_str() != Some(""))
        || item
            .pointer("/result/isError")
            .and_then(Value::as_bool)
            .unwrap_or(false)
}

pub(super) async fn read_limited_text<R>(mut reader: R, max_bytes: usize) -> AppResult<String>
where
    R: AsyncRead + Unpin,
{
    use tokio::io::AsyncReadExt;

    let mut output = Vec::new();
    let mut chunk = [0_u8; 4_096];
    loop {
        let read = reader.read(&mut chunk).await.map_err(process_error)?;
        if read == 0 {
            break;
        }
        let remaining = max_bytes.saturating_sub(output.len());
        if remaining > 0 {
            output.extend_from_slice(&chunk[..read.min(remaining)]);
        }
    }
    Ok(String::from_utf8_lossy(&output).into_owned())
}

pub(super) fn event_error_message(value: &Value) -> Option<String> {
    value
        .get("message")
        .and_then(Value::as_str)
        .or_else(|| {
            value
                .get("error")
                .and_then(|error| error.get("message"))
                .and_then(Value::as_str)
        })
        .map(|message| truncate(&sanitize_error(message), 2_000))
}

pub(super) fn should_replace_session(outcome: &ProcessOutcome) -> bool {
    if outcome.status != CodexRunStatus::Failed || outcome.turn_started {
        return false;
    }
    let message = outcome
        .error_message
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    ["resume", "session", "thread", "rollout", "conversation"]
        .iter()
        .any(|marker| message.contains(marker))
        && [
            "not found",
            "missing",
            "invalid",
            "cannot",
            "could not",
            "failed",
        ]
        .iter()
        .any(|marker| message.contains(marker))
}

pub(super) fn to_public_result(
    outcome: ProcessOutcome,
    resumed_existing_session: bool,
    replaced_unresumable_session: bool,
) -> CodexRunResult {
    CodexRunResult {
        status: outcome.status,
        thread_id: outcome.thread_id,
        exit_code: outcome.exit_code,
        final_message: outcome.final_message,
        error_message: outcome.error_message,
        resumed_existing_session,
        replaced_unresumable_session,
    }
}
