use std::{
    collections::hash_map::DefaultHasher,
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    time::Duration,
};

use ai_chat_infrastructure::{codex_control::CodexControlStore, codex_trigger::CodexTriggerRunner};
use ai_chat_shared::{now_utc, AppError, AppResult};
use serde::{Deserialize, Serialize};

const MAX_FINGERPRINT_FILE_BYTES: u64 = 2 * 1024 * 1024;
const DISCOVERY_SUCCESS_FILE: &str = "discovery-success.json";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct CodexDiscoveryChanges {
    pub(super) auth: bool,
    pub(super) mcp: bool,
    pub(super) models: bool,
    pub(super) plugins: bool,
}

impl CodexDiscoveryChanges {
    pub(super) const fn is_empty(self) -> bool {
        !self.auth && !self.mcp && !self.models && !self.plugins
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct CodexDiscoveryFingerprint {
    executable: u64,
    auth: u64,
    config: u64,
    plugins: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct CodexDiscoveryDelays {
    pub(super) auth: Duration,
    pub(super) mcp: Duration,
    pub(super) models: Duration,
    pub(super) plugins: Duration,
}

#[derive(Debug, Serialize, Deserialize)]
struct CodexDiscoverySuccess {
    fingerprint: CodexDiscoveryFingerprint,
    completed_at: i64,
}

impl CodexDiscoveryFingerprint {
    pub(super) fn capture(
        control: &CodexControlStore,
        runner: &CodexTriggerRunner,
        configured_profiles: &[String],
    ) -> Self {
        let homes = discovery_homes(control);
        Self::capture_paths(runner.executable_path(), &homes, configured_profiles)
    }

    fn capture_paths(executable: &Path, homes: &[PathBuf], configured_profiles: &[String]) -> Self {
        let executable = fingerprint_paths(vec![executable.to_path_buf()]);
        let auth = fingerprint_paths(
            homes
                .iter()
                .map(|home| home.join("auth.json"))
                .collect::<Vec<_>>(),
        );
        let mut config_paths = homes
            .iter()
            .map(|home| home.join("config.toml"))
            .collect::<Vec<_>>();
        config_paths.extend(homes.iter().flat_map(|home| {
            configured_profiles
                .iter()
                .filter(|profile| profile.as_str() != "default")
                .map(|profile| home.join(format!("{profile}.config.toml")))
        }));
        let config = fingerprint_paths(config_paths);
        let plugins = fingerprint_paths(
            homes
                .iter()
                .flat_map(|home| plugin_marker_paths(home))
                .collect::<Vec<_>>(),
        );
        Self {
            executable,
            auth,
            config,
            plugins,
        }
    }

    pub(super) fn changes_since(self, previous: Self) -> CodexDiscoveryChanges {
        let executable_changed = self.executable != previous.executable;
        let auth_changed = self.auth != previous.auth;
        let config_changed = self.config != previous.config;
        let plugins_changed = self.plugins != previous.plugins;
        CodexDiscoveryChanges {
            auth: executable_changed || auth_changed || config_changed,
            mcp: executable_changed || auth_changed || config_changed || plugins_changed,
            models: executable_changed || auth_changed || config_changed,
            plugins: executable_changed || auth_changed || config_changed || plugins_changed,
        }
    }
}

pub(super) fn discovery_retry_delay(configured: Duration, succeeded: bool) -> Duration {
    if succeeded {
        configured
    } else {
        configured.min(Duration::from_secs(60))
    }
}

pub(super) fn initial_discovery_delays(
    control: &CodexControlStore,
    model_catalog_path: &Path,
    plugin_catalog_available: bool,
    fingerprint: CodexDiscoveryFingerprint,
    configured: CodexDiscoveryDelays,
) -> CodexDiscoveryDelays {
    if !plugin_catalog_available
        || !nonempty_file(model_catalog_path)
        || !nonempty_file(&control.control_root().join("runtime.json"))
        || !nonempty_file(&control.control_root().join("mcp-catalog.json"))
    {
        return CodexDiscoveryDelays::default();
    }
    let Ok(bytes) = fs::read(control.control_root().join(DISCOVERY_SUCCESS_FILE)) else {
        return CodexDiscoveryDelays::default();
    };
    let Ok(success) = serde_json::from_slice::<CodexDiscoverySuccess>(&bytes) else {
        return CodexDiscoveryDelays::default();
    };
    let age_seconds = now_utc().timestamp() - success.completed_at;
    if success.fingerprint != fingerprint || age_seconds < 0 {
        return CodexDiscoveryDelays::default();
    }
    let age = Duration::from_secs(age_seconds as u64);
    CodexDiscoveryDelays {
        auth: configured.auth.saturating_sub(age),
        mcp: configured.mcp.saturating_sub(age),
        models: configured.models.saturating_sub(age),
        plugins: configured.plugins.saturating_sub(age),
    }
}

pub(super) fn record_discovery_success(
    control: &CodexControlStore,
    fingerprint: CodexDiscoveryFingerprint,
) -> AppResult<()> {
    let bytes = serde_json::to_vec(&CodexDiscoverySuccess {
        fingerprint,
        completed_at: now_utc().timestamp(),
    })
    .map_err(|error| AppError::Internal(format!("cannot encode discovery cache: {error}")))?;
    fs::write(control.control_root().join(DISCOVERY_SUCCESS_FILE), bytes)
        .map_err(|error| AppError::Internal(format!("cannot save discovery cache: {error}")))
}

fn nonempty_file(path: &Path) -> bool {
    fs::metadata(path).is_ok_and(|metadata| metadata.is_file() && metadata.len() > 0)
}

fn discovery_homes(control: &CodexControlStore) -> Vec<PathBuf> {
    let mut homes = Vec::new();
    if let Some(default_home) = default_codex_home() {
        homes.push(default_home);
    }
    if let Ok(selectors) = control.list_active_profile_selectors() {
        homes.extend(
            selectors
                .iter()
                .filter_map(|selector| control.managed_profile_home_for_selector(selector)),
        );
    }
    homes.sort();
    homes.dedup();
    homes
}

fn default_codex_home() -> Option<PathBuf> {
    std::env::var_os("CODEX_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .filter(|value| !value.is_empty())
                .map(|home| PathBuf::from(home).join(".codex"))
        })
}

fn plugin_marker_paths(home: &Path) -> Vec<PathBuf> {
    let mut paths = vec![
        home.join(".tmp/plugins.sha"),
        home.join("plugins/.plugin-appserver"),
        home.join("plugins"),
        home.join(".tmp/marketplaces"),
        home.join(".tmp/bundled-marketplaces"),
    ];
    for root in [
        home.join(".tmp/marketplaces"),
        home.join(".tmp/bundled-marketplaces"),
    ] {
        let Ok(entries) = fs::read_dir(root) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            paths.push(path.join(".git/HEAD"));
            paths.push(path.join(".git/index"));
            paths.push(path.join(".agents/plugins/marketplace.json"));
            paths.push(path.join(".claude-plugin/marketplace.json"));
        }
    }
    paths
}

fn fingerprint_paths(paths: Vec<PathBuf>) -> u64 {
    let mut paths = paths;
    paths.sort();
    paths.dedup();
    let mut hasher = DefaultHasher::new();
    for path in paths {
        path.hash(&mut hasher);
        fingerprint_path(&path, &mut hasher);
    }
    hasher.finish()
}

fn fingerprint_path(path: &Path, hasher: &mut DefaultHasher) {
    let Ok(metadata) = fs::metadata(path) else {
        false.hash(hasher);
        return;
    };
    true.hash(hasher);
    metadata.is_dir().hash(hasher);
    metadata.len().hash(hasher);
    metadata.modified().ok().hash(hasher);
    if metadata.is_file() && metadata.len() <= MAX_FINGERPRINT_FILE_BYTES {
        fs::read(path).ok().hash(hasher);
        return;
    }
    if !metadata.is_dir() {
        return;
    }
    let Ok(entries) = fs::read_dir(path) else {
        return;
    };
    let mut entries = entries
        .flatten()
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    entries.sort();
    for entry in entries {
        entry.file_name().hash(hasher);
        if let Ok(metadata) = entry.metadata() {
            metadata.is_dir().hash(hasher);
            metadata.len().hash(hasher);
            metadata.modified().ok().hash(hasher);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn test_root() -> PathBuf {
        std::env::temp_dir().join(format!("relay-discovery-fingerprint-{}", Uuid::new_v4()))
    }

    #[test]
    fn config_changes_refresh_all_config_dependent_catalogs() {
        let root = test_root();
        let home = root.join("codex-home");
        fs::create_dir_all(&home).expect("home");
        let executable = root.join("codex");
        fs::write(&executable, b"binary").expect("executable");
        fs::write(home.join("config.toml"), b"model = 'first'").expect("config");
        let first = CodexDiscoveryFingerprint::capture_paths(
            &executable,
            std::slice::from_ref(&home),
            &["default".into()],
        );
        fs::write(home.join("config.toml"), b"model = 'second'").expect("config update");
        let second = CodexDiscoveryFingerprint::capture_paths(
            &executable,
            std::slice::from_ref(&home),
            &["default".into()],
        );
        let _ = fs::remove_dir_all(root);
        let changes = second.changes_since(first);
        assert!(changes.auth);
        assert!(changes.mcp);
        assert!(changes.models);
        assert!(changes.plugins);
    }

    #[test]
    fn plugin_changes_do_not_force_auth_or_model_discovery() {
        let root = test_root();
        let home = root.join("codex-home");
        fs::create_dir_all(home.join(".tmp")).expect("plugin state");
        let executable = root.join("codex");
        fs::write(&executable, b"binary").expect("executable");
        fs::write(home.join(".tmp/plugins.sha"), b"first").expect("plugin marker");
        let first = CodexDiscoveryFingerprint::capture_paths(
            &executable,
            std::slice::from_ref(&home),
            &["default".into()],
        );
        fs::write(home.join(".tmp/plugins.sha"), b"second").expect("plugin marker update");
        let second = CodexDiscoveryFingerprint::capture_paths(
            &executable,
            std::slice::from_ref(&home),
            &["default".into()],
        );
        let _ = fs::remove_dir_all(root);
        let changes = second.changes_since(first);
        assert!(!changes.auth);
        assert!(changes.mcp);
        assert!(!changes.models);
        assert!(changes.plugins);
    }

    #[test]
    fn failures_retry_within_one_minute() {
        assert_eq!(
            discovery_retry_delay(Duration::from_secs(21_600), false),
            Duration::from_secs(60)
        );
        assert_eq!(
            discovery_retry_delay(Duration::from_secs(21_600), true),
            Duration::from_secs(21_600)
        );
    }

    #[test]
    fn successful_discovery_is_reused_across_restarts_until_inputs_change() {
        let root = test_root();
        let control = CodexControlStore::new(root.join("control")).expect("control store");
        let home = root.join("codex-home");
        fs::create_dir_all(&home).expect("home");
        let executable = root.join("codex");
        let model_catalog = root.join("codex-models.json");
        fs::write(&executable, b"binary").expect("executable");
        fs::write(&model_catalog, b"{}").expect("model catalog");
        fs::write(control.control_root().join("runtime.json"), b"{}").expect("runtime");
        fs::write(control.control_root().join("mcp-catalog.json"), b"{}").expect("mcp catalog");
        let first = CodexDiscoveryFingerprint::capture_paths(
            &executable,
            std::slice::from_ref(&home),
            &["default".into()],
        );
        record_discovery_success(&control, first).expect("discovery cache");
        let configured = CodexDiscoveryDelays {
            auth: Duration::from_secs(300),
            mcp: Duration::from_secs(300),
            models: Duration::from_secs(300),
            plugins: Duration::from_secs(300),
        };
        let cached = initial_discovery_delays(&control, &model_catalog, true, first, configured);
        assert!(cached.mcp > Duration::ZERO);

        fs::write(home.join("config.toml"), b"model = 'changed'").expect("config");
        let changed = CodexDiscoveryFingerprint::capture_paths(
            &executable,
            std::slice::from_ref(&home),
            &["default".into()],
        );
        let due = initial_discovery_delays(&control, &model_catalog, true, changed, configured);
        let _ = fs::remove_dir_all(root);
        assert_eq!(due, CodexDiscoveryDelays::default());
    }
}
