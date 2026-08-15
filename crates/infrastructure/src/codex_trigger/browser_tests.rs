use super::*;
use std::sync::Mutex;

#[derive(Default)]
struct CapturingApprovalHandler {
    requests: Mutex<Vec<CodexApprovalRequest>>,
}

#[async_trait]
impl CodexApprovalHandler for CapturingApprovalHandler {
    async fn request_approval(
        &self,
        request: CodexApprovalRequest,
    ) -> AppResult<CodexApprovalDecision> {
        self.requests
            .lock()
            .expect("approval requests")
            .push(request);
        Ok(CodexApprovalDecision::Accept)
    }
}

#[test]
fn browser_page_state_operations_do_not_require_website_approval() {
    for input in [
        json!({ "type": "reload" }),
        json!({ "type": "back" }),
        json!({ "type": "forward" }),
        json!({}),
        json!({ "url": "about:blank" }),
    ] {
        assert!(!requires_browser_human_approval("navigate_page", &input));
    }
    assert!(!requires_browser_human_approval(
        "new_page",
        &json!({ "url": "about:blank" }),
    ));
    assert!(requires_browser_human_approval(
        "navigate_page",
        &json!({ "url": "https://example.com/dashboard" }),
    ));
    assert!(requires_browser_human_approval(
        "new_page",
        &json!({ "url": "https://example.com/report" }),
    ));
    assert!(requires_browser_human_approval(
        "upload_file",
        &json!({ "filePath": "report.pdf" }),
    ));
}

#[test]
fn docker_fallback_profiles_are_isolated_by_project_and_resource_limited() {
    let root = std::env::temp_dir().join(format!(
        "relay-browser-profile-test-{}",
        Uuid::new_v4().simple()
    ));
    let mut runner = CodexTriggerRunner::new(
        PathBuf::from("codex"),
        Vec::new(),
        "http://127.0.0.1:8080/mcp".into(),
        "relay_company".into(),
        DEFAULT_RUN_TOKEN_ENV.into(),
    )
    .expect("runner");
    runner.browser_mcp = BrowserMcpConfig::for_test(root.clone());
    let company = Uuid::new_v4();
    let agent = Uuid::new_v4();
    let first_project = Uuid::new_v4();
    let second_project = Uuid::new_v4();
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&workspace).expect("workspace");
    let canonical_workspace = workspace.canonicalize().expect("canonical workspace");

    let first = runner
        .managed_browser_mcp_server(company, agent, first_project, &workspace, "run-first")
        .expect("first browser MCP")
        .expect("enabled browser MCP");
    let second = runner
        .managed_browser_mcp_server(company, agent, second_project, &workspace, "run-second")
        .expect("second browser MCP")
        .expect("enabled browser MCP");
    let first_profile_mount = first
        .args
        .iter()
        .find(|argument| {
            argument.starts_with("--volume=")
                && argument.contains(&first_project.to_string())
                && !argument.contains(BROWSER_ARTIFACTS_RELATIVE_PATH)
        })
        .expect("first profile mount");
    let second_profile_mount = second
        .args
        .iter()
        .find(|argument| {
            argument.starts_with("--volume=")
                && argument.contains(&second_project.to_string())
                && !argument.contains(BROWSER_ARTIFACTS_RELATIVE_PATH)
        })
        .expect("second profile mount");

    assert_ne!(first_profile_mount, second_profile_mount);
    assert!(first_profile_mount.contains(&first_project.to_string()));
    assert!(second_profile_mount.contains(&second_project.to_string()));
    assert!(first
        .args
        .iter()
        .any(|argument| argument == "--allow-unrestricted-paths"));
    assert!(first
        .args
        .iter()
        .any(|argument| argument == "--add-host=host.docker.internal:host-gateway"));
    assert!(first.args.iter().any(|argument| {
        argument
            == "--chrome-arg=--host-resolver-rules=MAP 127.0.0.1 host.docker.internal, MAP localhost host.docker.internal"
    }));
    assert!(first
        .args
        .iter()
        .any(|argument| argument == "--chrome-arg=--force-prefers-reduced-motion=reduce"));
    assert!(first.args.iter().any(|argument| argument == "--cpus=1.0"));
    assert!(first
        .args
        .iter()
        .any(|argument| argument == "--memory=768m"));
    assert!(first
        .args
        .iter()
        .any(|argument| argument == "--memory-swap=768m"));
    assert_eq!(first.default_tools_approval_mode, "approve");
    assert!(first.args.iter().any(|argument| {
        argument.starts_with("--volume=")
            && argument.ends_with(":ro")
            && argument.contains(&canonical_workspace.to_string_lossy().to_string())
    }));
    let canonical_artifacts = canonical_workspace.join(BROWSER_ARTIFACTS_RELATIVE_PATH);
    assert!(canonical_artifacts.is_dir());
    assert!(first.args.iter().any(|argument| {
        argument.starts_with("--volume=")
            && argument.ends_with(":rw")
            && argument.contains(&canonical_artifacts.to_string_lossy().to_string())
    }));
    assert!(first.args.iter().any(|argument| {
        argument == &format!("--workdir={}", canonical_workspace.to_string_lossy())
    }));
    assert_eq!(
        first
            .tool_approval_modes
            .get("navigate_page")
            .map(String::as_str),
        Some("prompt")
    );
    std::fs::remove_dir_all(root).expect("cleanup browser profiles");
}

#[test]
fn managed_browser_profile_removes_stale_chromium_runtime_files() {
    let root = std::env::temp_dir().join(format!(
        "relay-browser-stale-profile-test-{}",
        Uuid::new_v4().simple()
    ));
    let company = Uuid::new_v4();
    let agent = Uuid::new_v4();
    let project = Uuid::new_v4();
    let profile = root
        .join(company.to_string())
        .join(agent.to_string())
        .join(project.to_string());
    std::fs::create_dir_all(&profile).expect("profile");
    for name in CHROMIUM_RUNTIME_FILES {
        std::fs::write(profile.join(name), "stale").expect("stale Chromium runtime file");
    }
    std::fs::write(profile.join("Local State"), "persistent").expect("persistent profile file");
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&workspace).expect("workspace");
    let mut runner = CodexTriggerRunner::new(
        PathBuf::from("codex"),
        Vec::new(),
        "http://127.0.0.1:8080/mcp".into(),
        "relay_company".into(),
        DEFAULT_RUN_TOKEN_ENV.into(),
    )
    .expect("runner");
    runner.browser_mcp = BrowserMcpConfig::for_test(root.clone());

    runner
        .managed_browser_mcp_server(company, agent, project, &workspace, "run-cleanup")
        .expect("browser MCP")
        .expect("enabled browser MCP");

    for name in CHROMIUM_RUNTIME_FILES {
        assert!(!profile.join(name).exists(), "{name} should be removed");
    }
    assert_eq!(
        std::fs::read_to_string(profile.join("Local State")).expect("persistent profile file"),
        "persistent"
    );
    std::fs::remove_dir_all(root).expect("cleanup browser profiles");
}

#[test]
fn host_browser_mcp_connects_to_the_runner_pool_with_page_routing() {
    let args = host_mcp_args(Path::new("npx"), "http://127.0.0.1:19222");
    assert!(args
        .iter()
        .any(|argument| argument == "--browserUrl=http://127.0.0.1:19222"));
    assert!(args
        .iter()
        .any(|argument| argument == "--experimentalPageIdRouting"));
    assert!(args
        .iter()
        .any(|argument| argument == "--allowUnrestrictedPaths"));
    assert!(!args.iter().any(|argument| argument == "--headless"));

    let direct_args = host_mcp_args(
        Path::new("/opt/relay/chrome-devtools-mcp"),
        "http://127.0.0.1:19222",
    );
    assert!(!direct_args.iter().any(|argument| argument == "--yes"));
    assert!(!direct_args
        .iter()
        .any(|argument| argument == DEFAULT_BROWSER_MCP_PACKAGE));
}

#[test]
fn host_browser_is_shared_by_the_trigger_with_one_tab_per_agent() {
    let root = std::env::temp_dir().join(format!(
        "relay-host-browser-pool-test-{}",
        Uuid::new_v4().simple()
    ));
    let Some(config) = BrowserMcpConfig::for_test_host(root.clone()) else {
        return;
    };
    let mut runner = CodexTriggerRunner::new(
        PathBuf::from("codex"),
        Vec::new(),
        "http://127.0.0.1:8080/mcp".into(),
        "relay_company".into(),
        DEFAULT_RUN_TOKEN_ENV.into(),
    )
    .expect("runner");
    runner.browser_mcp = config;
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&workspace).expect("workspace");
    let company = Uuid::new_v4();
    let agent = Uuid::new_v4();

    let first = runner
        .managed_browser_mcp_server(company, agent, Uuid::new_v4(), &workspace, "run-first")
        .expect("first host browser")
        .expect("enabled host browser");
    let second = runner
        .managed_browser_mcp_server(company, agent, Uuid::new_v4(), &workspace, "run-second")
        .expect("second host browser")
        .expect("enabled host browser");
    let other_agent = runner
        .managed_browser_mcp_server(
            company,
            Uuid::new_v4(),
            Uuid::new_v4(),
            &workspace,
            "run-other-agent",
        )
        .expect("other Agent host browser")
        .expect("enabled host browser");
    let same_agent_other_company = runner
        .managed_browser_mcp_server(
            Uuid::new_v4(),
            agent,
            Uuid::new_v4(),
            &workspace,
            "run-same-agent",
        )
        .expect("same Agent in another company host browser")
        .expect("enabled host browser");
    let other_company = runner
        .managed_browser_mcp_server(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            &workspace,
            "run-other-company",
        )
        .expect("other company host browser")
        .expect("enabled host browser");
    let first_url = first.url.as_deref().expect("first browser proxy URL");
    assert_eq!(first.url, second.url);
    assert_eq!(first.url, other_agent.url);
    assert_eq!(first.url, same_agent_other_company.url);
    assert_eq!(first.url, other_company.url);
    assert!(first_url.starts_with("http://127.0.0.1:"));
    assert!(first_url.ends_with("/mcp"));
    assert_eq!(first.prompt_hint, second.prompt_hint);
    assert_eq!(first.prompt_hint, same_agent_other_company.prompt_hint);
    assert_eq!(first.prompt_hint, other_agent.prompt_hint);
    assert_eq!(other_agent.prompt_hint, other_company.prompt_hint);
    assert!(first.command.is_empty());
    assert!(first.args.is_empty());
    assert!(first.env.is_empty());
    assert_eq!(
        first.env_http_headers.get("x-agent-run-token"),
        Some(&DEFAULT_RUN_TOKEN_ENV.to_string())
    );
    assert_eq!(first.command, second.command);
    drop(runner);
    std::fs::remove_dir_all(root).expect("cleanup host browser profiles");
}

#[test]
fn shared_browser_proxy_isolates_agent_pages_and_tokens() {
    let root = std::env::temp_dir().join(format!(
        "relay-host-browser-proxy-test-{}",
        Uuid::new_v4().simple()
    ));
    let Some(config) = BrowserMcpConfig::for_test_host(root.clone()) else {
        return;
    };
    let mut runner = CodexTriggerRunner::new(
        PathBuf::from("codex"),
        Vec::new(),
        "http://127.0.0.1:8080/mcp".into(),
        "relay_company".into(),
        DEFAULT_RUN_TOKEN_ENV.into(),
    )
    .expect("runner");
    runner.browser_mcp = config;
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&workspace).expect("workspace");
    let company = Uuid::new_v4();
    let project = Uuid::new_v4();
    let first_agent = Uuid::new_v4();
    let second_agent = Uuid::new_v4();
    let first = runner
        .managed_browser_mcp_server(company, first_agent, project, &workspace, "proxy-run-first")
        .expect("first browser MCP")
        .expect("enabled first browser MCP");
    let second = runner
        .managed_browser_mcp_server(
            company,
            second_agent,
            project,
            &workspace,
            "proxy-run-second",
        )
        .expect("second browser MCP")
        .expect("enabled second browser MCP");
    let url = first.url.as_deref().expect("browser proxy URL");
    assert_eq!(first.url, second.url);

    for token in ["proxy-run-first", "proxy-run-second"] {
        browser_proxy_rpc(
            url,
            token,
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "protocolVersion": "2025-11-25",
                    "capabilities": {},
                    "clientInfo": {"name": "relay-test", "version": "1"}
                }
            }),
        );
    }
    let tools = browser_proxy_rpc(
        url,
        "proxy-run-first",
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}}),
    );
    let tools = tools["result"]["tools"]
        .as_array()
        .expect("browser proxy tools");
    assert!(!tools
        .iter()
        .any(|tool| { matches!(tool["name"].as_str(), Some("select_page" | "close_page")) }));
    assert!(tools
        .iter()
        .all(|tool| { tool["inputSchema"]["properties"].get("pageId").is_none() }));

    let first_pages = browser_proxy_tool_call(url, "proxy-run-first", 3, "list_pages", json!({}));
    let second_pages = browser_proxy_tool_call(url, "proxy-run-second", 4, "list_pages", json!({}));
    assert!(
        first_pages["result"]["structuredContent"]["pages"][0]["url"].is_string(),
        "unexpected first Agent page response: {first_pages:#}"
    );
    assert!(
        second_pages["result"]["structuredContent"]["pages"][0]["url"].is_string(),
        "unexpected second Agent page response: {second_pages:#}"
    );
    assert_eq!(
        first_pages["result"]["structuredContent"]["pages"][0]["url"],
        format!("about:blank#relay-agent-{first_agent}")
    );
    assert_eq!(
        second_pages["result"]["structuredContent"]["pages"][0]["url"],
        format!("about:blank#relay-agent-{second_agent}")
    );
    assert_eq!(
        first_pages["result"]["structuredContent"]["pages"]
            .as_array()
            .expect("first Agent pages")
            .len(),
        1
    );
    assert_eq!(
        second_pages["result"]["structuredContent"]["pages"]
            .as_array()
            .expect("second Agent pages")
            .len(),
        1
    );
    let browser_urls = runner
        .browser_mcp
        .host_page_urls_for_test()
        .expect("shared browser pages");
    assert_eq!(
        browser_urls.len(),
        2,
        "unexpected browser pages: {browser_urls:#?}"
    );

    let navigate = browser_proxy_tool_call(
        url,
        "proxy-run-first",
        5,
        "new_page",
        json!({"url": "about:blank"}),
    );
    assert_ne!(navigate["result"]["isError"], true, "{navigate:#}");
    let first_after_navigation =
        browser_proxy_tool_call(url, "proxy-run-first", 6, "list_pages", json!({}));
    let second_after_navigation =
        browser_proxy_tool_call(url, "proxy-run-second", 7, "list_pages", json!({}));
    assert_eq!(
        first_after_navigation["result"]["structuredContent"]["pages"][0]["url"],
        "about:blank"
    );
    assert_eq!(
        second_after_navigation["result"]["structuredContent"]["pages"][0]["url"],
        format!("about:blank#relay-agent-{second_agent}")
    );
    let browser_urls = runner
        .browser_mcp
        .host_page_urls_for_test()
        .expect("shared browser pages after navigation");
    assert_eq!(
        browser_urls.len(),
        2,
        "new_page created a tab: {browser_urls:#?}"
    );

    let hidden = browser_proxy_tool_call(
        url,
        "proxy-run-first",
        8,
        "select_page",
        json!({"pageId": 999}),
    );
    assert_eq!(hidden["result"]["isError"], true);
    let unauthorized = browser_proxy_rpc(
        url,
        "invalid-run-token",
        json!({"jsonrpc": "2.0", "id": 9, "method": "tools/list", "params": {}}),
    );
    assert!(unauthorized.get("error").is_some());

    drop(runner);
    std::fs::remove_dir_all(root).expect("cleanup host browser profiles");
}

fn browser_proxy_tool_call(url: &str, token: &str, id: u64, name: &str, arguments: Value) -> Value {
    browser_proxy_rpc(
        url,
        token,
        json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/call",
            "params": {"name": name, "arguments": arguments}
        }),
    )
}

fn browser_proxy_rpc(url: &str, token: &str, body: Value) -> Value {
    let response = reqwest::blocking::Client::new()
        .post(url)
        .header("accept", "application/json, text/event-stream")
        .header("content-type", "application/json")
        .header("mcp-protocol-version", "2025-11-25")
        .header("x-agent-run-token", token)
        .json(&body)
        .send()
        .expect("browser proxy request");
    let status = response.status();
    let body = response.text().expect("browser proxy response body");
    assert!(
        status.is_success(),
        "browser proxy returned {status}: {body}"
    );
    serde_json::from_str(&body).expect("browser proxy JSON response")
}

#[cfg(unix)]
#[test]
fn browser_navigation_approval_continues_the_same_app_server_turn() {
    let workspace = std::env::temp_dir().join(format!(
        "relay-browser-approval-test-{}",
        Uuid::new_v4().simple()
    ));
    std::fs::create_dir_all(&workspace).expect("workspace");
    let codex_home = workspace.join("codex-home");
    std::fs::create_dir_all(&codex_home).expect("codex home");
    let script = r#"case "$*" in *'mcp_servers.chrome-devtools.tools.navigate_page.approval_mode="prompt"'*) ;; *) exit 7 ;; esac
case "$*" in *'approval_policy={ granular = { sandbox_approval = false, rules = false, skill_approval = false, request_permissions = false, mcp_elicitations = true } }'*) ;; *) exit 19 ;; esac
case "$*" in *'approvals_reviewer="user"'*) ;; *) exit 20 ;; esac
case "$*" in *'--profile'*) exit 8 ;; esac
case "$*" in *'plugins."browser@openai-bundled".enabled=false'*) ;; *) exit 11 ;; esac
case "$*" in *'plugins."chrome@openai-bundled".enabled=false'*) ;; *) exit 12 ;; esac
case "$*" in *'plugins."computer-use@openai-bundled".enabled=false'*) ;; *) exit 13 ;; esac
case "$*" in *'plugins."visualize@openai-bundled".enabled=false'*) ;; *) exit 14 ;; esac
case "$*" in *'plugins."documents@openai-primary-runtime".enabled=false'*) ;; *) exit 15 ;; esac
IFS= read -r initialize
case "$initialize" in *'"mcpServerOpenaiFormElicitation":true'*) ;; *) exit 16 ;; esac
printf '%s\n' '{"id":0,"result":{"userAgent":"fake","platformFamily":"unix","platformOs":"linux","codexHome":"/tmp"}}'
IFS= read -r initialized
IFS= read -r thread
case "$thread" in *'"approvalPolicy":{"granular":{"mcp_elicitations":true,"request_permissions":false,"rules":false,"sandbox_approval":false,"skill_approval":false}}'*) ;; *) exit 17 ;; esac
printf '%s\n' '{"id":1,"result":{"thread":{"id":"thread-browser"},"model":"fake","modelProvider":"fake","cwd":"/tmp","approvalPolicy":"never","approvalsReviewer":"user","sandbox":{"type":"workspaceWrite","writableRoots":[],"readOnlyAccess":{"type":"fullAccess"},"networkAccess":true,"excludeTmpdirEnvVar":false,"excludeSlashTmp":false}}}'
IFS= read -r turn
case "$turn" in *'"approvalPolicy":{"granular":{"mcp_elicitations":true,"request_permissions":false,"rules":false,"sandbox_approval":false,"skill_approval":false}}'*) ;; *) exit 18 ;; esac
printf '%s\n' '{"id":2,"result":{"turn":{"id":"turn-browser","items":[],"status":"inProgress"}}}'
printf '%s\n' '{"method":"item/started","params":{"threadId":"thread-browser","turnId":"turn-browser","item":{"id":"browser-item","type":"mcpToolCall","server":"chrome-devtools","tool":"navigate_page","arguments":{"url":"https://example.com/dashboard"},"status":"inProgress"}}}'
printf '%s\n' '{"id":99,"method":"mcpServer/elicitation/request","params":{"threadId":"thread-browser","turnId":"turn-browser","serverName":"chrome-devtools","mode":"form","_meta":{"codex_approval_kind":"mcp_tool_call","tool_params":{"url":"https://example.com/dashboard"}},"message":"Allow the chrome-devtools MCP server to run tool navigate_page?","requestedSchema":{"type":"object","properties":{}}}}'
IFS= read -r approval
case "$approval" in *'"action":"accept","content":{}'*) ;; *) exit 9 ;; esac
printf '%s\n' '{"method":"item/completed","params":{"threadId":"thread-browser","turnId":"turn-browser","item":{"id":"browser-item","type":"mcpToolCall","server":"chrome-devtools","tool":"navigate_page","arguments":{"url":"https://example.com/dashboard"},"status":"completed"}}}'
printf '%s\n' '{"method":"item/completed","params":{"threadId":"thread-browser","turnId":"turn-browser","item":{"id":"message-1","type":"agentMessage","text":"browser completed"}}}'
printf '%s\n' '{"method":"turn/completed","params":{"threadId":"thread-browser","turn":{"id":"turn-browser","items":[{"id":"message-1","type":"agentMessage","text":"browser completed"}],"status":"completed"}}}'"#;
    let script_path = workspace.join("fake-browser-app-server.sh");
    std::fs::write(&script_path, script).expect("fake browser app-server");
    let mut runner = CodexTriggerRunner::new(
        PathBuf::from("/bin/sh"),
        vec![script_path.to_string_lossy().into_owned()],
        "http://127.0.0.1:8080/mcp".into(),
        "relay_company".into(),
        DEFAULT_RUN_TOKEN_ENV.into(),
    )
    .expect("runner");
    runner.inherited_environment.insert(
        "CODEX_HOME".into(),
        codex_home.to_string_lossy().into_owned(),
    );
    let handler = Arc::new(CapturingApprovalHandler::default());
    let server = ManagedCodexMcpServer {
        name: MANAGED_BROWSER_MCP_NAME.into(),
        command: "docker".into(),
        args: vec!["run".into(), "browser".into()],
        env: BTreeMap::new(),
        url: None,
        env_http_headers: BTreeMap::new(),
        disabled_plugin_ids: vec![
            "browser@openai-bundled".into(),
            "chrome@openai-bundled".into(),
        ],
        required: false,
        startup_timeout_sec: Some(90),
        tool_timeout_sec: Some(180),
        default_tools_approval_mode: "auto".into(),
        tool_approval_modes: BTreeMap::from([("navigate_page".into(), "prompt".into())]),
        prompt_hint: None,
    };
    let result = tokio::runtime::Runtime::new()
        .expect("runtime")
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
            prompt: "open the dashboard".into(),
            existing_thread_id: None,
            run_token: "art_test".into(),
            session_kind: "project".into(),
            environment: HashMap::new(),
            managed_mcp_servers: vec![server],
            approval_handler: Some(handler.clone()),
            progress_handler: None,
            cancellation_handler: None,
        }))
        .expect("browser app-server run");

    assert_eq!(result.status, CodexRunStatus::Succeeded);
    let requests = handler.requests.lock().expect("approval requests");
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].tool_name,
        AGENT_CODEX_APPROVAL_TOOL_WEBSITE_ACCESS
    );
    assert_eq!(
        requests[0].arguments.get("url").and_then(Value::as_str),
        Some("https://example.com/dashboard")
    );
    drop(requests);
    std::fs::remove_dir_all(workspace).expect("cleanup workspace");
}

#[test]
fn new_mcp_elicitation_matches_the_active_browser_call_by_arguments() {
    let first = json!({
        "id": "first",
        "type": "mcpToolCall",
        "server": "chrome-devtools",
        "tool": "new_page",
        "arguments": { "url": "https://example.com/first" }
    });
    let second = json!({
        "id": "second",
        "type": "mcpToolCall",
        "server": "chrome-devtools",
        "tool": "new_page",
        "arguments": { "url": "https://example.com/second" }
    });
    let active = HashMap::from([("first".into(), first), ("second".into(), second)]);
    let request = json!({
        "method": "mcpServer/elicitation/request",
        "params": {
            "serverName": "chrome-devtools",
            "mode": "form",
            "_meta": {
                "codex_approval_kind": "mcp_tool_call",
                "tool_params": { "url": "https://example.com/second" }
            }
        }
    });

    let matched = active_mcp_item_for_request(&request, &active).expect("matching MCP call");
    assert_eq!(matched.get("id").and_then(Value::as_str), Some("second"));
}

#[test]
fn new_mcp_elicitation_response_uses_codex_v2_shape() {
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let mut accepted = Vec::new();
    runtime
        .block_on(send_approval_response(
            &mut accepted,
            json!(99),
            "mcpServer/elicitation/request",
            &json!({}),
            CodexApprovalDecision::Accept,
        ))
        .expect("accept response");
    assert_eq!(
        serde_json::from_slice::<Value>(&accepted).expect("accept JSON"),
        json!({
            "id": 99,
            "result": { "action": "accept", "content": {}, "_meta": null }
        })
    );

    let mut declined = Vec::new();
    runtime
        .block_on(send_approval_response(
            &mut declined,
            json!(100),
            "mcpServer/elicitation/request",
            &json!({}),
            CodexApprovalDecision::Decline,
        ))
        .expect("decline response");
    assert_eq!(
        serde_json::from_slice::<Value>(&declined).expect("decline JSON"),
        json!({
            "id": 100,
            "result": { "action": "decline", "content": null, "_meta": null }
        })
    );
}

#[test]
fn managed_browser_follow_up_tools_are_accepted_without_another_human_approval() {
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let handler = CapturingApprovalHandler::default();
    let item = json!({
        "id": "snapshot-item",
        "type": "mcpToolCall",
        "server": "chrome-devtools",
        "tool": "take_snapshot",
        "arguments": {}
    });
    let params = json!({
        "serverName": "chrome-devtools",
        "mode": "form",
        "_meta": {
            "codex_approval_kind": "mcp_tool_call",
            "tool_params": {}
        }
    });
    let mut response = Vec::new();

    runtime
        .block_on(handle_browser_tool_approval(
            &mut response,
            json!(101),
            "mcpServer/elicitation/request",
            &params,
            Some(&item),
            &handler,
        ))
        .expect("follow-up browser approval response");

    assert_eq!(
        serde_json::from_slice::<Value>(&response).expect("response JSON"),
        json!({
            "id": 101,
            "result": { "action": "accept", "content": {}, "_meta": null }
        })
    );
    assert!(handler
        .requests
        .lock()
        .expect("approval requests")
        .is_empty());
}

#[test]
fn managed_browser_follow_up_approval_survives_missing_item_started_event() {
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let handler = CapturingApprovalHandler::default();
    let params = json!({
        "serverName": "chrome-devtools",
        "mode": "form",
        "message": "Allow the chrome-devtools MCP server to run tool evaluate_script?",
        "_meta": {
            "codex_approval_kind": "mcp_tool_call",
            "tool_params": { "function": "() => document.title" }
        }
    });
    let mut response = Vec::new();

    runtime
        .block_on(handle_browser_tool_approval(
            &mut response,
            json!(102),
            "mcpServer/elicitation/request",
            &params,
            None,
            &handler,
        ))
        .expect("follow-up browser approval response");

    assert_eq!(
        serde_json::from_slice::<Value>(&response).expect("response JSON"),
        json!({
            "id": 102,
            "result": { "action": "accept", "content": {}, "_meta": null }
        })
    );
    assert!(handler
        .requests
        .lock()
        .expect("approval requests")
        .is_empty());
}

#[test]
fn active_managed_browser_call_keeps_one_absolute_stall_deadline() {
    let item = json!({
        "id": "browser-stall",
        "type": "mcpToolCall",
        "server": MANAGED_BROWSER_MCP_NAME,
        "tool": "new_page",
        "arguments": { "url": "http://localhost:13000/" }
    });
    let active = HashMap::from([("browser-stall".into(), item)]);
    let started = HashMap::from([(
        "browser-stall".into(),
        Instant::now() - Duration::from_secs(190),
    )]);

    let (item_id, tool, remaining) =
        active_managed_browser_timeout(&active, &started, Duration::from_secs(195))
            .expect("active browser timeout");

    assert_eq!(item_id, "browser-stall");
    assert_eq!(tool, "new_page");
    assert!(remaining <= Duration::from_secs(5));
}
