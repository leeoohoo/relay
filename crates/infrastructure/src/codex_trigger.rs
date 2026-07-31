use std::{collections::HashMap, path::PathBuf, process::Stdio, sync::Arc, time::Duration};

use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader},
    process::Command,
    time::timeout,
};

use ai_chat_domain::company::{
    AGENT_CODEX_APPROVAL_TOOL_COMMAND, AGENT_CODEX_APPROVAL_TOOL_FILE_CHANGE,
    AGENT_CODEX_APPROVAL_TOOL_PERMISSIONS,
};
use ai_chat_shared::{AppError, AppResult};

const DEFAULT_RUN_TOKEN_ENV: &str = "RELAY_AGENT_RUN_TOKEN";
const MAX_STDERR_BYTES: usize = 32 * 1024;
const MAX_JSONL_LINE_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone)]
pub struct CodexTriggerRunner {
    executable: PathBuf,
    prefix_args: Vec<String>,
    mcp_url: String,
    mcp_server_name: String,
    run_token_env_name: String,
    inherited_environment: HashMap<String, String>,
    shell_excluded_environment_names: Vec<String>,
}

#[derive(Clone)]
pub struct CodexRunRequest {
    pub cwd: PathBuf,
    pub codex_profile: String,
    pub model: Option<String>,
    pub reasoning_effort: Option<String>,
    pub sandbox_mode: String,
    pub approval_policy: String,
    pub max_run_seconds: u64,
    pub prompt: String,
    pub existing_thread_id: Option<String>,
    pub run_token: String,
    pub environment: HashMap<String, String>,
    pub approval_handler: Option<Arc<dyn CodexApprovalHandler>>,
    pub progress_handler: Option<Arc<dyn CodexProgressHandler>>,
}

#[derive(Debug, Clone)]
pub struct CodexProgressEvent {
    pub phase: String,
    pub summary: String,
    pub thread_id: Option<String>,
}

pub trait CodexProgressHandler: Send + Sync {
    fn report(&self, event: CodexProgressEvent);
}

#[derive(Debug, Clone)]
pub struct CodexApprovalRequest {
    pub tool_name: String,
    pub risk_level: String,
    pub reason: String,
    pub arguments: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodexApprovalDecision {
    Accept,
    Decline,
}

#[async_trait]
pub trait CodexApprovalHandler: Send + Sync {
    async fn request_approval(
        &self,
        request: CodexApprovalRequest,
    ) -> AppResult<CodexApprovalDecision>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodexRunStatus {
    Succeeded,
    Failed,
    TimedOut,
}

#[derive(Debug, Clone)]
pub struct CodexRunResult {
    pub status: CodexRunStatus,
    pub thread_id: Option<String>,
    pub exit_code: Option<i32>,
    pub final_message: Option<String>,
    pub error_message: Option<String>,
    pub resumed_existing_session: bool,
    pub replaced_unresumable_session: bool,
}

#[derive(Debug)]
struct ProcessOutcome {
    status: CodexRunStatus,
    thread_id: Option<String>,
    exit_code: Option<i32>,
    final_message: Option<String>,
    error_message: Option<String>,
    turn_started: bool,
}

#[derive(Debug, Default)]
struct JsonlEvents {
    thread_id: Option<String>,
    turn_started: bool,
    turn_completed: bool,
    turn_failed: bool,
    final_message: Option<String>,
    error_message: Option<String>,
}

impl CodexTriggerRunner {
    pub fn from_env() -> AppResult<Self> {
        let executable = std::env::var("AGENT_TRIGGER_CODEX_BIN")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("codex"));
        let prefix_args = std::env::var("AGENT_TRIGGER_CODEX_PREFIX_ARGS_JSON")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(|value| {
                serde_json::from_str::<Vec<String>>(&value).map_err(|error| {
                    AppError::Validation(format!(
                        "invalid AGENT_TRIGGER_CODEX_PREFIX_ARGS_JSON: {error}"
                    ))
                })
            })
            .transpose()?
            .unwrap_or_default();
        let mcp_url = std::env::var("AGENT_TRIGGER_MCP_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:8080/mcp".into());
        let mcp_server_name = std::env::var("AGENT_TRIGGER_MCP_SERVER_NAME")
            .unwrap_or_else(|_| "relay_company".into());
        let mut runner = Self::new(
            executable,
            prefix_args,
            mcp_url,
            mcp_server_name,
            DEFAULT_RUN_TOKEN_ENV.into(),
        )?;
        let allowlist = std::env::var("AGENT_TRIGGER_CODEX_ENV_ALLOWLIST")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(|value| split_csv(&value))
            .unwrap_or_else(default_environment_allowlist);
        for name in &allowlist {
            validate_environment_name(name)?;
        }
        runner.inherited_environment = collect_inherited_environment(&allowlist);
        let mut excluded = std::env::var("AGENT_TRIGGER_CODEX_SENSITIVE_ENV_NAMES")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(|value| split_csv(&value))
            .unwrap_or_else(|| vec!["CODEX_API_KEY".into(), "OPENAI_API_KEY".into()]);
        excluded.push(runner.run_token_env_name.clone());
        excluded.sort();
        excluded.dedup();
        for name in &excluded {
            validate_environment_name(name)?;
        }
        runner.shell_excluded_environment_names = excluded;
        Ok(runner)
    }

    pub fn new(
        executable: PathBuf,
        prefix_args: Vec<String>,
        mcp_url: String,
        mcp_server_name: String,
        run_token_env_name: String,
    ) -> AppResult<Self> {
        validate_safe_value(&mcp_url, "AGENT_TRIGGER_MCP_URL", 2_048)?;
        if !mcp_url.starts_with("http://") && !mcp_url.starts_with("https://") {
            return Err(AppError::Validation(
                "AGENT_TRIGGER_MCP_URL must use http or https".into(),
            ));
        }
        validate_config_key(&mcp_server_name, "AGENT_TRIGGER_MCP_SERVER_NAME")?;
        validate_environment_name(&run_token_env_name)?;
        for argument in &prefix_args {
            validate_safe_value(argument, "Codex command prefix argument", 4_096)?;
        }
        Ok(Self {
            executable,
            prefix_args,
            mcp_url,
            mcp_server_name,
            inherited_environment: collect_inherited_environment(&default_environment_allowlist()),
            shell_excluded_environment_names: vec![
                "CODEX_API_KEY".into(),
                "OPENAI_API_KEY".into(),
                run_token_env_name.clone(),
            ],
            run_token_env_name,
        })
    }

    pub fn detect_version(&self) -> Option<String> {
        let mut command = std::process::Command::new(&self.executable);
        command
            .args(&self.prefix_args)
            .arg("--version")
            .env_clear()
            .envs(&self.inherited_environment);
        let output = command.output().ok()?;
        if !output.status.success() {
            return None;
        }
        let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
        (!version.is_empty()).then(|| truncate(&version, 200))
    }

    pub async fn run(&self, request: CodexRunRequest) -> AppResult<CodexRunResult> {
        validate_request(&request)?;
        if let Some(thread_id) = request.existing_thread_id.as_deref() {
            let resumed = self.run_once(&request, Some(thread_id)).await?;
            if should_replace_session(&resumed) {
                let created = self.run_once(&request, None).await?;
                return Ok(to_public_result(created, false, true));
            }
            return Ok(to_public_result(resumed, true, false));
        }
        let created = self.run_once(&request, None).await?;
        Ok(to_public_result(created, false, false))
    }

    async fn run_once(
        &self,
        request: &CodexRunRequest,
        resume_thread_id: Option<&str>,
    ) -> AppResult<ProcessOutcome> {
        if request.approval_policy == "on-request" {
            self.run_app_server_once(request, resume_thread_id).await
        } else {
            self.run_exec_once(request, resume_thread_id).await
        }
    }

    async fn run_exec_once(
        &self,
        request: &CodexRunRequest,
        resume_thread_id: Option<&str>,
    ) -> AppResult<ProcessOutcome> {
        let sandbox_mode = codex_sandbox_mode(&request.sandbox_mode)?;
        let mut command = Command::new(&self.executable);
        command
            .args(&self.prefix_args)
            .arg("--ask-for-approval")
            .arg("never");
        command
            .arg("exec")
            .arg("--skip-git-repo-check")
            .arg("--json");
        if request.codex_profile != "default" {
            command.arg("--profile").arg(&request.codex_profile);
        }
        if let Some(model) = request.model.as_deref() {
            command.arg("--model").arg(model);
        }
        if let Some(reasoning_effort) = request.reasoning_effort.as_deref() {
            command.arg("--config").arg(format!(
                "model_reasoning_effort={}",
                toml_string(reasoning_effort)
            ));
        }
        command
            .arg("--sandbox")
            .arg(sandbox_mode)
            .arg("--config")
            .arg(format!(
                "mcp_servers.{}.url={}",
                self.mcp_server_name,
                toml_string(&self.mcp_url)
            ))
            .arg("--config")
            .arg(format!(
                "mcp_servers.{}.required=true",
                self.mcp_server_name
            ))
            .arg("--config")
            .arg(format!(
                "mcp_servers.{}.env_http_headers={{ \"x-agent-run-token\" = {} }}",
                self.mcp_server_name,
                toml_string(&self.run_token_env_name)
            ))
            .arg("--config")
            .arg(format!(
                "shell_environment_policy.exclude={}",
                toml_string_array(&self.shell_excluded_environment_names)
            ))
            .arg("--config")
            .arg("shell_environment_policy.ignore_default_excludes=true");
        if sandbox_mode == "workspace-write" {
            command
                .arg("--config")
                .arg("sandbox_workspace_write.network_access=true");
        }
        if let Some(thread_id) = resume_thread_id {
            command.arg("resume").arg(thread_id);
        }
        command
            .arg(&request.prompt)
            .current_dir(&request.cwd)
            .env_clear()
            .envs(&self.inherited_environment)
            .env(&self.run_token_env_name, &request.run_token)
            .env("GIT_TERMINAL_PROMPT", "0")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        for (key, value) in &request.environment {
            command.env(key, value);
        }
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.as_std_mut().process_group(0);
        }

        let mut child = command.spawn().map_err(|error| {
            AppError::Validation(format!(
                "failed to start Codex executable {}: {}",
                self.executable.display(),
                sanitize_error(&error.to_string())
            ))
        })?;
        let process_id = child.id();
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AppError::Validation("Codex stdout pipe was not available".into()))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| AppError::Validation("Codex stderr pipe was not available".into()))?;
        report_progress(
            request.progress_handler.as_ref(),
            "starting",
            "正在启动本地 Codex",
            resume_thread_id,
        );
        let stdout_task = tokio::spawn(read_jsonl_events(stdout, request.progress_handler.clone()));
        let stderr_task = tokio::spawn(read_limited_text(stderr, MAX_STDERR_BYTES));

        report_progress(
            request.progress_handler.as_ref(),
            "starting",
            "正在连接 Codex app-server",
            resume_thread_id,
        );

        let mut timed_out = false;
        let wait_result = timeout(Duration::from_secs(request.max_run_seconds), child.wait()).await;
        let exit_status = match wait_result {
            Ok(result) => result.map_err(process_error)?,
            Err(_) => {
                timed_out = true;
                terminate_process_tree(process_id);
                match timeout(Duration::from_secs(5), child.wait()).await {
                    Ok(result) => result.map_err(process_error)?,
                    Err(_) => {
                        kill_process_tree(process_id);
                        child.wait().await.map_err(process_error)?
                    }
                }
            }
        };
        let events = stdout_task.await.map_err(join_error)??;
        let stderr = stderr_task.await.map_err(join_error)??;
        let mut error_message = events.error_message.clone();
        if error_message.is_none() && !exit_status.success() && !stderr.trim().is_empty() {
            error_message = Some(truncate(&sanitize_error(&stderr), 2_000));
        }
        if timed_out {
            error_message = Some(format!(
                "Codex run exceeded {} seconds",
                request.max_run_seconds
            ));
        }
        let mut status = if timed_out {
            CodexRunStatus::TimedOut
        } else if exit_status.success() && events.turn_completed && !events.turn_failed {
            CodexRunStatus::Succeeded
        } else {
            CodexRunStatus::Failed
        };
        let thread_id = events
            .thread_id
            .or_else(|| resume_thread_id.map(str::to_string));
        if status == CodexRunStatus::Succeeded && thread_id.is_none() {
            status = CodexRunStatus::Failed;
            error_message = Some("Codex completed without emitting a thread.started event".into());
        }
        if status == CodexRunStatus::Succeeded {
            // Codex may emit transient transport errors while its own reconnect loop is
            // recovering. A completed turn is authoritative, so those intermediate
            // warnings must not be persisted as the final run error.
            error_message = None;
        }
        if status == CodexRunStatus::Failed && error_message.is_none() {
            error_message = Some("Codex run failed before completing the turn".into());
        }
        Ok(ProcessOutcome {
            status,
            thread_id,
            exit_code: exit_status.code(),
            final_message: events.final_message,
            error_message,
            turn_started: events.turn_started,
        })
    }

    async fn run_app_server_once(
        &self,
        request: &CodexRunRequest,
        resume_thread_id: Option<&str>,
    ) -> AppResult<ProcessOutcome> {
        let sandbox_mode = codex_sandbox_mode(&request.sandbox_mode)?;
        let approval_handler = request.approval_handler.as_ref().ok_or_else(|| {
            AppError::Validation(
                "Codex on-request approval policy requires an approval handler".into(),
            )
        })?;
        let mut command = Command::new(&self.executable);
        command.args(&self.prefix_args);
        if request.codex_profile != "default" {
            command.arg("--profile").arg(&request.codex_profile);
        }
        if let Some(reasoning_effort) = request.reasoning_effort.as_deref() {
            command.arg("--config").arg(format!(
                "model_reasoning_effort={}",
                toml_string(reasoning_effort)
            ));
        }
        command
            .arg("--config")
            .arg(format!(
                "mcp_servers.{}.url={}",
                self.mcp_server_name,
                toml_string(&self.mcp_url)
            ))
            .arg("--config")
            .arg(format!(
                "mcp_servers.{}.required=true",
                self.mcp_server_name
            ))
            .arg("--config")
            .arg(format!(
                "mcp_servers.{}.env_http_headers={{ \"x-agent-run-token\" = {} }}",
                self.mcp_server_name,
                toml_string(&self.run_token_env_name)
            ))
            .arg("--config")
            .arg(format!(
                "shell_environment_policy.exclude={}",
                toml_string_array(&self.shell_excluded_environment_names)
            ))
            .arg("--config")
            .arg("shell_environment_policy.ignore_default_excludes=true");
        if sandbox_mode == "workspace-write" {
            command
                .arg("--config")
                .arg("sandbox_workspace_write.network_access=true");
        }
        command
            .arg("app-server")
            .arg("--stdio")
            .current_dir(&request.cwd)
            .env_clear()
            .envs(&self.inherited_environment)
            .env(&self.run_token_env_name, &request.run_token)
            .env("GIT_TERMINAL_PROMPT", "0")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        for (key, value) in &request.environment {
            command.env(key, value);
        }
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.as_std_mut().process_group(0);
        }

        let mut child = command.spawn().map_err(|error| {
            AppError::Validation(format!(
                "failed to start Codex app-server {}: {}",
                self.executable.display(),
                sanitize_error(&error.to_string())
            ))
        })?;
        let process_id = child.id();
        let mut stdin = child.stdin.take().ok_or_else(|| {
            AppError::Validation("Codex app-server stdin pipe was not available".into())
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            AppError::Validation("Codex app-server stdout pipe was not available".into())
        })?;
        let stderr = child.stderr.take().ok_or_else(|| {
            AppError::Validation("Codex app-server stderr pipe was not available".into())
        })?;
        let stderr_task = tokio::spawn(read_limited_text(stderr, MAX_STDERR_BYTES));

        let drive_result = timeout(
            Duration::from_secs(request.max_run_seconds),
            drive_app_server(
                &mut stdin,
                stdout,
                request,
                resume_thread_id,
                sandbox_mode,
                approval_handler.as_ref(),
            ),
        )
        .await;
        let mut outcome = match drive_result {
            Ok(Ok(outcome)) => outcome,
            Ok(Err(error)) => ProcessOutcome {
                status: CodexRunStatus::Failed,
                thread_id: resume_thread_id.map(str::to_string),
                exit_code: Some(1),
                final_message: None,
                error_message: Some(truncate(&sanitize_error(&error.to_string()), 2_000)),
                turn_started: false,
            },
            Err(_) => ProcessOutcome {
                status: CodexRunStatus::TimedOut,
                thread_id: resume_thread_id.map(str::to_string),
                exit_code: None,
                final_message: None,
                error_message: Some(format!(
                    "Codex run exceeded {} seconds while waiting for completion or approval",
                    request.max_run_seconds
                )),
                turn_started: true,
            },
        };

        drop(stdin);
        let exit_status = match timeout(Duration::from_secs(3), child.wait()).await {
            Ok(result) => Some(result.map_err(process_error)?),
            Err(_) => {
                terminate_process_tree(process_id);
                match timeout(Duration::from_secs(3), child.wait()).await {
                    Ok(result) => Some(result.map_err(process_error)?),
                    Err(_) => {
                        kill_process_tree(process_id);
                        Some(child.wait().await.map_err(process_error)?)
                    }
                }
            }
        };
        let stderr = stderr_task.await.map_err(join_error)??;
        if outcome.status == CodexRunStatus::Failed
            && outcome.error_message.is_none()
            && !stderr.trim().is_empty()
        {
            outcome.error_message = Some(truncate(&sanitize_error(&stderr), 2_000));
        }
        if outcome.exit_code.is_none() {
            outcome.exit_code = exit_status.and_then(|status| status.code());
        }
        Ok(outcome)
    }
}

async fn drive_app_server<W, R>(
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

async fn handle_app_server_request<W>(
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

async fn send_approval_response<W>(
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

async fn send_json_rpc<W>(writer: &mut W, value: &Value) -> AppResult<()>
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

async fn wait_for_rpc_response<R>(
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

async fn next_json_rpc<R>(lines: &mut tokio::io::Lines<BufReader<R>>) -> AppResult<Option<Value>>
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

fn json_rpc_id_matches(value: &Value, expected_id: i64) -> bool {
    value
        .get("id")
        .and_then(Value::as_i64)
        .is_some_and(|id| id == expected_id)
}

fn rpc_error_message(value: &Value) -> Option<String> {
    value
        .pointer("/error/message")
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn extract_agent_message(item: &Value) -> Option<String> {
    (item.get("type").and_then(Value::as_str) == Some("agentMessage"))
        .then(|| item.get("text").and_then(Value::as_str))
        .flatten()
        .map(|message| truncate(message, 4_000))
}

fn extract_final_turn_message(turn: &Value) -> Option<String> {
    turn.get("items")
        .and_then(Value::as_array)
        .and_then(|items| items.iter().rev().find_map(extract_agent_message))
}

fn validate_request(request: &CodexRunRequest) -> AppResult<()> {
    if !request.cwd.is_absolute() || !request.cwd.is_dir() {
        return Err(AppError::Validation(
            "Codex working directory must be an existing absolute directory".into(),
        ));
    }
    validate_config_key(&request.codex_profile, "codex_profile")?;
    if let Some(model) = request.model.as_deref() {
        validate_safe_value(model, "Codex model", 128)?;
    }
    if let Some(reasoning_effort) = request.reasoning_effort.as_deref() {
        if !matches!(
            reasoning_effort,
            "minimal" | "low" | "medium" | "high" | "xhigh" | "max" | "ultra"
        ) {
            return Err(AppError::Validation(
                "Codex reasoning effort must be minimal, low, medium, high, xhigh, max, or ultra"
                    .into(),
            ));
        }
    }
    if !matches!(request.approval_policy.as_str(), "never" | "on-request") {
        return Err(AppError::Validation(
            "Codex approval policy must be never or on-request".into(),
        ));
    }
    if request.approval_policy == "on-request" && request.approval_handler.is_none() {
        return Err(AppError::Validation(
            "Codex on-request approval policy requires an approval handler".into(),
        ));
    }
    if request.max_run_seconds == 0 {
        return Err(AppError::Validation(
            "Codex max_run_seconds must be positive".into(),
        ));
    }
    validate_prompt(&request.prompt)?;
    if request.run_token.trim().is_empty() || request.run_token.chars().any(char::is_control) {
        return Err(AppError::Validation("Codex run token is invalid".into()));
    }
    if let Some(thread_id) = request.existing_thread_id.as_deref() {
        validate_safe_value(thread_id, "Codex thread ID", 200)?;
    }
    for (key, value) in &request.environment {
        validate_environment_name(key)?;
        if value
            .chars()
            .any(|character| matches!(character, '\0' | '\r' | '\n'))
        {
            return Err(AppError::Validation(format!(
                "Codex environment value for {key} contains control characters"
            )));
        }
    }
    Ok(())
}

fn codex_sandbox_mode(value: &str) -> AppResult<&'static str> {
    match value {
        "read_only" => Ok("read-only"),
        "workspace_write" => Ok("workspace-write"),
        _ => Err(AppError::Validation(
            "Codex sandbox mode must be read_only or workspace_write".into(),
        )),
    }
}

async fn read_jsonl_events<R>(
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

fn report_progress(
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

fn summarize_codex_item(item: &Value, completed: bool) -> Option<(String, String)> {
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

fn mcp_tool_action(item: &Value) -> Option<String> {
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

fn codex_item_failed(item: &Value) -> bool {
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

async fn read_limited_text<R>(mut reader: R, max_bytes: usize) -> AppResult<String>
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

fn event_error_message(value: &Value) -> Option<String> {
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

fn should_replace_session(outcome: &ProcessOutcome) -> bool {
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

fn to_public_result(
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

fn validate_safe_value(value: &str, field: &str, max_characters: usize) -> AppResult<()> {
    if value.trim().is_empty()
        || value.chars().count() > max_characters
        || value
            .chars()
            .any(|character| matches!(character, '\0' | '\r' | '\n'))
    {
        return Err(AppError::Validation(format!("{field} is invalid")));
    }
    Ok(())
}

fn validate_prompt(value: &str) -> AppResult<()> {
    if value.trim().is_empty()
        || value.chars().count() > 20_000
        || value
            .chars()
            .any(|character| character.is_control() && !matches!(character, '\n' | '\t'))
    {
        return Err(AppError::Validation("Codex prompt is invalid".into()));
    }
    Ok(())
}

fn validate_config_key(value: &str, field: &str) -> AppResult<()> {
    if value.is_empty()
        || value.chars().count() > 80
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err(AppError::Validation(format!(
            "{field} contains unsupported characters"
        )));
    }
    Ok(())
}

fn validate_environment_name(value: &str) -> AppResult<()> {
    let mut characters = value.chars();
    let valid_first = characters
        .next()
        .is_some_and(|character| character == '_' || character.is_ascii_alphabetic());
    if !valid_first
        || !characters.all(|character| character == '_' || character.is_ascii_alphanumeric())
    {
        return Err(AppError::Validation(format!(
            "environment variable name {value} is invalid"
        )));
    }
    Ok(())
}

fn toml_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".into())
}

fn toml_string_array(values: &[String]) -> String {
    let values = values
        .iter()
        .map(|value| toml_string(value))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{values}]")
}

fn split_csv(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect()
}

fn default_environment_allowlist() -> Vec<String> {
    [
        "PATH",
        "HOME",
        "USER",
        "LOGNAME",
        "SHELL",
        "TMPDIR",
        "TMP",
        "TEMP",
        "LANG",
        "LC_ALL",
        "LC_CTYPE",
        "TERM",
        "COLORTERM",
        "CODEX_HOME",
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "ALL_PROXY",
        "NO_PROXY",
        "http_proxy",
        "https_proxy",
        "all_proxy",
        "no_proxy",
        "SSL_CERT_FILE",
        "SSL_CERT_DIR",
        "SYSTEMROOT",
        "COMSPEC",
        "PATHEXT",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

fn collect_inherited_environment(allowlist: &[String]) -> HashMap<String, String> {
    allowlist
        .iter()
        .filter_map(|name| std::env::var(name).ok().map(|value| (name.clone(), value)))
        .collect()
}

fn process_error(error: std::io::Error) -> AppError {
    AppError::Validation(format!(
        "Codex process I/O error: {}",
        sanitize_error(&error.to_string())
    ))
}

fn join_error(error: tokio::task::JoinError) -> AppError {
    AppError::Validation(format!("Codex output reader failed: {error}"))
}

fn sanitize_error(value: &str) -> String {
    value.replace(['\r', '\n'], " ")
}

fn truncate(value: &str, max_characters: usize) -> String {
    value.chars().take(max_characters).collect()
}

#[cfg(unix)]
fn terminate_process_tree(process_id: Option<u32>) {
    if let Some(process_id) = process_id {
        unsafe {
            libc::kill(-(process_id as i32), libc::SIGTERM);
        }
    }
}

#[cfg(not(unix))]
fn terminate_process_tree(_process_id: Option<u32>) {}

#[cfg(unix)]
fn kill_process_tree(process_id: Option<u32>) {
    if let Some(process_id) = process_id {
        unsafe {
            libc::kill(-(process_id as i32), libc::SIGKILL);
        }
        std::thread::yield_now();
    }
}

#[cfg(not(unix))]
fn kill_process_tree(_process_id: Option<u32>) {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Default)]
    struct AcceptingApprovalHandler {
        calls: AtomicUsize,
    }

    #[async_trait]
    impl CodexApprovalHandler for AcceptingApprovalHandler {
        async fn request_approval(
            &self,
            request: CodexApprovalRequest,
        ) -> AppResult<CodexApprovalDecision> {
            assert_eq!(request.tool_name, AGENT_CODEX_APPROVAL_TOOL_COMMAND);
            assert_eq!(
                request
                    .arguments
                    .pointer("/params/command")
                    .and_then(Value::as_str),
                Some("git push origin relay/test")
            );
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(CodexApprovalDecision::Accept)
        }
    }

    #[test]
    fn parser_tracks_thread_and_final_message() {
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        let input = concat!(
            "{\"type\":\"thread.started\",\"thread_id\":\"thread-1\"}\n",
            "{\"type\":\"turn.started\"}\n",
            "{\"type\":\"item.completed\",\"item\":{\"type\":\"agent_message\",\"text\":\"done\"}}\n",
            "{\"type\":\"turn.completed\"}\n"
        );
        let events = runtime
            .block_on(read_jsonl_events(input.as_bytes(), None))
            .expect("events");
        assert_eq!(events.thread_id.as_deref(), Some("thread-1"));
        assert!(events.turn_started);
        assert!(events.turn_completed);
        assert_eq!(events.final_message.as_deref(), Some("done"));
    }

    #[test]
    fn mcp_progress_summary_includes_the_action() {
        let item = json!({
            "type": "mcp_tool_call",
            "server": "relay_company",
            "tool": "company.project",
            "arguments": { "action": "create", "name": "WMS" }
        });
        let (_, started) = summarize_codex_item(&item, false).expect("summary should exist");
        let (_, completed) = summarize_codex_item(&item, true).expect("summary should exist");
        assert_eq!(
            started,
            "正在调用工具：relay_company.company.project（action: create）"
        );
        assert_eq!(
            completed,
            "完成调用工具：relay_company.company.project（action: create）"
        );

        let failed_item = json!({
            "type": "mcp_tool_call",
            "server": "relay_company",
            "tool": "company.project",
            "arguments": "{\"action\":\"member_add\"}",
            "error": { "message": "Agent is still provisioning" }
        });
        let (_, failed) =
            summarize_codex_item(&failed_item, true).expect("failure summary should exist");
        assert_eq!(
            failed,
            "工具调用失败：relay_company.company.project（action: member_add）"
        );
    }

    #[test]
    fn only_pre_turn_resume_errors_replace_a_session() {
        assert!(should_replace_session(&ProcessOutcome {
            status: CodexRunStatus::Failed,
            thread_id: None,
            exit_code: Some(1),
            final_message: None,
            error_message: Some("session not found; cannot resume".into()),
            turn_started: false,
        }));
        assert!(!should_replace_session(&ProcessOutcome {
            status: CodexRunStatus::Failed,
            thread_id: Some("thread-1".into()),
            exit_code: Some(1),
            final_message: None,
            error_message: Some("turn failed after command execution".into()),
            turn_started: true,
        }));
    }

    #[test]
    fn codex_prompt_accepts_multiline_instructions_but_rejects_unsafe_controls() {
        assert!(validate_prompt("先读取 Inbox。\n然后处理任务。\n\t没有任务时结束。").is_ok());
        assert!(validate_prompt("unsafe\rprompt").is_err());
        assert!(validate_prompt("unsafe\0prompt").is_err());
    }

    #[test]
    fn sandbox_mode_mapping_is_independent_from_approval_policy() {
        assert_eq!(
            codex_sandbox_mode("workspace_write").expect("workspace-write policy"),
            "workspace-write"
        );
        assert_eq!(
            codex_sandbox_mode("read_only").expect("read-only policy"),
            "read-only"
        );
    }

    #[cfg(unix)]
    #[test]
    fn transient_reconnect_error_is_cleared_after_the_turn_completes() {
        let workspace = std::env::temp_dir().join(format!(
            "relay-fake-codex-reconnect-{}",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&workspace).expect("workspace");
        let script = r#"printf '%s\n' '{"type":"thread.started","thread_id":"thread-reconnect"}' '{"type":"turn.started"}' '{"type":"error","message":"Reconnecting... 1/5 (stream disconnected before completion)"}' '{"type":"item.completed","item":{"type":"agent_message","text":"work completed after reconnect"}}' '{"type":"turn.completed"}'"#;
        let runner = CodexTriggerRunner::new(
            PathBuf::from("/bin/sh"),
            vec!["-c".into(), script.into(), "--".into()],
            "http://127.0.0.1:8080/mcp".into(),
            "relay_company".into(),
            DEFAULT_RUN_TOKEN_ENV.into(),
        )
        .expect("runner");
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        let result = runtime
            .block_on(runner.run(CodexRunRequest {
                cwd: workspace.clone(),
                codex_profile: "default".into(),
                model: None,
                reasoning_effort: None,
                sandbox_mode: "workspace_write".into(),
                approval_policy: "never".into(),
                max_run_seconds: 10,
                prompt: "check Relay inbox".into(),
                existing_thread_id: None,
                run_token: "art_test".into(),
                environment: HashMap::new(),
                approval_handler: None,
                progress_handler: None,
            }))
            .expect("fake Codex run");
        assert_eq!(result.status, CodexRunStatus::Succeeded);
        assert_eq!(
            result.final_message.as_deref(),
            Some("work completed after reconnect")
        );
        assert_eq!(result.error_message, None);
        std::fs::remove_dir_all(workspace).expect("cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn fake_codex_replaces_only_an_unresumable_thread() {
        let workspace = std::env::temp_dir().join(format!(
            "relay-fake-codex-{}",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&workspace).expect("workspace");
        let script = r#"if [ "$1" = "--version" ]; then echo fake-codex-1.0; exit 0; fi; for arg in "$@"; do if [ "$arg" = "resume" ]; then echo 'session not found; cannot resume' >&2; exit 1; fi; done; printf '%s\n' '{"type":"thread.started","thread_id":"new-thread"}' '{"type":"turn.started"}' '{"type":"item.completed","item":{"type":"agent_message","text":"handled"}}' '{"type":"turn.completed"}'"#;
        let runner = CodexTriggerRunner::new(
            PathBuf::from("/bin/sh"),
            vec!["-c".into(), script.into(), "--".into()],
            "http://127.0.0.1:8080/mcp".into(),
            "relay_company".into(),
            DEFAULT_RUN_TOKEN_ENV.into(),
        )
        .expect("runner");
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        let result = runtime
            .block_on(runner.run(CodexRunRequest {
                cwd: workspace.clone(),
                codex_profile: "relay-test".into(),
                model: Some("gpt-5.6-sol".into()),
                reasoning_effort: Some("high".into()),
                sandbox_mode: "workspace_write".into(),
                approval_policy: "never".into(),
                max_run_seconds: 10,
                prompt: "check Relay inbox".into(),
                existing_thread_id: Some("missing-thread".into()),
                run_token: "art_test".into(),
                environment: HashMap::new(),
                approval_handler: None,
                progress_handler: None,
            }))
            .expect("fake Codex run");
        assert_eq!(result.status, CodexRunStatus::Succeeded);
        assert_eq!(result.thread_id.as_deref(), Some("new-thread"));
        assert!(result.replaced_unresumable_session);
        assert!(!result.resumed_existing_session);
        std::fs::remove_dir_all(workspace).expect("cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn app_server_approval_continues_the_same_turn() {
        let workspace = std::env::temp_dir().join(format!(
            "relay-fake-app-server-{}",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&workspace).expect("workspace");
        let script = r#"IFS= read -r initialize
printf '%s\n' '{"id":0,"result":{"userAgent":"fake","platformFamily":"unix","platformOs":"macos","codexHome":"/tmp"}}'
IFS= read -r initialized
IFS= read -r thread
printf '%s\n' '{"id":1,"result":{"thread":{"id":"thread-approval"},"model":"fake","modelProvider":"fake","cwd":"/tmp","approvalPolicy":"on-request","approvalsReviewer":"user","sandbox":{"type":"workspaceWrite","writableRoots":[],"readOnlyAccess":{"type":"fullAccess"},"networkAccess":true,"excludeTmpdirEnvVar":false,"excludeSlashTmp":false}}}'
IFS= read -r turn
case "$turn" in *'"effort":"high"'*) ;; *) exit 8 ;; esac
printf '%s\n' '{"id":2,"result":{"turn":{"id":"turn-1","items":[],"status":"inProgress"}}}'
printf '%s\n' '{"method":"turn/started","params":{"threadId":"thread-approval","turn":{"id":"turn-1","items":[],"status":"inProgress"}}}'
printf '%s\n' '{"id":99,"method":"item/commandExecution/requestApproval","params":{"threadId":"thread-approval","turnId":"turn-1","itemId":"item-1","startedAtMs":1,"command":"git push origin relay/test","cwd":"/tmp","reason":"push branch"}}'
IFS= read -r approval
case "$approval" in *'"decision":"accept"'*) ;; *) exit 9 ;; esac
printf '%s\n' '{"method":"item/completed","params":{"threadId":"thread-approval","turnId":"turn-1","completedAtMs":2,"item":{"id":"message-1","type":"agentMessage","text":"push completed"}}}'
printf '%s\n' '{"method":"turn/completed","params":{"threadId":"thread-approval","turn":{"id":"turn-1","items":[{"id":"message-1","type":"agentMessage","text":"push completed"}],"status":"completed"}}}'"#;
        let script_path = workspace.join("fake-app-server.sh");
        std::fs::write(&script_path, script).expect("fake app-server script");
        let runner = CodexTriggerRunner::new(
            PathBuf::from("/bin/sh"),
            vec![script_path.to_string_lossy().into_owned()],
            "http://127.0.0.1:8080/mcp".into(),
            "relay_company".into(),
            DEFAULT_RUN_TOKEN_ENV.into(),
        )
        .expect("runner");
        let handler = Arc::new(AcceptingApprovalHandler::default());
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        let result = runtime
            .block_on(runner.run(CodexRunRequest {
                cwd: workspace.clone(),
                codex_profile: "default".into(),
                model: None,
                reasoning_effort: Some("high".into()),
                sandbox_mode: "workspace_write".into(),
                approval_policy: "on-request".into(),
                max_run_seconds: 10,
                prompt: "push the branch".into(),
                existing_thread_id: None,
                run_token: "art_test".into(),
                environment: HashMap::new(),
                approval_handler: Some(handler.clone()),
                progress_handler: None,
            }))
            .expect("fake app-server run");
        assert_eq!(result.status, CodexRunStatus::Succeeded);
        assert_eq!(result.thread_id.as_deref(), Some("thread-approval"));
        assert_eq!(result.final_message.as_deref(), Some("push completed"));
        assert_eq!(handler.calls.load(Ordering::SeqCst), 1);
        std::fs::remove_dir_all(workspace).expect("cleanup");
    }
}
