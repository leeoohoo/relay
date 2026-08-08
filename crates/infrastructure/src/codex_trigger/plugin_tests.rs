use super::*;

#[test]
fn plugin_operations_map_to_cross_platform_codex_cli_subcommands() {
    assert_eq!(
        super::configuration::plugin_cli_operation("install").expect("install"),
        "add"
    );
    assert_eq!(
        super::configuration::plugin_cli_operation("remove").expect("remove"),
        "remove"
    );
    assert!(super::configuration::plugin_cli_operation("refresh").is_err());
}

#[cfg(unix)]
#[test]
fn plugin_discovery_hides_desktop_only_plugins() {
    let profile_id = Uuid::new_v4();
    let selector = format!("relay_{profile_id}");
    let state_root = std::env::temp_dir().join(format!(
        "relay-plugin-profile-test-{}",
        Uuid::new_v4().simple()
    ));
    let expected_home = state_root
        .join("codex-profiles")
        .join("homes")
        .join(profile_id.to_string());
    let script = format!(
        r#"test "$CODEX_HOME" = '{}' || exit 8; case "$*" in *"marketplace"*) printf '%s\n' '{{"marketplaces":[{{"name":"openai-bundled"}}]}}' ;; *) printf '%s\n' '{{"installed":[],"available":[{{"pluginId":"browser@openai-bundled"}},{{"pluginId":"github@openai-api-curated"}}]}}' ;; esac"#,
        expected_home.display()
    );
    let mut runner = CodexTriggerRunner::new(
        PathBuf::from("/bin/sh"),
        vec!["-c".into(), script, "--".into()],
        "http://127.0.0.1:8080/mcp".into(),
        "relay_company".into(),
        DEFAULT_RUN_TOKEN_ENV.into(),
    )
    .expect("runner");
    runner.managed_profile_homes_root = state_root.join("codex-profiles").join("homes");

    let discovery = tokio::runtime::Runtime::new()
        .expect("runtime")
        .block_on(runner.discover_plugins(&selector))
        .expect("plugin discovery");

    assert_eq!(discovery.available.as_array().map(Vec::len), Some(1));
    assert_eq!(
        discovery.available[0]
            .get("pluginId")
            .and_then(Value::as_str),
        Some("github@openai-api-curated")
    );
    assert_eq!(discovery.marketplaces.as_array().map(Vec::len), Some(1));
}

#[cfg(unix)]
#[test]
fn plugin_operations_use_codex_cli_add_and_remove_subcommands() {
    let script = r#"case "$*" in "plugin add github@openai-api-curated --json") printf '%s\n' '{"installed":true}' ;; "plugin remove github@openai-api-curated --json") printf '%s\n' '{"removed":true}' ;; *) printf '%s\n' "unexpected arguments: $*" >&2; exit 9 ;; esac"#;
    let runner = CodexTriggerRunner::new(
        PathBuf::from("/bin/sh"),
        vec!["-c".into(), script.into(), "--".into()],
        "http://127.0.0.1:8080/mcp".into(),
        "relay_company".into(),
        DEFAULT_RUN_TOKEN_ENV.into(),
    )
    .expect("runner");
    let runtime = tokio::runtime::Runtime::new().expect("runtime");

    let installed = runtime
        .block_on(runner.apply_plugin_operation(
            "default",
            "install",
            Some("github@openai-api-curated"),
        ))
        .expect("plugin install");
    let removed = runtime
        .block_on(runner.apply_plugin_operation(
            "default",
            "remove",
            Some("github@openai-api-curated"),
        ))
        .expect("plugin remove");

    assert_eq!(installed, json!({ "installed": true }));
    assert_eq!(removed, json!({ "removed": true }));
}

#[test]
fn desktop_only_plugins_cannot_be_installed_for_relay_cli_runners() {
    let runner = CodexTriggerRunner::new(
        PathBuf::from("codex"),
        Vec::new(),
        "http://127.0.0.1:8080/mcp".into(),
        "relay_company".into(),
        DEFAULT_RUN_TOKEN_ENV.into(),
    )
    .expect("runner");
    let error = tokio::runtime::Runtime::new()
        .expect("runtime")
        .block_on(runner.apply_plugin_operation(
            "default",
            "install",
            Some("browser@openai-bundled"),
        ))
        .expect_err("desktop plugin must be rejected");
    assert!(error.to_string().contains("Codex desktop host"));
}

#[test]
fn stale_catalogs_are_filtered_before_they_reach_relay_views() {
    let filtered = filter_relay_supported_codex_plugin_items(&json!([
        {"pluginId": "browser@openai-bundled"},
        {"pluginId": "chrome@openai-bundled"},
        {"pluginId": "computer-use@openai-bundled"},
        {"pluginId": "github@openai-api-curated"}
    ]));
    assert_eq!(filtered, json!([{"pluginId": "github@openai-api-curated"}]));
}
