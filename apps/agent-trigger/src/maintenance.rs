use super::*;

pub(super) struct BlockingMaintenance {
    next_model_discovery: tokio::time::Instant,
    next_default_auth_discovery: tokio::time::Instant,
    next_mcp_discovery: tokio::time::Instant,
    next_plugin_discovery: tokio::time::Instant,
    next_update_check: tokio::time::Instant,
    job: Option<tokio::task::JoinHandle<BlockingMaintenanceResult>>,
}

#[derive(Default)]
pub(super) struct BlockingMaintenanceResult {
    model_succeeded: Option<bool>,
    auth_succeeded: Option<bool>,
    mcp_succeeded: Option<bool>,
    plugin_succeeded: Option<bool>,
    update_checked: bool,
}

#[derive(Clone, Copy)]
struct DueMaintenance {
    model: bool,
    auth: bool,
    mcp: bool,
    plugins: bool,
    update: bool,
}

impl DueMaintenance {
    fn is_empty(self) -> bool {
        !self.model && !self.auth && !self.mcp && !self.plugins && !self.update
    }
}

impl BlockingMaintenance {
    pub(super) fn new() -> Self {
        let now = tokio::time::Instant::now();
        Self {
            next_model_discovery: now,
            next_default_auth_discovery: now,
            next_mcp_discovery: now,
            next_plugin_discovery: now,
            next_update_check: now,
            job: None,
        }
    }

    pub(super) fn is_running(&self) -> bool {
        self.job.is_some()
    }

    pub(super) fn request_changes(&mut self, changes: discovery::CodexDiscoveryChanges) {
        let now = tokio::time::Instant::now();
        if changes.auth {
            self.next_default_auth_discovery = now;
        }
        if changes.mcp {
            self.next_mcp_discovery = now;
        }
        if changes.models {
            self.next_model_discovery = now;
        }
        if changes.plugins {
            self.next_plugin_discovery = now;
        }
    }

    pub(super) fn request_all_discovery(&mut self) {
        self.request_changes(discovery::CodexDiscoveryChanges {
            auth: true,
            mcp: true,
            models: true,
            plugins: true,
        });
    }

    pub(super) fn abort_for_execution(&mut self) {
        let Some(job) = self.job.take() else {
            return;
        };
        job.abort();
        tracing::debug!("paused Codex discovery maintenance for an execution queue");
    }

    pub(super) fn try_start(
        &mut self,
        allowed: bool,
        platform: &TriggerPlatform,
        codex_control: &CodexControlStore,
        codex_runner: &CodexTriggerRunner,
        config: &TriggerServiceConfig,
    ) {
        if !allowed || self.job.is_some() {
            return;
        }
        let now = tokio::time::Instant::now();
        let due = DueMaintenance {
            model: now >= self.next_model_discovery,
            auth: now >= self.next_default_auth_discovery,
            mcp: now >= self.next_mcp_discovery,
            plugins: now >= self.next_plugin_discovery,
            update: now >= self.next_update_check,
        };
        if due.is_empty() {
            return;
        }
        let platform = platform.clone();
        let codex_control = codex_control.clone();
        let codex_runner = codex_runner.clone();
        let config = config.clone();
        self.job = Some(tokio::spawn(async move {
            run_due_maintenance(&platform, &codex_control, &codex_runner, &config, due).await
        }));
    }

    pub(super) async fn wait(
        &mut self,
    ) -> Result<BlockingMaintenanceResult, tokio::task::JoinError> {
        self.job
            .as_mut()
            .expect("blocking maintenance job is running")
            .await
    }

    pub(super) fn complete(
        &mut self,
        completion: Result<BlockingMaintenanceResult, tokio::task::JoinError>,
        config: &TriggerServiceConfig,
    ) {
        self.job.take();
        let now = tokio::time::Instant::now();
        let result = match completion {
            Ok(result) => result,
            Err(error) => {
                tracing::warn!(%error, "Codex discovery maintenance task stopped unexpectedly");
                let retry = now + StdDuration::from_secs(60);
                self.next_model_discovery = retry;
                self.next_default_auth_discovery = retry;
                self.next_mcp_discovery = retry;
                self.next_plugin_discovery = retry;
                self.next_update_check = retry;
                return;
            }
        };
        if let Some(succeeded) = result.model_succeeded {
            self.next_model_discovery =
                now + discovery::discovery_retry_delay(config.model_discovery_interval, succeeded);
        }
        if let Some(succeeded) = result.auth_succeeded {
            self.next_default_auth_discovery = now
                + discovery::discovery_retry_delay(
                    config.default_auth_discovery_interval,
                    succeeded,
                );
        }
        if let Some(succeeded) = result.mcp_succeeded {
            self.next_mcp_discovery =
                now + discovery::discovery_retry_delay(config.mcp_discovery_interval, succeeded);
        }
        if let Some(succeeded) = result.plugin_succeeded {
            self.next_plugin_discovery =
                now + discovery::discovery_retry_delay(config.plugin_discovery_interval, succeeded);
        }
        if result.update_checked {
            self.next_update_check = now + config.codex_update_check_interval;
        }
    }

    pub(super) fn next_deadline(&self) -> tokio::time::Instant {
        if self.job.is_some() {
            return tokio::time::Instant::now() + StdDuration::from_secs(60);
        }
        [
            self.next_model_discovery,
            self.next_default_auth_discovery,
            self.next_mcp_discovery,
            self.next_plugin_discovery,
            self.next_update_check,
        ]
        .into_iter()
        .min()
        .expect("blocking maintenance deadlines are available")
    }
}

async fn run_due_maintenance(
    platform: &TriggerPlatform,
    codex_control: &CodexControlStore,
    codex_runner: &CodexTriggerRunner,
    config: &TriggerServiceConfig,
    due: DueMaintenance,
) -> BlockingMaintenanceResult {
    let mut result = BlockingMaintenanceResult::default();
    if due.update {
        refresh_codex_latest_version(codex_control, codex_runner, config).await;
        result.update_checked = true;
    }
    if due.model {
        let discovery = refresh_codex_model_catalog(codex_control, codex_runner, config).await;
        result.model_succeeded = Some(discovery.is_ok());
        if let Err(error) = discovery {
            tracing::warn!(
                error = %sanitize_error(&error.to_string()),
                "failed to refresh the local Codex model catalog"
            );
        }
    }
    if due.auth {
        result.auth_succeeded = Some(refresh_codex_default_auth(codex_control, codex_runner).await);
    }
    if due.mcp {
        result.mcp_succeeded = Some(refresh_codex_mcp_catalog(codex_control, codex_runner).await);
    }
    if due.plugins {
        let discovery =
            refresh_codex_plugin_catalogs(platform, codex_control, codex_runner, config).await;
        result.plugin_succeeded = Some(discovery.is_ok());
        if let Err(error) = discovery {
            tracing::warn!(
                error = %sanitize_error(&error.to_string()),
                "failed to refresh the local Codex plugin catalog"
            );
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovery_changes_only_advance_affected_deadlines() {
        let mut maintenance = BlockingMaintenance::new();
        let future = tokio::time::Instant::now() + StdDuration::from_secs(3600);
        maintenance.next_model_discovery = future;
        maintenance.next_default_auth_discovery = future;
        maintenance.next_mcp_discovery = future;
        maintenance.next_plugin_discovery = future;
        maintenance.request_changes(discovery::CodexDiscoveryChanges {
            auth: false,
            mcp: true,
            models: false,
            plugins: true,
        });
        assert_eq!(maintenance.next_model_discovery, future);
        assert_eq!(maintenance.next_default_auth_discovery, future);
        assert!(maintenance.next_mcp_discovery < future);
        assert!(maintenance.next_plugin_discovery < future);
    }
}
