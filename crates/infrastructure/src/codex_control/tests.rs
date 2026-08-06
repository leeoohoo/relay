use std::{fs, path::Path};

use ai_chat_shared::AppError;

use super::*;

fn temporary_store() -> (PathBuf, CodexControlStore) {
    let root = std::env::temp_dir().join(format!("relay-codex-control-{}", Uuid::new_v4()));
    let store = CodexControlStore::new(root.join("codex-control")).expect("store");
    (root, store)
}

#[test]
fn managed_profiles_have_independent_codex_homes() {
    let (_root, store) = temporary_store();
    let first = Uuid::new_v4();
    let second = Uuid::new_v4();
    assert_ne!(
        store.managed_profile_home(first),
        store.managed_profile_home(second)
    );
    assert_eq!(
        store
            .managed_profile_home_for_selector(&managed_profile_selector(first))
            .expect("managed home"),
        store.managed_profile_home(first)
    );
}

#[test]
fn agent_trigger_preferences_use_fallback_until_managed() {
    let (root, store) = temporary_store();
    let preferences = store
        .agent_trigger_preferences(24)
        .expect("fallback preferences");
    assert_eq!(preferences.batch_size, 24);
    assert_eq!(preferences.environment_default, 24);
    assert!(!preferences.managed);
    assert_eq!(preferences.updated_by_human_user_id, None);
    assert_eq!(preferences.updated_at, None);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn agent_trigger_preferences_are_saved_and_reloaded() {
    let (root, store) = temporary_store();
    let human_id = Uuid::new_v4();
    let saved = store
        .save_agent_trigger_preferences(32, human_id, 10)
        .expect("save preferences");
    assert_eq!(saved.batch_size, 32);
    assert!(saved.managed);
    assert_eq!(saved.updated_by_human_user_id, Some(human_id));

    let reloaded = store
        .agent_trigger_preferences(10)
        .expect("reload preferences");
    assert_eq!(reloaded, saved);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn agent_trigger_preferences_reject_out_of_range_values() {
    let (root, store) = temporary_store();
    let human_id = Uuid::new_v4();
    assert!(store
        .save_agent_trigger_preferences(0, human_id, 10)
        .is_err());
    assert!(store
        .save_agent_trigger_preferences(101, human_id, 10)
        .is_err());
    assert!(store.agent_trigger_preferences(0).is_err());
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn api_key_request_is_private_and_never_exposed_by_environment_view() {
    let (root, store) = temporary_store();
    let company_id = Uuid::new_v4();
    let profile = store
        .create_auth_profile(
            company_id,
            "Primary".into(),
            "sk-test-abcdefghijklmnopqrstuvwxyz".into(),
            None,
        )
        .expect("create profile");
    let environment = store
        .environment_for_company(company_id)
        .expect("environment");
    assert_eq!(environment.profiles, vec![profile]);
    assert_eq!(
        environment.profiles[0].base_url.as_deref(),
        Some(CODEX_DEFAULT_OPENAI_BASE_URL)
    );
    let request_path = fs::read_dir(store.requests_dir())
        .expect("requests")
        .next()
        .expect("request")
        .expect("entry")
        .path();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&request_path)
                .expect("metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn codex_base_url_is_normalized_and_rejects_embedded_secrets() {
    assert_eq!(
        normalize_codex_base_url(Some("https://proxy.example.com/v1/")).expect("safe Base URL"),
        "https://proxy.example.com/v1"
    );
    assert_eq!(
        normalize_codex_base_url(None).expect("default Base URL"),
        CODEX_DEFAULT_OPENAI_BASE_URL
    );
    for value in [
        "https://user:secret@example.com/v1",
        "https://example.com/v1?token=secret",
        "https://example.com/v1#secret",
    ] {
        assert!(normalize_codex_base_url(Some(value)).is_err());
    }
}

#[test]
fn update_is_only_available_when_latest_is_newer() {
    let (_root, store) = temporary_store();
    store
        .publish_runtime_probe(
            Some("codex-cli 0.145.0".into()),
            "system",
            Path::new("codex"),
        )
        .expect("runtime");
    let runtime = store
        .publish_latest_version(Some("0.146.0".into()), None)
        .expect("latest");
    assert!(runtime.update_available);
    let runtime = store
        .publish_latest_version(Some("0.145.0".into()), None)
        .expect("latest");
    assert!(!runtime.update_available);
}

#[test]
fn failed_api_key_can_be_replaced_and_the_temporary_request_is_removed() {
    let (root, store) = temporary_store();
    let company_id = Uuid::new_v4();
    let profile = store
        .create_auth_profile(
            company_id,
            "Primary".into(),
            "sk-test-first-abcdefghijklmnopqrstuvwxyz".into(),
            None,
        )
        .expect("create profile");
    let claimed = store.claim_next_request().expect("claim").expect("request");
    store
        .fail_request(claimed, "authentication failed")
        .expect("failure");
    let failed = store
        .environment_for_company(company_id)
        .expect("environment")
        .profiles
        .pop()
        .expect("profile");
    assert_eq!(failed.status, CODEX_AUTH_PROFILE_STATUS_FAILED);
    assert_eq!(
        fs::read_dir(store.processing_dir())
            .expect("processing")
            .count(),
        0
    );

    store
        .update_auth_profile(
            company_id,
            profile.id,
            "Primary".into(),
            Some("sk-test-second-abcdefghijklmnopqrstuvwxyz".into()),
            Some("https://proxy.example.com/v1".into()),
        )
        .expect("retry profile");
    let claimed = store
        .claim_next_request()
        .expect("claim retry")
        .expect("retry request");
    store
        .mark_auth_profile_active(profile.id)
        .expect("activate");
    store.finish_request(claimed).expect("finish");
    let active = store
        .environment_for_company(company_id)
        .expect("environment")
        .profiles
        .pop()
        .expect("profile");
    assert_eq!(active.status, CODEX_AUTH_PROFILE_STATUS_ACTIVE);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn company_cannot_resolve_another_company_profile() {
    let (root, store) = temporary_store();
    let owner_company = Uuid::new_v4();
    let other_company = Uuid::new_v4();
    let profile = store
        .create_auth_profile(
            owner_company,
            "Private".into(),
            "sk-test-company-abcdefghijklmnopqrstuvwxyz".into(),
            None,
        )
        .expect("create profile");
    store
        .mark_auth_profile_active(profile.id)
        .expect("activate");
    assert!(store
        .find_active_company_profile(other_company, &profile.selector)
        .expect("lookup")
        .is_none());
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn mcp_input_accepts_safe_http_and_stdio_configurations() {
    validate_mcp_server_input(&CodexMcpServerInput {
        target_selector: "default".into(),
        name: "docs".into(),
        transport: CODEX_MCP_TRANSPORT_HTTP.into(),
        url: Some("https://example.com/mcp".into()),
        command: None,
        args: Vec::new(),
        bearer_token_env_var: Some("EXAMPLE_MCP_TOKEN".into()),
    })
    .expect("safe HTTP MCP");
    validate_mcp_server_input(&CodexMcpServerInput {
        target_selector: "default".into(),
        name: "local_tools".into(),
        transport: CODEX_MCP_TRANSPORT_STDIO.into(),
        url: None,
        command: Some("npx".into()),
        args: vec!["-y".into(), "@example/mcp".into()],
        bearer_token_env_var: None,
    })
    .expect("safe stdio MCP");
}

#[test]
fn mcp_http_input_rejects_secrets_embedded_in_urls() {
    for url in [
        "https://user:secret@example.com/mcp",
        "https://example.com/mcp?token=secret",
        "https://example.com/mcp#secret",
    ] {
        let result = validate_mcp_server_input(&CodexMcpServerInput {
            target_selector: "default".into(),
            name: "unsafe".into(),
            transport: CODEX_MCP_TRANSPORT_HTTP.into(),
            url: Some(url.into()),
            command: None,
            args: Vec::new(),
            bearer_token_env_var: None,
        });
        assert!(result.is_err(), "URL must be rejected: {url}");
    }
}

#[test]
fn plugin_provided_mcp_server_cannot_be_removed() {
    let (root, store) = temporary_store();
    let company_id = Uuid::new_v4();
    store
        .publish_mcp_snapshot(
            "default",
            vec![CodexMcpServerView {
                name: "plugin-server".into(),
                transport: CODEX_MCP_TRANSPORT_STDIO.into(),
                enabled: true,
                configured_by_user: false,
                ..CodexMcpServerView::default()
            }],
            None,
        )
        .expect("publish snapshot");

    let result = store.enqueue_mcp_remove(company_id, "default".into(), "plugin-server".into());
    assert!(matches!(result, Err(AppError::Conflict(_))));
    fs::remove_dir_all(root).expect("cleanup");
}
