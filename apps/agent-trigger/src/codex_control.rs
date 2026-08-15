use super::*;

pub(super) fn publish_codex_runtime_probe(
    codex_control: &CodexControlStore,
    codex_runner: &CodexTriggerRunner,
) -> AppResult<()> {
    codex_control.publish_runtime_probe(
        codex_runner.detect_version(),
        codex_runner.executable_source(),
        codex_runner.executable_path(),
    )?;
    Ok(())
}

pub(super) async fn refresh_codex_default_auth(
    codex_control: &CodexControlStore,
    codex_runner: &CodexTriggerRunner,
) -> bool {
    let started = std::time::Instant::now();
    let result = codex_runner.probe_default_auth().await;
    let (publish_result, discovery_succeeded) = match result {
        Ok(probe) => (
            codex_control.publish_default_auth_probe(
                &probe.status,
                probe.method,
                probe.config,
                None,
            ),
            true,
        ),
        Err(error) => (
            codex_control.publish_default_auth_probe(
                CODEX_DEFAULT_AUTH_STATUS_UNKNOWN,
                None,
                CodexDefaultConfigSummary::default(),
                Some(sanitize_error(&error.to_string())),
            ),
            false,
        ),
    };
    let succeeded = match publish_result {
        Ok(_) => discovery_succeeded,
        Err(error) => {
            tracing::warn!(
                error = %sanitize_error(&error.to_string()),
                "failed to publish the host Codex authentication status"
            );
            false
        }
    };
    tracing::info!(
        succeeded,
        duration_ms = started.elapsed().as_millis() as u64,
        "Codex authentication discovery finished"
    );
    succeeded
}

pub(super) async fn refresh_codex_mcp_catalog(
    codex_control: &CodexControlStore,
    codex_runner: &CodexTriggerRunner,
) -> bool {
    let started = std::time::Instant::now();
    let selectors = match codex_control.list_mcp_target_selectors() {
        Ok(selectors) => selectors,
        Err(error) => {
            tracing::warn!(error = %sanitize_error(&error.to_string()), "failed to list Codex MCP target environments");
            return false;
        }
    };
    let mut succeeded = true;
    let environment_count = selectors.len();
    for selector in selectors {
        succeeded &= refresh_codex_mcp_environment(codex_control, codex_runner, &selector).await;
    }
    tracing::info!(
        succeeded,
        environments = environment_count,
        duration_ms = started.elapsed().as_millis() as u64,
        "Codex MCP discovery finished"
    );
    succeeded
}

pub(super) async fn refresh_codex_mcp_environment(
    codex_control: &CodexControlStore,
    codex_runner: &CodexTriggerRunner,
    selector: &str,
) -> bool {
    let result = codex_runner.discover_mcp_servers(selector).await;
    let (publish_result, discovery_succeeded) = match result {
        Ok(servers) => (
            codex_control.publish_mcp_snapshot(selector, servers, None),
            true,
        ),
        Err(error) => (
            codex_control.publish_mcp_snapshot(
                selector,
                Vec::new(),
                Some(sanitize_error(&error.to_string())),
            ),
            false,
        ),
    };
    if let Err(error) = publish_result {
        tracing::warn!(
            selector,
            error = %sanitize_error(&error.to_string()),
            "failed to publish the Codex MCP catalog"
        );
        return false;
    }
    discovery_succeeded
}

pub(super) async fn process_codex_control_request(
    codex_control: &CodexControlStore,
    codex_runner: &CodexTriggerRunner,
    config: &TriggerServiceConfig,
    claimed: ClaimedCodexControlRequest,
) {
    let started = std::time::Instant::now();
    if let Err(error) = codex_control.mark_request_processing(&claimed.request) {
        tracing::error!(error = %sanitize_error(&error.to_string()), "cannot mark Codex control request as processing");
        observability::record_control_request(started.elapsed(), false);
        return;
    }
    let result =
        execute_codex_control_request(codex_control, codex_runner, config, &claimed.request).await;
    let mut succeeded = result.is_ok();
    match result {
        Ok(()) => {
            if let Err(error) = codex_control.finish_request(claimed) {
                succeeded = false;
                tracing::error!(error = %sanitize_error(&error.to_string()), "cannot finish Codex control request");
            }
        }
        Err(error) => {
            let message = sanitize_error(&error.to_string());
            tracing::warn!(error = %message, "Codex control request failed");
            if let Err(store_error) = codex_control.fail_request(claimed, &message) {
                tracing::error!(error = %sanitize_error(&store_error.to_string()), "cannot persist Codex control request failure");
            }
        }
    }
    observability::record_control_request(started.elapsed(), succeeded);
}

pub(super) async fn execute_codex_control_request(
    codex_control: &CodexControlStore,
    codex_runner: &CodexTriggerRunner,
    config: &TriggerServiceConfig,
    request: &ai_chat_infrastructure::codex_control::CodexControlRequest,
) -> AppResult<()> {
    match request.kind {
        CodexControlRequestKind::InstallCli => {
            let version = if let Some(version) = codex_runner.detect_version() {
                version
            } else {
                install_managed_codex(codex_control, codex_runner, config).await?
            };
            codex_control.mark_cli_operation_succeeded(
                version,
                codex_runner.executable_source(),
                codex_runner.executable_path(),
            )?;
        }
        CodexControlRequestKind::UpdateCli => {
            let version = codex_runner.update_cli().await?;
            codex_control.mark_cli_operation_succeeded(
                version,
                codex_runner.executable_source(),
                codex_runner.executable_path(),
            )?;
        }
        CodexControlRequestKind::ProvisionAuth => {
            if codex_runner.detect_version().is_none() {
                if !config.codex_auto_install {
                    return Err(AppError::Conflict(
                        "Codex CLI is not installed; install it before configuring an API key"
                            .into(),
                    ));
                }
                let version = install_managed_codex(codex_control, codex_runner, config).await?;
                codex_control.mark_cli_operation_succeeded(
                    version,
                    codex_runner.executable_source(),
                    codex_runner.executable_path(),
                )?;
            }
            let profile_id = request.profile_id.ok_or_else(|| {
                AppError::Internal("Codex authentication request has no profile id".into())
            })?;
            if request.api_key.is_none() && request.base_url.is_none() {
                return Err(AppError::Internal(
                    "Codex authentication request has no configuration changes".into(),
                ));
            }
            codex_runner
                .provision_auth_profile(
                    profile_id,
                    request.api_key.as_deref(),
                    request.base_url.as_deref(),
                )
                .await?;
            codex_control.mark_auth_profile_active(profile_id)?;
        }
        CodexControlRequestKind::DeleteAuth => {
            let profile_id = request.profile_id.ok_or_else(|| {
                AppError::Internal("Codex authentication deletion has no profile id".into())
            })?;
            let home = codex_control.managed_profile_home(profile_id);
            if home.exists() {
                tokio::fs::remove_dir_all(&home).await.map_err(|error| {
                    AppError::Internal(format!("cannot remove managed Codex profile: {error}"))
                })?;
            }
            codex_control.remove_auth_profile(profile_id)?;
        }
        CodexControlRequestKind::RefreshMcp => {
            let selector = request.target_selector.as_deref().ok_or_else(|| {
                AppError::Internal("Codex MCP refresh has no target selector".into())
            })?;
            refresh_codex_mcp_environment(codex_control, codex_runner, selector).await;
            let snapshot = codex_control
                .environment_for_company(request.company_id.ok_or_else(|| {
                    AppError::Internal("Codex MCP refresh has no company id".into())
                })?)?
                .mcp_environments
                .into_iter()
                .find(|snapshot| snapshot.selector == selector)
                .ok_or_else(|| AppError::Internal("Codex MCP refresh was not published".into()))?;
            if snapshot.status == "failed" {
                return Err(AppError::Internal(
                    snapshot
                        .last_error
                        .unwrap_or_else(|| "Codex MCP refresh failed".into()),
                ));
            }
        }
        CodexControlRequestKind::AddMcp => {
            let input = request.mcp_server.as_ref().ok_or_else(|| {
                AppError::Internal("Codex MCP add request has no server configuration".into())
            })?;
            codex_runner.add_mcp_server(input).await?;
            refresh_codex_mcp_environment(codex_control, codex_runner, &input.target_selector)
                .await;
        }
        CodexControlRequestKind::RemoveMcp => {
            let selector = request.target_selector.as_deref().ok_or_else(|| {
                AppError::Internal("Codex MCP removal has no target selector".into())
            })?;
            let server_name = request
                .mcp_server_name
                .as_deref()
                .ok_or_else(|| AppError::Internal("Codex MCP removal has no server name".into()))?;
            codex_runner
                .remove_mcp_server(selector, server_name)
                .await?;
            refresh_codex_mcp_environment(codex_control, codex_runner, selector).await;
        }
    }
    Ok(())
}

pub(super) async fn install_managed_codex(
    codex_control: &CodexControlStore,
    codex_runner: &CodexTriggerRunner,
    config: &TriggerServiceConfig,
) -> AppResult<String> {
    if codex_runner.executable_source() != "managed" {
        return Err(AppError::Validation(format!(
            "configured Codex executable {} is unavailable; clear AGENT_TRIGGER_CODEX_BIN to use Relay managed installation",
            codex_runner.executable_path().display()
        )));
    }
    let response = reqwest::Client::builder()
        .connect_timeout(StdDuration::from_secs(15))
        .timeout(StdDuration::from_secs(60))
        .build()
        .map_err(|error| {
            AppError::Internal(format!("cannot create Codex installer client: {error}"))
        })?
        .get(&config.codex_install_url)
        .send()
        .await
        .map_err(|error| AppError::Internal(format!("cannot download Codex installer: {error}")))?
        .error_for_status()
        .map_err(|error| AppError::Internal(format!("Codex installer download failed: {error}")))?;
    if response
        .content_length()
        .is_some_and(|length| length > 5 * 1024 * 1024)
    {
        return Err(AppError::Internal(
            "Codex installer is unexpectedly large".into(),
        ));
    }
    let installer = response
        .bytes()
        .await
        .map_err(|error| AppError::Internal(format!("cannot read Codex installer: {error}")))?;
    if installer.len() > 5 * 1024 * 1024 {
        return Err(AppError::Internal(
            "Codex installer is unexpectedly large".into(),
        ));
    }
    tokio::fs::create_dir_all(codex_control.managed_cli_bin_dir())
        .await
        .map_err(|error| {
            AppError::Internal(format!("cannot create Codex install directory: {error}"))
        })?;
    tokio::fs::create_dir_all(codex_control.managed_cli_home())
        .await
        .map_err(|error| AppError::Internal(format!("cannot create Codex home: {error}")))?;

    let installer_command = codex_installer_command(std::env::consts::OS)?;
    tracing::info!(
        host_os = std::env::consts::OS,
        host_arch = std::env::consts::ARCH,
        installer_kind = installer_command.kind,
        "installing the Relay-managed Codex CLI"
    );
    let mut command = Command::new(installer_command.program);
    command
        .args(installer_command.arguments)
        .env("CODEX_HOME", codex_control.managed_cli_home())
        .env("CODEX_INSTALL_DIR", codex_control.managed_cli_bin_dir())
        .env("CODEX_NON_INTERACTIVE", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = command.spawn().map_err(|error| {
        AppError::Internal(format!("cannot start official Codex installer: {error}"))
    })?;
    let mut stdin = child.stdin.take().ok_or_else(|| {
        AppError::Internal("official Codex installer stdin is unavailable".into())
    })?;
    stdin.write_all(&installer).await.map_err(|error| {
        AppError::Internal(format!("cannot provide official Codex installer: {error}"))
    })?;
    drop(stdin);
    let output = tokio::time::timeout(StdDuration::from_secs(300), child.wait_with_output())
        .await
        .map_err(|_| AppError::Internal("official Codex installation timed out".into()))?
        .map_err(|error| AppError::Internal(format!("official Codex installer failed: {error}")))?;
    if !output.status.success() {
        return Err(AppError::Internal(format!(
            "official Codex installer failed: {}",
            sanitize_error(&String::from_utf8_lossy(&output.stderr))
        )));
    }
    codex_runner.detect_version().ok_or_else(|| {
        AppError::Internal(format!(
            "Codex installer completed but {} is unavailable",
            codex_runner.executable_path().display()
        ))
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CodexInstallerCommand {
    pub(super) program: &'static str,
    pub(super) arguments: &'static [&'static str],
    pub(super) kind: &'static str,
}

pub(super) fn default_codex_install_url(host_os: &str) -> &'static str {
    match host_os {
        "windows" => "https://chatgpt.com/codex/install.ps1",
        _ => "https://chatgpt.com/codex/install.sh",
    }
}

pub(super) fn codex_installer_command(host_os: &str) -> AppResult<CodexInstallerCommand> {
    match host_os {
        "macos" | "linux" => Ok(CodexInstallerCommand {
            program: "sh",
            arguments: &["-s"],
            kind: CODEX_INSTALLER_POSIX_SHELL,
        }),
        "windows" => Ok(CodexInstallerCommand {
            program: "powershell.exe",
            arguments: &[
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                "-",
            ],
            kind: CODEX_INSTALLER_POWERSHELL,
        }),
        other => Err(AppError::Validation(format!(
            "Relay managed Codex installation does not support host operating system {other}"
        ))),
    }
}

pub(super) async fn refresh_codex_latest_version(
    codex_control: &CodexControlStore,
    codex_runner: &CodexTriggerRunner,
    config: &TriggerServiceConfig,
) {
    if codex_runner.detect_version().is_none() {
        return;
    }
    let result = async {
        let response = reqwest::Client::builder()
            .connect_timeout(StdDuration::from_secs(10))
            .timeout(StdDuration::from_secs(20))
            .build()
            .map_err(|error| AppError::Internal(format!("cannot create update client: {error}")))?
            .get(&config.codex_update_registry_url)
            .send()
            .await
            .map_err(|error| AppError::Internal(format!("cannot check Codex updates: {error}")))?
            .error_for_status()
            .map_err(|error| AppError::Internal(format!("Codex update check failed: {error}")))?;
        let body = response
            .json::<serde_json::Value>()
            .await
            .map_err(|error| {
                AppError::Internal(format!("invalid Codex update response: {error}"))
            })?;
        body.get("version")
            .and_then(serde_json::Value::as_str)
            .filter(|version| !version.is_empty() && version.len() <= 80)
            .map(str::to_string)
            .ok_or_else(|| AppError::Internal("Codex update response has no version".into()))
    }
    .await;
    match result {
        Ok(version) => {
            if let Err(error) = codex_control.publish_latest_version(Some(version), None) {
                tracing::warn!(error = %sanitize_error(&error.to_string()), "cannot publish Codex latest version");
            }
        }
        Err(error) => {
            let message = sanitize_error(&error.to_string());
            if let Err(store_error) = codex_control.publish_latest_version(None, Some(message)) {
                tracing::warn!(error = %sanitize_error(&store_error.to_string()), "cannot publish Codex update check failure");
            }
        }
    }
}

pub(super) async fn refresh_codex_model_catalog(
    codex_control: &CodexControlStore,
    codex_runner: &CodexTriggerRunner,
    config: &TriggerServiceConfig,
) -> AppResult<()> {
    let mut catalog = CodexModelCatalogFile::default();
    let mut profiles = config.model_discovery_profiles.clone();
    profiles.extend(codex_control.list_active_profile_selectors()?);
    profiles.sort();
    profiles.dedup();
    for profile in &profiles {
        for bundled in [false, true] {
            match codex_runner.discover_models(profile, bundled).await {
                Ok(snapshot) => catalog.catalogs.push(snapshot),
                Err(error) if bundled => tracing::debug!(
                    codex_profile = profile,
                    error = %sanitize_error(&error.to_string()),
                    "bundled Codex model discovery is unavailable"
                ),
                Err(error) => return Err(error),
            }
        }
    }
    let parent = config
        .model_catalog_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    tokio::fs::create_dir_all(parent).await.map_err(|error| {
        AppError::Internal(format!(
            "cannot create Codex model catalog directory: {error}"
        ))
    })?;
    let bytes = serde_json::to_vec_pretty(&catalog).map_err(|error| {
        AppError::Internal(format!("cannot serialize Codex model catalog: {error}"))
    })?;
    let temporary_path = config.model_catalog_path.with_extension("json.tmp");
    tokio::fs::write(&temporary_path, bytes)
        .await
        .map_err(|error| {
            AppError::Internal(format!("cannot write Codex model catalog: {error}"))
        })?;
    tokio::fs::rename(&temporary_path, &config.model_catalog_path)
        .await
        .map_err(|error| {
            AppError::Internal(format!("cannot publish Codex model catalog: {error}"))
        })?;
    tracing::info!(
        path = %config.model_catalog_path.display(),
        catalogs = catalog.catalogs.len(),
        "local Codex model catalog refreshed by Trigger"
    );
    Ok(())
}

pub(super) async fn refresh_codex_plugin_catalog(
    platform: &TriggerPlatform,
    codex_runner: &CodexTriggerRunner,
    config: &TriggerServiceConfig,
    target_selector: &str,
) -> AppResult<(String, bool)> {
    let started = std::time::Instant::now();
    let discovery = codex_runner.discover_plugins(target_selector).await?;
    let fingerprint = codex_plugin_fingerprint(&discovery.installed);
    let installed = public_codex_plugin_items(&discovery.installed);
    let available = public_codex_plugin_items(&discovery.available);
    let marketplaces = public_codex_marketplaces(&discovery.marketplaces);
    let catalog_is_empty = codex_plugin_catalog_is_empty(&installed, &available, &marketplaces);
    let diagnostic_message = catalog_is_empty.then(|| {
        "The selected Codex authentication environment has no configured plugin marketplace. Sign in or configure a marketplace in this environment, then refresh again.".into()
    });
    let now = now_utc();
    platform.save_codex_plugin_catalog_snapshot(CodexPluginCatalogSnapshot {
        runner_id: config.plugin_host_id.clone(),
        target_selector: target_selector.into(),
        hostname: config.hostname.clone(),
        codex_version: codex_runner.detect_version(),
        fingerprint: fingerprint.clone(),
        discovery_status: if catalog_is_empty { "empty" } else { "ready" }.into(),
        diagnostic_message,
        installed,
        available,
        marketplaces,
        discovered_at: now,
        updated_at: now,
    })?;
    tracing::info!(
        runner_id = %config.plugin_host_id,
        target_selector,
        fingerprint = %fingerprint,
        duration_ms = started.elapsed().as_millis() as u64,
        "local Codex plugin catalog refreshed by Trigger"
    );
    Ok((fingerprint, catalog_is_empty))
}

pub(super) fn codex_plugin_catalog_is_empty(
    installed: &serde_json::Value,
    available: &serde_json::Value,
    marketplaces: &serde_json::Value,
) -> bool {
    installed.as_array().is_none_or(Vec::is_empty)
        && available.as_array().is_none_or(Vec::is_empty)
        && marketplaces.as_array().is_none_or(Vec::is_empty)
}

pub(super) async fn refresh_codex_plugin_catalogs(
    platform: &TriggerPlatform,
    codex_control: &CodexControlStore,
    codex_runner: &CodexTriggerRunner,
    config: &TriggerServiceConfig,
) -> AppResult<()> {
    let mut selectors = codex_control.list_active_profile_selectors()?;
    selectors.push("default".into());
    selectors.sort();
    selectors.dedup();
    let mut first_error = None;
    for selector in selectors {
        if let Err(error) =
            refresh_codex_plugin_catalog(platform, codex_runner, config, &selector).await
        {
            tracing::warn!(
                target_selector = %selector,
                error = %sanitize_error(&error.to_string()),
                "failed to refresh a Codex plugin environment"
            );
            first_error.get_or_insert(error);
        }
    }
    match first_error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

pub(super) async fn process_codex_plugin_operation(
    platform: &TriggerPlatform,
    codex_runner: &CodexTriggerRunner,
    config: &TriggerServiceConfig,
    operation: CodexPluginOperation,
) {
    let command_result = codex_runner
        .apply_plugin_operation(
            &operation.target_selector,
            &operation.operation,
            operation.plugin_id.as_deref(),
        )
        .await;
    let (succeeded, mut result, error_message) = match command_result {
        Ok(result) => (true, result, None),
        Err(error) => (
            false,
            serde_json::json!({}),
            Some(sanitize_error(&error.to_string())),
        ),
    };
    let mut final_succeeded = succeeded;
    let mut final_error = error_message;
    if succeeded {
        match refresh_codex_plugin_catalog(
            platform,
            codex_runner,
            config,
            &operation.target_selector,
        )
        .await
        {
            Ok((fingerprint, catalog_is_empty)) => {
                if let Some(object) = result.as_object_mut() {
                    object.insert("catalog_fingerprint".into(), fingerprint.into());
                }
                if catalog_is_empty && operation.operation == CODEX_PLUGIN_OPERATION_REFRESH {
                    final_succeeded = false;
                    final_error = Some(
                        "当前 Codex 认证环境没有配置插件 Marketplace；请先完成该环境登录或配置 Marketplace，再刷新。".into(),
                    );
                }
            }
            Err(error) if operation.operation == CODEX_PLUGIN_OPERATION_REFRESH => {
                final_succeeded = false;
                final_error = Some(sanitize_error(&error.to_string()));
            }
            Err(error) => {
                if let Some(object) = result.as_object_mut() {
                    object.insert(
                        "catalog_refresh_error".into(),
                        sanitize_error(&error.to_string()).into(),
                    );
                }
            }
        }
    }
    if let Err(error) = platform.finish_codex_plugin_operation(
        operation.id,
        &config.lease_owner,
        final_succeeded,
        result,
        final_error,
    ) {
        tracing::error!(
            operation_id = %operation.id,
            error = %sanitize_error(&error.to_string()),
            "failed to finish Codex plugin operation"
        );
    }
}

pub(super) fn codex_plugin_fingerprint(installed: &serde_json::Value) -> String {
    let mut plugins = installed
        .as_array()
        .into_iter()
        .flatten()
        .filter(|plugin| {
            plugin
                .get("pluginId")
                .and_then(serde_json::Value::as_str)
                .is_some_and(
                    ai_chat_infrastructure::codex_trigger::is_relay_supported_codex_plugin_id,
                )
        })
        .filter(|plugin| plugin.get("enabled").and_then(serde_json::Value::as_bool) != Some(false))
        .filter_map(|plugin| {
            let plugin_id = plugin.get("pluginId")?.as_str()?;
            let version = plugin
                .get("version")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            Some(format!("{plugin_id}@{version}"))
        })
        .collect::<Vec<_>>();
    plugins.sort();
    hash_secret(&plugins.join("\n")).chars().take(24).collect()
}

pub(super) fn public_codex_plugin_items(items: &serde_json::Value) -> serde_json::Value {
    let supported =
        ai_chat_infrastructure::codex_trigger::filter_relay_supported_codex_plugin_items(items);
    serde_json::Value::Array(
        supported
            .as_array()
            .into_iter()
            .flatten()
            .filter(|plugin| {
                plugin
                    .get("pluginId")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(ai_chat_infrastructure::codex_trigger::is_relay_supported_codex_plugin_id)
            })
            .filter_map(|plugin| {
                Some(serde_json::json!({
                    "pluginId": plugin.get("pluginId")?.as_str()?,
                    "name": plugin.get("name")?.as_str()?,
                    "marketplaceName": plugin.get("marketplaceName")?.as_str()?,
                    "version": plugin.get("version").and_then(serde_json::Value::as_str).unwrap_or_default(),
                    "installed": plugin.get("installed").and_then(serde_json::Value::as_bool).unwrap_or(false),
                    "enabled": plugin.get("enabled").and_then(serde_json::Value::as_bool).unwrap_or(false),
                    "installPolicy": plugin.get("installPolicy").and_then(serde_json::Value::as_str),
                    "authPolicy": plugin.get("authPolicy").and_then(serde_json::Value::as_str),
                }))
            })
            .collect(),
    )
}

pub(super) fn public_codex_marketplaces(items: &serde_json::Value) -> serde_json::Value {
    serde_json::Value::Array(
        items
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|marketplace| {
                Some(serde_json::json!({
                    "name": marketplace.get("name")?.as_str()?,
                    "sourceType": marketplace
                        .get("marketplaceSource")
                        .and_then(|source| source.get("sourceType"))
                        .and_then(serde_json::Value::as_str),
                }))
            })
            .collect(),
    )
}
