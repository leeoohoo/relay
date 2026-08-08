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
fn managed_browser_profiles_are_isolated_by_company_agent_and_project() {
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
        .managed_browser_mcp_server(company, agent, first_project, &workspace)
        .expect("first browser MCP")
        .expect("enabled browser MCP");
    let second = runner
        .managed_browser_mcp_server(company, agent, second_project, &workspace)
        .expect("second browser MCP")
        .expect("enabled browser MCP");
    let first_mount = first
        .args
        .iter()
        .find(|argument| argument.starts_with("--volume="))
        .expect("first profile mount");
    let second_mount = second
        .args
        .iter()
        .find(|argument| argument.starts_with("--volume="))
        .expect("second profile mount");

    assert_ne!(first_mount, second_mount);
    assert!(first_mount.contains(&first_project.to_string()));
    assert!(second_mount.contains(&second_project.to_string()));
    assert!(first
        .args
        .iter()
        .any(|argument| argument == "--allow-unrestricted-paths"));
    assert!(first
        .args
        .iter()
        .any(|argument| argument == "--add-host=host.docker.internal:host-gateway"));
    assert!(first.args.iter().any(|argument| {
        argument == "--chrome-arg=--host-resolver-rules=MAP localhost host.docker.internal"
    }));
    assert!(first.args.iter().any(|argument| {
        argument.starts_with("--volume=")
            && argument.ends_with(":ro")
            && argument.contains(&canonical_workspace.to_string_lossy().to_string())
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
        .managed_browser_mcp_server(company, agent, project, &workspace)
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
printf '%s\n' '{"id":99,"method":"item/tool/requestUserInput","params":{"threadId":"thread-browser","turnId":"turn-browser","itemId":"browser-item","questions":[{"id":"approval","header":"Website access","question":"Allow navigation?","options":[{"label":"Accept","description":"Continue"},{"label":"Decline","description":"Stop"}]}]}}'
IFS= read -r approval
case "$approval" in *'"answers":{"approval":{"answers":["Accept"]}}'*) ;; *) exit 9 ;; esac
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
        disabled_plugin_ids: vec![
            "browser@openai-bundled".into(),
            "chrome@openai-bundled".into(),
        ],
        required: false,
        startup_timeout_sec: Some(90),
        tool_timeout_sec: Some(180),
        default_tools_approval_mode: "auto".into(),
        tool_approval_modes: BTreeMap::from([("navigate_page".into(), "prompt".into())]),
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
