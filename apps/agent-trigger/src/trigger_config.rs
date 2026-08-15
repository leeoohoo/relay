use super::*;

impl TriggerServiceConfig {
    pub(super) fn from_env() -> AppResult<Self> {
        let instance = std::env::var("AGENT_TRIGGER_INSTANCE_ID")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| Uuid::new_v4().simple().to_string());
        if instance.chars().count() > 80 || instance.chars().any(char::is_control) {
            return Err(AppError::Validation(
                "AGENT_TRIGGER_INSTANCE_ID is invalid".into(),
            ));
        }
        let host = whoami::fallible::hostname().unwrap_or_else(|_| "unknown-host".into());
        let os_user = std::env::var("USER")
            .or_else(|_| std::env::var("USERNAME"))
            .unwrap_or_else(|_| "unknown-user".into());
        let plugin_host_id = std::env::var("AGENT_TRIGGER_PLUGIN_HOST_ID")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| format!("{host}:{os_user}"));
        if plugin_host_id.chars().count() > 160 || plugin_host_id.chars().any(char::is_control) {
            return Err(AppError::Validation(
                "AGENT_TRIGGER_PLUGIN_HOST_ID is invalid".into(),
            ));
        }
        let poll_interval_seconds = std::env::var("AGENT_TRIGGER_POLL_INTERVAL_SECONDS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .map(|value| value.clamp(10, 300))
            .unwrap_or(60);
        let run_heartbeat_stale_after_seconds =
            std::env::var("AGENT_TRIGGER_RUN_HEARTBEAT_STALE_SECONDS")
                .ok()
                .and_then(|value| value.parse::<i64>().ok())
                .map(|value| value.clamp(60, 900))
                .unwrap_or(180);
        let batch_size = agent_trigger_batch_size_from_env();
        let logical_cpus = logical_cpu_count();
        let resource_concurrency_limit = std::env::var("AGENT_TRIGGER_RESOURCE_CONCURRENCY_LIMIT")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .map(|value| value.clamp(1, 32))
            .unwrap_or_else(default_resource_concurrency_limit);
        let minimum_available_memory_bytes = minimum_available_memory_bytes_from_env();
        let run_once = bool_env("AGENT_TRIGGER_RUN_ONCE", false);
        let model_catalog_path = std::env::var("AGENT_TRIGGER_MODEL_CATALOG_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(".relay-agent-trigger/codex-models.json"));
        let model_discovery_profiles = std::env::var("AGENT_TRIGGER_MODEL_DISCOVERY_PROFILES")
            .ok()
            .map(|value| {
                value
                    .split(',')
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .filter(|profiles| !profiles.is_empty())
            .unwrap_or_else(|| vec!["default".into()]);
        let model_discovery_interval = StdDuration::from_secs(
            std::env::var("AGENT_TRIGGER_MODEL_DISCOVERY_INTERVAL_SECONDS")
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
                .map(|value| value.clamp(60, 86_400))
                .unwrap_or(21_600),
        );
        let discovery_fingerprint_interval = StdDuration::from_secs(
            std::env::var("AGENT_TRIGGER_DISCOVERY_FINGERPRINT_INTERVAL_SECONDS")
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
                .map(|value| value.clamp(5, 300))
                .unwrap_or(30),
        );
        let default_auth_discovery_interval = StdDuration::from_secs(
            std::env::var("AGENT_TRIGGER_CODEX_AUTH_DISCOVERY_INTERVAL_SECONDS")
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
                .map(|value| value.clamp(15, 86_400))
                .unwrap_or(21_600),
        );
        let mcp_discovery_interval = StdDuration::from_secs(
            std::env::var("AGENT_TRIGGER_MCP_DISCOVERY_INTERVAL_SECONDS")
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
                .map(|value| value.clamp(15, 86_400))
                .unwrap_or(21_600),
        );
        let plugin_discovery_interval = StdDuration::from_secs(
            std::env::var("AGENT_TRIGGER_PLUGIN_DISCOVERY_INTERVAL_SECONDS")
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
                .map(|value| value.clamp(30, 86_400))
                .unwrap_or(21_600),
        );
        let codex_auto_install = bool_env("AGENT_TRIGGER_CODEX_AUTO_INSTALL", false);
        let codex_install_url = std::env::var("AGENT_TRIGGER_CODEX_INSTALL_URL")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| default_codex_install_url(std::env::consts::OS).into());
        if !codex_install_url.starts_with("https://") {
            return Err(AppError::Validation(
                "AGENT_TRIGGER_CODEX_INSTALL_URL must use https".into(),
            ));
        }
        let codex_update_registry_url = std::env::var("AGENT_TRIGGER_CODEX_UPDATE_REGISTRY_URL")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "https://registry.npmjs.org/@openai%2Fcodex/latest".into());
        if !codex_update_registry_url.starts_with("https://") {
            return Err(AppError::Validation(
                "AGENT_TRIGGER_CODEX_UPDATE_REGISTRY_URL must use https".into(),
            ));
        }
        let codex_update_check_interval = StdDuration::from_secs(
            std::env::var("AGENT_TRIGGER_CODEX_UPDATE_CHECK_INTERVAL_SECONDS")
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
                .map(|value| value.clamp(300, 86_400))
                .unwrap_or(3_600),
        );
        Ok(Self {
            lease_owner: format!("{host}:{instance}"),
            plugin_host_id,
            hostname: host,
            poll_interval: StdDuration::from_secs(poll_interval_seconds),
            run_heartbeat_stale_after_seconds,
            batch_size,
            resource_concurrency_limit,
            logical_cpus,
            minimum_available_memory_bytes,
            run_once,
            model_catalog_path,
            model_discovery_profiles,
            discovery_fingerprint_interval,
            model_discovery_interval,
            default_auth_discovery_interval,
            mcp_discovery_interval,
            plugin_discovery_interval,
            codex_auto_install,
            codex_install_url,
            codex_update_registry_url,
            codex_update_check_interval,
        })
    }
}
