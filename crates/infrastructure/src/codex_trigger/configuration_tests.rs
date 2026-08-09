use super::*;

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
        session_kind: "control".into(),
        environment: HashMap::new(),
        managed_mcp_servers: Vec::new(),
        approval_handler: None,
        progress_handler: None,
        cancellation_handler: None,
    };
    let mut command = Command::new("codex");
    apply_managed_cli_settings(&mut command, &request, 200_000);
    let args = command
        .as_std()
        .get_args()
        .map(|value| value.to_string_lossy().to_string())
        .collect::<Vec<_>>();

    assert!(args.contains(&"model_reasoning_summary=\"concise\"".into()));
    assert!(args.contains(&"model_verbosity=\"medium\"".into()));
    assert!(args.contains(&"service_tier=\"fast\"".into()));
    assert!(args.contains(&"web_search=\"live\"".into()));
    assert!(args.contains(&"model_auto_compact_token_limit=200000".into()));
    assert!(args.contains(&"model_auto_compact_token_limit_scope=\"total\"".into()));
    assert!(args.contains(&"sandbox_workspace_write.network_access=false".into()));
    assert!(args.contains(&"features.remote_plugin=false".into()));
    assert!(args.contains(&"features.shell_tool=true".into()));
}
