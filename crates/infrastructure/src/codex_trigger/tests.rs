use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

#[cfg(unix)]
#[test]
fn project_session_kind_is_forwarded_to_relay_mcp() {
    let workspace = std::env::temp_dir().join(format!(
        "relay-session-kind-test-{}",
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::create_dir_all(&workspace).expect("workspace");
    let script = r#"test "$RELAY_AGENT_SESSION_KIND" = project || exit 8; case "$*" in *x-relay-session-kind*) ;; *) exit 9 ;; esac; printf '%s\n' '{"type":"thread.started","thread_id":"thread-session-kind"}' '{"type":"turn.started"}' '{"type":"item.completed","item":{"type":"agent_message","text":"done"}}' '{"type":"turn.completed"}'"#;
    let runner = CodexTriggerRunner::new(
        PathBuf::from("/bin/sh"),
        vec!["-c".into(), script.into(), "--".into()],
        "http://127.0.0.1:8080/mcp".into(),
        "relay_company".into(),
        DEFAULT_RUN_TOKEN_ENV.into(),
    )
    .expect("runner");
    let result = tokio::runtime::Runtime::new()
        .expect("runtime")
        .block_on(runner.run(CodexRunRequest {
            cwd: workspace.clone(),
            codex_profile: "default".into(),
            model: None,
            reasoning_effort: None,
            reasoning_summary: None,
            verbosity: None,
            personality: None,
            service_tier: None,
            sandbox_mode: "workspace_write".into(),
            approval_policy: "never".into(),
            network_access: false,
            web_search: "disabled".into(),
            feature_multi_agent: false,
            feature_remote_plugin: false,
            feature_hooks: false,
            feature_goals: false,
            feature_shell_tool: false,
            max_run_seconds: 10,
            prompt: "work".into(),
            existing_thread_id: None,
            run_token: "art_test".into(),
            session_kind: "project".into(),
            environment: HashMap::new(),
            managed_mcp_servers: Vec::new(),
            approval_handler: None,
            progress_handler: None,
            cancellation_handler: None,
        }))
        .expect("fake Codex run");
    assert_eq!(result.status, CodexRunStatus::Succeeded);
    std::fs::remove_dir_all(workspace).expect("cleanup");
}

#[test]
fn context_compaction_is_reported_as_session_maintenance() {
    let started = summarize_codex_item(&json!({ "type": "contextCompaction" }), false)
        .expect("started compaction summary");
    let completed = summarize_codex_item(&json!({ "type": "contextCompaction" }), true)
        .expect("completed compaction summary");

    assert_eq!(started.0, "compacting");
    assert!(started.1.contains("正在压缩"));
    assert_eq!(completed.0, "compacting");
    assert!(completed.1.contains("已压缩"));
}

#[test]
fn agent_messages_report_the_actual_progress_text() {
    let item = json!({
        "type": "agent_message",
        "text": "Production 已进入 Surefire，正在等待目标测试结果。"
    });
    let (phase, summary) =
        summarize_codex_item(&item, true).expect("agent message summary should exist");

    assert_eq!(phase, "reporting");
    assert_eq!(
        summary,
        "Production 已进入 Surefire，正在等待目标测试结果。"
    );
}

#[test]
fn empty_agent_messages_keep_a_useful_fallback() {
    let (phase, summary) = summarize_codex_item(&json!({ "type": "agentMessage" }), true)
        .expect("agent message fallback should exist");

    assert_eq!(phase, "reporting");
    assert_eq!(summary, "Agent 已更新执行进度");
}

#[test]
fn progress_text_redacts_common_secret_assignments() {
    let sanitized = sanitize_error(
        "mvn -Dflyway.password=wms_dev_password API_TOKEN=abc --client-secret hidden Authorization: Bearer token-value",
    );

    assert!(sanitized.contains("-Dflyway.password=[REDACTED]"));
    assert!(sanitized.contains("API_TOKEN=[REDACTED]"));
    assert!(sanitized.contains("--client-secret [REDACTED]"));
    assert!(sanitized.contains("Authorization: Bearer [REDACTED]"));
    assert!(!sanitized.contains("wms_dev_password"));
    assert!(!sanitized.contains("token-value"));
}

#[derive(Default)]
struct AcceptingApprovalHandler {
    calls: AtomicUsize,
}

struct AlwaysCancelHandler;

impl CodexCancellationHandler for AlwaysCancelHandler {
    fn should_cancel(&self) -> bool {
        true
    }
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
fn parser_accepts_valid_events_larger_than_the_previous_one_megabyte_limit() {
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let large_output = "x".repeat(2 * 1024 * 1024);
    let input = format!(
        "{}\n{}\n{}\n",
        serde_json::to_string(&json!({
            "type": "thread.started",
            "thread_id": "thread-large"
        }))
        .expect("thread event"),
        serde_json::to_string(&json!({
            "type": "item.completed",
            "item": {
                "type": "command_execution",
                "command": "generate output",
                "aggregated_output": large_output
            }
        }))
        .expect("large command event"),
        serde_json::to_string(&json!({ "type": "turn.completed" })).expect("turn event")
    );
    let events = runtime
        .block_on(read_jsonl_events(input.as_bytes(), None))
        .expect("large valid event should be accepted");

    assert_eq!(events.thread_id.as_deref(), Some("thread-large"));
    assert!(events.turn_completed);
}

#[test]
fn bounded_line_reader_discards_oversized_messages_and_keeps_stream_alignment() {
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let input = format!("{}\nnext\n", "x".repeat(128));
    let mut reader = BufReader::new(input.as_bytes());

    let oversized = runtime
        .block_on(read_bounded_line(&mut reader, 32))
        .expect("oversized read")
        .expect("first line");
    assert!(matches!(oversized, BoundedLine::Oversized));

    let next = runtime
        .block_on(read_bounded_line(&mut reader, 32))
        .expect("next read")
        .expect("second line");
    let BoundedLine::Message(next) = next else {
        panic!("second line should remain readable");
    };
    assert_eq!(next, b"next");
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
        "工具调用失败：relay_company.company.project（action: member_add） — Agent is still provisioning"
    );

    let failed_content_item = json!({
        "type": "mcpToolCall",
        "server": "chrome-devtools",
        "tool": "take_snapshot",
        "status": "failed",
        "result": {
            "isError": true,
            "content": [{
                "type": "text",
                "text": "Could not save a file\nCause: EACCES: permission denied, mkdir '/docs'"
            }]
        }
    });
    let (_, failed_content) = summarize_codex_item(&failed_content_item, true)
        .expect("failure content summary should exist");
    assert_eq!(
        failed_content,
        "工具调用失败：chrome-devtools.take_snapshot — Could not save a file Cause: EACCES: permission denied, mkdir '/docs'"
    );
}

#[test]
fn mcp_discovery_view_removes_secrets_and_argument_contents() {
    let configured_names = std::collections::HashSet::from(["private-http".to_string()]);
    let http = safe_mcp_server_view(
        json!({
            "name": "private-http",
            "enabled": true,
            "auth_status": "bearer_token",
            "transport": {
                "type": "streamable_http",
                "url": "https://user:secret@example.com/mcp?token=secret#fragment",
                "bearer_token_env_var": "PRIVATE_MCP_TOKEN",
                "http_headers": {"Authorization": "Bearer secret"}
            }
        }),
        &configured_names,
    )
    .expect("HTTP MCP view");
    assert_eq!(http.address.as_deref(), Some("https://example.com/mcp"));
    assert_eq!(
        http.bearer_token_env_var.as_deref(),
        Some("PRIVATE_MCP_TOKEN")
    );
    assert!(http.configured_by_user);

    let stdio = safe_mcp_server_view(
        json!({
            "name": "local",
            "enabled": true,
            "transport": {
                "type": "stdio",
                "command": "/usr/local/bin/npx",
                "args": ["-y", "@example/mcp", "--token", "secret"],
                "env": {"PRIVATE_TOKEN": "secret"}
            }
        }),
        &std::collections::HashSet::new(),
    )
    .expect("stdio MCP view");
    assert_eq!(stdio.command.as_deref(), Some("npx"));
    assert_eq!(stdio.argument_count, 4);
    assert!(!stdio.configured_by_user);

    let serialized = serde_json::to_string(&(http, stdio)).expect("serialize safe views");
    assert!(!serialized.contains("secret"));
    assert!(!serialized.contains("Authorization"));
    assert!(!serialized.contains("PRIVATE_TOKEN"));
    assert!(!serialized.contains("@example/mcp"));
}

#[test]
fn unresumable_or_terminal_stream_errors_replace_a_session() {
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
    assert!(should_replace_session(&ProcessOutcome {
        status: CodexRunStatus::Failed,
        thread_id: Some("thread-1".into()),
        exit_code: Some(1),
        final_message: None,
        error_message: Some(
            "stream disconnected before completion: stream closed before response.completed".into(),
        ),
        turn_started: true,
    }));
}

#[test]
fn only_pre_initialize_app_server_disconnects_are_retried() {
    assert!(should_retry_app_server_startup(&ProcessOutcome {
        status: CodexRunStatus::Failed,
        thread_id: Some("thread-1".into()),
        exit_code: Some(70),
        final_message: None,
        error_message: Some(
            "Codex app-server closed before JSON-RPC response 0; Codex stderr: temporary failure"
                .into(),
        ),
        turn_started: false,
    }));
    assert!(!should_retry_app_server_startup(&ProcessOutcome {
        status: CodexRunStatus::Failed,
        thread_id: Some("thread-1".into()),
        exit_code: Some(1),
        final_message: None,
        error_message: Some("turn failed".into()),
        turn_started: true,
    }));
    assert!(should_retry_app_server_startup(&ProcessOutcome {
        status: CodexRunStatus::Failed,
        thread_id: Some("thread-1".into()),
        exit_code: None,
        final_message: None,
        error_message: Some("Codex app-server timed out waiting for initialize response 0".into(),),
        turn_started: false,
    }));
    assert!(should_replace_session(&ProcessOutcome {
        status: CodexRunStatus::Failed,
        thread_id: Some("thread-1".into()),
        exit_code: None,
        final_message: None,
        error_message: Some(
            "Codex app-server timed out waiting for thread/resume response 1".into(),
        ),
        turn_started: false,
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

#[test]
fn default_auth_probe_reports_login_without_persisting_account_details() {
    assert_eq!(
        classify_default_auth_probe(true, "Logged in using an API key - sk-example***masked", ""),
        CodexDefaultAuthProbe {
            status: "active".into(),
            method: Some("api_key".into()),
            config: CodexDefaultConfigSummary {
                credential_hint: Some("sk-example***masked".into()),
                ..CodexDefaultConfigSummary::default()
            },
        }
    );
    assert_eq!(
        classify_default_auth_probe(true, "Logged in using ChatGPT", ""),
        CodexDefaultAuthProbe {
            status: "active".into(),
            method: Some("chatgpt".into()),
            config: CodexDefaultConfigSummary::default(),
        }
    );
    assert_eq!(
        classify_default_auth_probe(false, "", "Not logged in"),
        CodexDefaultAuthProbe {
            status: "logged_out".into(),
            method: None,
            config: CodexDefaultConfigSummary::default(),
        }
    );
}

#[test]
fn default_config_summary_exposes_only_safe_operational_metadata() {
    let mut summary = CodexDefaultConfigSummary::default();
    populate_default_config_summary(
        r#"
openai_base_url = "https://proxy.example.com/v1"
model_provider = "codex"
model = "gpt-5.6-sol"
model_reasoning_effort = "high"

[mcp_servers.relay]
url = "http://127.0.0.1:48181/mcp"
[mcp_servers.relay.env]
OPENAI_API_KEY = "must-not-be-returned"
[profiles.team]
model = "gpt-5.5"
[projects."/private/work"]
trust_level = "trusted"
[plugins."browser@openai-bundled"]
enabled = true
"#,
        &mut summary,
    );
    assert_eq!(summary.model_provider.as_deref(), Some("codex"));
    assert_eq!(
        summary.openai_base_url.as_deref(),
        Some("https://proxy.example.com/v1")
    );
    assert_eq!(summary.model.as_deref(), Some("gpt-5.6-sol"));
    assert_eq!(summary.reasoning_effort.as_deref(), Some("high"));
    assert_eq!(summary.mcp_servers, vec!["relay"]);
    assert_eq!(summary.named_profiles, vec!["team"]);
    assert_eq!(summary.trusted_project_count, 1);
    assert_eq!(summary.plugin_count, 1);
    assert!(!format!("{summary:?}").contains("must-not-be-returned"));
}

#[test]
fn managed_base_url_config_replaces_only_the_root_codex_setting() {
    let rendered = render_openai_base_url_config(
        r#"model = "gpt-5.6-sol"
openai_base_url = "https://old.example.com/v1"

[profiles.team]
openai_base_url = "keep-this-profile-value"
model = "gpt-5.5"
"#,
        Some("https://new.example.com/v1"),
    );
    assert!(rendered.contains("openai_base_url = \"https://new.example.com/v1\""));
    assert!(!rendered.contains("https://old.example.com/v1"));
    assert!(rendered.contains("openai_base_url = \"keep-this-profile-value\""));
    assert_eq!(
        rendered
            .lines()
            .filter(|line| *line == "openai_base_url = \"https://new.example.com/v1\"")
            .count(),
        1
    );
}

#[test]
fn managed_base_url_validation_rejects_credentials_and_query_tokens() {
    assert!(validate_managed_openai_base_url(Some("https://proxy.example.com/v1")).is_ok());
    assert!(
        validate_managed_openai_base_url(Some("https://user:secret@proxy.example.com/v1")).is_err()
    );
    assert!(
        validate_managed_openai_base_url(Some("https://proxy.example.com/v1?token=secret"))
            .is_err()
    );
}

#[test]
fn managed_profile_uses_an_independent_codex_home_without_profile_argument() {
    let runner = CodexTriggerRunner::new(
        PathBuf::from("codex"),
        Vec::new(),
        "http://127.0.0.1:8080/mcp".into(),
        "relay_company".into(),
        DEFAULT_RUN_TOKEN_ENV.into(),
    )
    .expect("runner");
    let profile_id = Uuid::new_v4();
    let selector = format!("relay_{profile_id}");
    let mut command = Command::new("codex");
    runner
        .apply_profile_arguments(&mut command, &selector)
        .expect("managed arguments");
    runner
        .apply_profile_environment(&mut command, &selector)
        .expect("managed environment");
    assert!(!command
        .as_std()
        .get_args()
        .any(|argument| argument == "--profile"));
    let codex_home = command
        .as_std()
        .get_envs()
        .find(|(name, _)| *name == "CODEX_HOME")
        .and_then(|(_, value)| value)
        .expect("CODEX_HOME");
    assert!(codex_home
        .to_string_lossy()
        .ends_with(&profile_id.to_string()));
}

#[test]
fn ordinary_codex_profile_still_uses_profile_argument() {
    let runner = CodexTriggerRunner::new(
        PathBuf::from("codex"),
        Vec::new(),
        "http://127.0.0.1:8080/mcp".into(),
        "relay_company".into(),
        DEFAULT_RUN_TOKEN_ENV.into(),
    )
    .expect("runner");
    let mut command = Command::new("codex");
    runner
        .apply_profile_arguments(&mut command, "team")
        .expect("ordinary arguments");
    assert_eq!(
        command
            .as_std()
            .get_args()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect::<Vec<_>>(),
        vec!["--profile", "team"]
    );
}

#[cfg(unix)]
#[test]
fn running_codex_process_is_cancelled_when_the_project_pauses() {
    let workspace = std::env::temp_dir().join(format!(
        "relay-fake-codex-cancel-{}",
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::create_dir_all(&workspace).expect("workspace");
    let runner = CodexTriggerRunner::new(
        PathBuf::from("/bin/sh"),
        vec!["-c".into(), "sleep 30".into(), "--".into()],
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
            reasoning_summary: Some("auto".into()),
            verbosity: None,
            personality: Some("pragmatic".into()),
            service_tier: None,
            sandbox_mode: "workspace_write".into(),
            approval_policy: "never".into(),
            network_access: true,
            web_search: "cached".into(),
            feature_multi_agent: true,
            feature_remote_plugin: true,
            feature_hooks: true,
            feature_goals: true,
            feature_shell_tool: true,
            max_run_seconds: 30,
            prompt: "work on project".into(),
            existing_thread_id: None,
            run_token: "art_test".into(),
            session_kind: "project".into(),
            environment: HashMap::new(),
            managed_mcp_servers: Vec::new(),
            approval_handler: None,
            progress_handler: None,
            cancellation_handler: Some(Arc::new(AlwaysCancelHandler)),
        }))
        .expect("fake Codex cancellation");
    assert_eq!(result.status, CodexRunStatus::Cancelled);
    assert!(result
        .error_message
        .as_deref()
        .is_some_and(|message| message.contains("project was paused")));
    std::fs::remove_dir_all(workspace).expect("cleanup");
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
            reasoning_summary: Some("auto".into()),
            verbosity: None,
            personality: Some("pragmatic".into()),
            service_tier: None,
            sandbox_mode: "workspace_write".into(),
            approval_policy: "never".into(),
            network_access: true,
            web_search: "cached".into(),
            feature_multi_agent: true,
            feature_remote_plugin: true,
            feature_hooks: true,
            feature_goals: true,
            feature_shell_tool: true,
            max_run_seconds: 10,
            prompt: "check Relay inbox".into(),
            existing_thread_id: None,
            run_token: "art_test".into(),
            session_kind: "control".into(),
            environment: HashMap::new(),
            managed_mcp_servers: Vec::new(),
            approval_handler: None,
            progress_handler: None,
            cancellation_handler: None,
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
            codex_profile: "default".into(),
            model: Some("gpt-5.6-sol".into()),
            reasoning_effort: Some("high".into()),
            reasoning_summary: Some("auto".into()),
            verbosity: Some("medium".into()),
            personality: Some("pragmatic".into()),
            service_tier: None,
            sandbox_mode: "workspace_write".into(),
            approval_policy: "never".into(),
            network_access: true,
            web_search: "cached".into(),
            feature_multi_agent: true,
            feature_remote_plugin: true,
            feature_hooks: true,
            feature_goals: true,
            feature_shell_tool: true,
            max_run_seconds: 10,
            prompt: "check Relay inbox".into(),
            existing_thread_id: Some("missing-thread".into()),
            run_token: "art_test".into(),
            session_kind: "control".into(),
            environment: HashMap::new(),
            managed_mcp_servers: Vec::new(),
            approval_handler: None,
            progress_handler: None,
            cancellation_handler: None,
        }))
        .expect("fake Codex run");
    assert_eq!(result.status, CodexRunStatus::Succeeded);
    assert_eq!(result.thread_id.as_deref(), Some("new-thread"));
    assert!(result.replaced_failed_session);
    assert!(!result.resumed_existing_session);
    std::fs::remove_dir_all(workspace).expect("cleanup");
}

#[cfg(unix)]
#[test]
fn fake_codex_replaces_a_thread_after_terminal_stream_disconnect() {
    let workspace = std::env::temp_dir().join(format!(
        "relay-fake-codex-stream-recovery-{}",
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::create_dir_all(&workspace).expect("workspace");
    let script = r#"for arg in "$@"; do if [ "$arg" = "resume" ]; then printf '%s\n' '{"type":"thread.started","thread_id":"large-thread"}' '{"type":"turn.started"}' '{"type":"error","message":"stream disconnected before completion: stream closed before response.completed"}'; exit 1; fi; done; printf '%s\n' '{"type":"thread.started","thread_id":"replacement-thread"}' '{"type":"turn.started"}' '{"type":"item.completed","item":{"type":"agent_message","text":"recovered"}}' '{"type":"turn.completed"}'"#;
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
            model: Some("gpt-5.5".into()),
            reasoning_effort: Some("medium".into()),
            reasoning_summary: Some("auto".into()),
            verbosity: Some("medium".into()),
            personality: Some("pragmatic".into()),
            service_tier: None,
            sandbox_mode: "workspace_write".into(),
            approval_policy: "never".into(),
            network_access: true,
            web_search: "cached".into(),
            feature_multi_agent: true,
            feature_remote_plugin: true,
            feature_hooks: true,
            feature_goals: true,
            feature_shell_tool: true,
            max_run_seconds: 10,
            prompt: "continue project work".into(),
            existing_thread_id: Some("large-thread".into()),
            run_token: "art_test".into(),
            session_kind: "project".into(),
            environment: HashMap::new(),
            managed_mcp_servers: Vec::new(),
            approval_handler: None,
            progress_handler: None,
            cancellation_handler: None,
        }))
        .expect("stream recovery run");

    assert_eq!(result.status, CodexRunStatus::Succeeded);
    assert_eq!(result.thread_id.as_deref(), Some("replacement-thread"));
    assert_eq!(result.final_message.as_deref(), Some("recovered"));
    assert!(result.replaced_failed_session);
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
            reasoning_summary: Some("auto".into()),
            verbosity: None,
            personality: Some("pragmatic".into()),
            service_tier: None,
            sandbox_mode: "workspace_write".into(),
            approval_policy: "on-request".into(),
            network_access: true,
            web_search: "cached".into(),
            feature_multi_agent: true,
            feature_remote_plugin: true,
            feature_hooks: true,
            feature_goals: true,
            feature_shell_tool: true,
            max_run_seconds: 10,
            prompt: "push the branch".into(),
            existing_thread_id: None,
            run_token: "art_test".into(),
            session_kind: "project".into(),
            environment: HashMap::new(),
            managed_mcp_servers: Vec::new(),
            approval_handler: Some(handler.clone()),
            progress_handler: None,
            cancellation_handler: None,
        }))
        .expect("fake app-server run");
    assert_eq!(result.status, CodexRunStatus::Succeeded);
    assert_eq!(result.thread_id.as_deref(), Some("thread-approval"));
    assert_eq!(result.final_message.as_deref(), Some("push completed"));
    assert_eq!(handler.calls.load(Ordering::SeqCst), 1);
    std::fs::remove_dir_all(workspace).expect("cleanup");
}

#[cfg(unix)]
#[test]
fn app_server_retries_once_when_it_exits_before_initialize() {
    let workspace = std::env::temp_dir().join(format!(
        "relay-fake-app-server-retry-{}",
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::create_dir_all(&workspace).expect("workspace");
    let script = r#"marker="$PWD/app-server-started"
if [ ! -f "$marker" ]; then
  touch "$marker"
  printf '%s\n' 'temporary app-server startup failure' >&2
  exit 70
fi
IFS= read -r initialize
printf '%s\n' '{"id":0,"result":{"userAgent":"fake","platformFamily":"unix","platformOs":"linux","codexHome":"/tmp"}}'
IFS= read -r initialized
IFS= read -r thread
printf '%s\n' '{"id":1,"result":{"thread":{"id":"thread-retried"},"model":"fake","modelProvider":"fake","cwd":"/tmp","approvalPolicy":"on-request","approvalsReviewer":"user","sandbox":{"type":"workspaceWrite","writableRoots":[],"readOnlyAccess":{"type":"fullAccess"},"networkAccess":true,"excludeTmpdirEnvVar":false,"excludeSlashTmp":false}}}'
IFS= read -r turn
printf '%s\n' '{"id":2,"result":{"turn":{"id":"turn-retried","items":[],"status":"inProgress"}}}'
printf '%s\n' '{"method":"turn/started","params":{"threadId":"thread-retried","turn":{"id":"turn-retried","items":[],"status":"inProgress"}}}'
printf '%s\n' '{"method":"item/completed","params":{"threadId":"thread-retried","turnId":"turn-retried","item":{"id":"message-1","type":"agentMessage","text":"startup recovered"}}}'
printf '%s\n' '{"method":"turn/completed","params":{"threadId":"thread-retried","turn":{"id":"turn-retried","items":[{"id":"message-1","type":"agentMessage","text":"startup recovered"}],"status":"completed"}}}'"#;
    let script_path = workspace.join("fake-app-server-retry.sh");
    std::fs::write(&script_path, script).expect("fake app-server retry script");
    let runner = CodexTriggerRunner::new(
        PathBuf::from("/bin/sh"),
        vec![script_path.to_string_lossy().into_owned()],
        "http://127.0.0.1:8080/mcp".into(),
        "relay_company".into(),
        DEFAULT_RUN_TOKEN_ENV.into(),
    )
    .expect("runner");
    let result = tokio::runtime::Runtime::new()
        .expect("runtime")
        .block_on(runner.run(CodexRunRequest {
            cwd: workspace.clone(),
            codex_profile: "default".into(),
            model: None,
            reasoning_effort: None,
            reasoning_summary: Some("auto".into()),
            verbosity: None,
            personality: None,
            service_tier: None,
            sandbox_mode: "workspace_write".into(),
            approval_policy: "on-request".into(),
            network_access: true,
            web_search: "cached".into(),
            feature_multi_agent: true,
            feature_remote_plugin: true,
            feature_hooks: true,
            feature_goals: true,
            feature_shell_tool: true,
            max_run_seconds: 10,
            prompt: "retry startup".into(),
            existing_thread_id: Some("thread-existing".into()),
            run_token: "art_test".into(),
            session_kind: "project".into(),
            environment: HashMap::new(),
            managed_mcp_servers: Vec::new(),
            approval_handler: Some(Arc::new(AcceptingApprovalHandler::default())),
            progress_handler: None,
            cancellation_handler: None,
        }))
        .expect("app-server startup retry");

    assert_eq!(result.status, CodexRunStatus::Succeeded);
    assert_eq!(result.thread_id.as_deref(), Some("thread-retried"));
    assert_eq!(result.final_message.as_deref(), Some("startup recovered"));
    assert!(result.resumed_existing_session);
    std::fs::remove_dir_all(workspace).expect("cleanup");
}
