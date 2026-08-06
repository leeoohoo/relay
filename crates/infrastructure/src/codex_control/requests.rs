use std::{fs, path::Path};

use ai_chat_shared::{now_utc, AppError, AppResult};

use super::storage::{file_error, read_json, truncate};
use super::validation::versions_show_update;
use super::*;

impl CodexControlStore {
    pub fn claim_next_request(&self) -> AppResult<Option<ClaimedCodexControlRequest>> {
        let _lock = self.lock()?;
        self.recover_stale_processing_unlocked()?;
        let mut requests = fs::read_dir(self.requests_dir())
            .map_err(file_error)?
            .filter_map(Result::ok)
            .filter(|entry| {
                entry.path().extension().and_then(|value| value.to_str()) == Some("json")
            })
            .filter_map(|entry| {
                let request = read_json::<CodexControlRequest>(&entry.path()).ok()?;
                Some((request.created_at, entry.path(), request))
            })
            .collect::<Vec<_>>();
        requests.sort_by_key(|(created_at, _, _)| *created_at);
        let Some((_, queued_path, request)) = requests.into_iter().next() else {
            return Ok(None);
        };
        let processing_path = self.processing_dir().join(format!("{}.json", request.id));
        fs::rename(&queued_path, &processing_path).map_err(file_error)?;
        Ok(Some(ClaimedCodexControlRequest {
            request,
            path: processing_path,
        }))
    }

    fn recover_stale_processing_unlocked(&self) -> AppResult<()> {
        let stale_before = std::time::SystemTime::now()
            .checked_sub(std::time::Duration::from_secs(15 * 60))
            .unwrap_or(std::time::UNIX_EPOCH);
        for entry in fs::read_dir(self.processing_dir()).map_err(file_error)? {
            let entry = entry.map_err(file_error)?;
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            let modified = entry
                .metadata()
                .and_then(|metadata| metadata.modified())
                .map_err(file_error)?;
            if modified <= stale_before {
                let queued = self.requests_dir().join(
                    path.file_name()
                        .ok_or_else(|| AppError::Internal("invalid Codex request path".into()))?,
                );
                fs::rename(path, queued).map_err(file_error)?;
            }
        }
        Ok(())
    }

    pub fn mark_request_processing(&self, request: &CodexControlRequest) -> AppResult<()> {
        match request.kind {
            CodexControlRequestKind::InstallCli => {
                self.update_runtime(|runtime| {
                    runtime.operation_status = CODEX_CLI_OPERATION_INSTALLING.into();
                    runtime.last_error = None;
                })?;
            }
            CodexControlRequestKind::UpdateCli => {
                self.update_runtime(|runtime| {
                    runtime.operation_status = CODEX_CLI_OPERATION_UPDATING.into();
                    runtime.last_error = None;
                })?;
            }
            CodexControlRequestKind::ProvisionAuth => {}
            CodexControlRequestKind::DeleteAuth => {}
            CodexControlRequestKind::RefreshMcp => {
                self.mark_mcp_request_processing(request, CODEX_MCP_OPERATION_REFRESHING)?
            }
            CodexControlRequestKind::AddMcp => {
                self.mark_mcp_request_processing(request, CODEX_MCP_OPERATION_ADDING)?
            }
            CodexControlRequestKind::RemoveMcp => {
                self.mark_mcp_request_processing(request, CODEX_MCP_OPERATION_REMOVING)?
            }
        }
        Ok(())
    }

    pub fn finish_request(&self, claimed: ClaimedCodexControlRequest) -> AppResult<()> {
        fs::remove_file(claimed.path).map_err(file_error)
    }

    pub fn fail_request(
        &self,
        claimed: ClaimedCodexControlRequest,
        message: &str,
    ) -> AppResult<()> {
        let safe_message = truncate(message, 1_000);
        match claimed.request.kind {
            CodexControlRequestKind::InstallCli | CodexControlRequestKind::UpdateCli => {
                self.update_runtime(|runtime| {
                    runtime.operation_status = CODEX_CLI_OPERATION_FAILED.into();
                    runtime.last_error = Some(safe_message.clone());
                })?;
            }
            CodexControlRequestKind::ProvisionAuth | CodexControlRequestKind::DeleteAuth => {
                if let Some(profile_id) = claimed.request.profile_id {
                    self.update_profile_status(
                        profile_id,
                        CODEX_AUTH_PROFILE_STATUS_FAILED,
                        Some(safe_message),
                    )?;
                }
            }
            CodexControlRequestKind::RefreshMcp
            | CodexControlRequestKind::AddMcp
            | CodexControlRequestKind::RemoveMcp => {
                if let Some(selector) = claimed.request.target_selector.as_deref() {
                    let _lock = self.lock()?;
                    self.update_mcp_operation_unlocked(
                        selector,
                        CODEX_MCP_OPERATION_FAILED,
                        claimed.request.mcp_server_name.clone(),
                        Some(safe_message.clone()),
                    )?;
                }
            }
        }
        fs::remove_file(claimed.path).map_err(file_error)
    }

    pub fn mark_auth_profile_active(&self, profile_id: Uuid) -> AppResult<()> {
        self.update_profile_status(profile_id, CODEX_AUTH_PROFILE_STATUS_ACTIVE, None)
    }

    pub fn remove_auth_profile(&self, profile_id: Uuid) -> AppResult<()> {
        let _lock = self.lock()?;
        let mut file = self.read_profiles_unlocked()?;
        file.profiles.retain(|profile| profile.id != profile_id);
        self.write_profiles_unlocked(&file)
    }

    pub fn publish_runtime_probe(
        &self,
        installed_version: Option<String>,
        source: &str,
        executable_path: &Path,
    ) -> AppResult<CodexCliRuntime> {
        self.update_runtime(|runtime| {
            runtime.installed = installed_version.is_some();
            runtime.installed_version = installed_version;
            runtime.source = if runtime.installed {
                source.to_string()
            } else {
                "unavailable".into()
            };
            runtime.executable_path = runtime
                .installed
                .then(|| executable_path.display().to_string());
            runtime.host_os = current_host_os();
            runtime.host_arch = current_host_arch();
            runtime.installer_kind = current_installer_kind();
            runtime.installation_supported = current_installation_supported();
            if runtime.operation_status != CODEX_CLI_OPERATION_INSTALL_PENDING
                && runtime.operation_status != CODEX_CLI_OPERATION_INSTALLING
                && runtime.operation_status != CODEX_CLI_OPERATION_UPDATE_PENDING
                && runtime.operation_status != CODEX_CLI_OPERATION_UPDATING
                && runtime.operation_status != CODEX_CLI_OPERATION_FAILED
            {
                runtime.operation_status = CODEX_CLI_OPERATION_IDLE.into();
            }
            if runtime.installed && runtime.operation_status == CODEX_CLI_OPERATION_IDLE {
                runtime.last_error = None;
            }
            runtime.update_available = versions_show_update(
                runtime.installed_version.as_deref(),
                runtime.latest_version.as_deref(),
            );
        })
    }

    pub fn publish_default_auth_probe(
        &self,
        status: &str,
        method: Option<String>,
        config: CodexDefaultConfigSummary,
        error: Option<String>,
    ) -> AppResult<CodexCliRuntime> {
        if !matches!(
            status,
            CODEX_DEFAULT_AUTH_STATUS_UNKNOWN
                | CODEX_DEFAULT_AUTH_STATUS_ACTIVE
                | CODEX_DEFAULT_AUTH_STATUS_LOGGED_OUT
        ) {
            return Err(AppError::Validation(
                "unsupported Codex default authentication status".into(),
            ));
        }
        self.update_runtime(|runtime| {
            runtime.default_auth.status = status.into();
            runtime.default_auth.method = method;
            runtime.default_auth.last_checked_at = Some(now_utc());
            runtime.default_auth.last_error = error.map(|message| truncate(&message, 1_000));
            runtime.default_auth.config = config;
        })
    }

    pub fn mark_cli_operation_succeeded(
        &self,
        installed_version: String,
        source: &str,
        executable_path: &Path,
    ) -> AppResult<CodexCliRuntime> {
        self.update_runtime(|runtime| {
            runtime.installed = true;
            runtime.installed_version = Some(installed_version);
            runtime.source = source.to_string();
            runtime.executable_path = Some(executable_path.display().to_string());
            runtime.operation_status = CODEX_CLI_OPERATION_IDLE.into();
            runtime.last_error = None;
            runtime.update_available = versions_show_update(
                runtime.installed_version.as_deref(),
                runtime.latest_version.as_deref(),
            );
        })
    }

    pub fn publish_latest_version(
        &self,
        latest_version: Option<String>,
        error: Option<String>,
    ) -> AppResult<CodexCliRuntime> {
        self.update_runtime(|runtime| {
            if let Some(version) = latest_version {
                runtime.latest_version = Some(version);
                runtime.update_check_error = None;
            } else {
                runtime.update_check_error = error.map(|message| truncate(&message, 1_000));
            }
            runtime.last_checked_at = Some(now_utc());
            runtime.update_available = versions_show_update(
                runtime.installed_version.as_deref(),
                runtime.latest_version.as_deref(),
            );
        })
    }

    fn update_profile_status(
        &self,
        profile_id: Uuid,
        status: &str,
        last_error: Option<String>,
    ) -> AppResult<()> {
        let _lock = self.lock()?;
        let mut file = self.read_profiles_unlocked()?;
        let profile = file
            .profiles
            .iter_mut()
            .find(|profile| profile.id == profile_id)
            .ok_or_else(|| AppError::NotFound("Codex authentication profile not found".into()))?;
        profile.status = status.into();
        profile.last_error = last_error;
        profile.updated_at = now_utc();
        self.write_profiles_unlocked(&file)
    }

    fn update_runtime(
        &self,
        update: impl FnOnce(&mut CodexCliRuntime),
    ) -> AppResult<CodexCliRuntime> {
        let _lock = self.lock()?;
        let mut runtime = self.read_runtime_unlocked()?;
        update(&mut runtime);
        runtime.updated_at = now_utc();
        self.write_runtime_unlocked(&runtime)?;
        Ok(runtime)
    }
}
