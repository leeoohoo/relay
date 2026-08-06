use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn model_catalog_excludes_hidden_models_and_nested_ids() {
    let catalog = json!({
        "models": [
            {
                "slug": "gpt-5.6-sol",
                "display_name": "GPT-5.6-Sol",
                "visibility": "list",
                "default_reasoning_level": "low",
                "supported_reasoning_levels": [
                    { "effort": "low", "description": "Fast" },
                    { "effort": "high", "description": "Deep" }
                ],
                "service_tiers": [{ "id": "priority", "name": "Fast" }]
            },
            {
                "slug": "codex-auto-review",
                "display_name": "Codex Auto Review",
                "visibility": "hide"
            }
        ]
    });
    let mut models = BTreeMap::new();

    collect_codex_models(&catalog, &mut models);

    assert_eq!(models.len(), 1);
    let model = models.get("gpt-5.6-sol").expect("selectable model");
    assert_eq!(model.display_name, "GPT-5.6-Sol");
    assert_eq!(model.default_reasoning_effort.as_deref(), Some("low"));
    assert_eq!(
        model
            .reasoning_efforts
            .iter()
            .map(|effort| effort.effort.as_str())
            .collect::<Vec<_>>(),
        vec!["low", "high"]
    );
}

#[test]
fn managed_cli_settings_are_injected_as_cli_overrides() {
    let request = CodexRunRequest {
        cwd: PathBuf::from("/tmp"),
        codex_profile: "default".into(),
        model: Some("gpt-test".into()),
        reasoning_effort: Some("high".into()),
        reasoning_summary: Some("concise".into()),
        verbosity: Some("medium".into()),
        personality: Some("pragmatic".into()),
        service_tier: Some("fast".into()),
        sandbox_mode: "workspace_write".into(),
        approval_policy: "never".into(),
        network_access: false,
        web_search: "live".into(),
        feature_multi_agent: true,
        feature_remote_plugin: false,
        feature_hooks: true,
        feature_goals: false,
        feature_shell_tool: true,
        max_run_seconds: 60,
        prompt: "test".into(),
        existing_thread_id: None,
        run_token: "token".into(),
        environment: HashMap::new(),
        approval_handler: None,
        progress_handler: None,
        cancellation_handler: None,
    };
    let mut command = Command::new("codex");
    apply_managed_cli_settings(&mut command, &request);
    let args = command
        .as_std()
        .get_args()
        .map(|value| value.to_string_lossy().to_string())
        .collect::<Vec<_>>();

    assert!(args.contains(&"model_reasoning_summary=\"concise\"".into()));
    assert!(args.contains(&"model_verbosity=\"medium\"".into()));
    assert!(args.contains(&"service_tier=\"fast\"".into()));
    assert!(args.contains(&"web_search=\"live\"".into()));
    assert!(args.contains(&"sandbox_workspace_write.network_access=false".into()));
    assert!(args.contains(&"features.remote_plugin=false".into()));
    assert!(args.contains(&"features.shell_tool=true".into()));
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
            environment: HashMap::new(),
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
            environment: HashMap::new(),
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
            codex_profile: "relay-test".into(),
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
            environment: HashMap::new(),
            approval_handler: None,
            progress_handler: None,
            cancellation_handler: None,
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
            environment: HashMap::new(),
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
