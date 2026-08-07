use super::*;

impl CodexTriggerRunner {
    pub fn from_env() -> AppResult<Self> {
        let control_store = CodexControlStore::from_env()?;
        let configured_executable = std::env::var("AGENT_TRIGGER_CODEX_BIN")
            .ok()
            .filter(|value| !value.trim().is_empty());
        let (executable, executable_source) = resolve_codex_executable(
            configured_executable.as_deref(),
            &control_store.managed_cli_executable(),
        );
        let prefix_args = std::env::var("AGENT_TRIGGER_CODEX_PREFIX_ARGS_JSON")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(|value| {
                serde_json::from_str::<Vec<String>>(&value).map_err(|error| {
                    AppError::Validation(format!(
                        "invalid AGENT_TRIGGER_CODEX_PREFIX_ARGS_JSON: {error}"
                    ))
                })
            })
            .transpose()?
            .unwrap_or_default();
        let mcp_url = std::env::var("AGENT_TRIGGER_MCP_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:8080/mcp".into());
        let mcp_server_name = std::env::var("AGENT_TRIGGER_MCP_SERVER_NAME")
            .unwrap_or_else(|_| "relay_company".into());
        let mut runner = Self::new(
            executable,
            prefix_args,
            mcp_url,
            mcp_server_name,
            DEFAULT_RUN_TOKEN_ENV.into(),
        )?;
        runner.executable_source = executable_source;
        runner.managed_profile_homes_root = control_store
            .managed_profile_home(Uuid::nil())
            .parent()
            .expect("managed profile home has a parent")
            .to_path_buf();
        runner.managed_cli_home = control_store.managed_cli_home();
        let allowlist = std::env::var("AGENT_TRIGGER_CODEX_ENV_ALLOWLIST")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(|value| split_csv(&value))
            .unwrap_or_else(default_environment_allowlist);
        for name in &allowlist {
            validate_environment_name(name)?;
        }
        runner.inherited_environment = collect_inherited_environment(&allowlist);
        let mut excluded = std::env::var("AGENT_TRIGGER_CODEX_SENSITIVE_ENV_NAMES")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(|value| split_csv(&value))
            .unwrap_or_else(|| vec!["CODEX_API_KEY".into(), "OPENAI_API_KEY".into()]);
        excluded.push(runner.run_token_env_name.clone());
        excluded.sort();
        excluded.dedup();
        for name in &excluded {
            validate_environment_name(name)?;
        }
        runner.shell_excluded_environment_names = excluded;
        Ok(runner)
    }

    pub fn new(
        executable: PathBuf,
        prefix_args: Vec<String>,
        mcp_url: String,
        mcp_server_name: String,
        run_token_env_name: String,
    ) -> AppResult<Self> {
        validate_safe_value(&mcp_url, "AGENT_TRIGGER_MCP_URL", 2_048)?;
        if !mcp_url.starts_with("http://") && !mcp_url.starts_with("https://") {
            return Err(AppError::Validation(
                "AGENT_TRIGGER_MCP_URL must use http or https".into(),
            ));
        }
        validate_config_key(&mcp_server_name, "AGENT_TRIGGER_MCP_SERVER_NAME")?;
        validate_environment_name(&run_token_env_name)?;
        for argument in &prefix_args {
            validate_safe_value(argument, "Codex command prefix argument", 4_096)?;
        }
        Ok(Self {
            executable,
            executable_source: "explicit".into(),
            prefix_args,
            mcp_url,
            mcp_server_name,
            inherited_environment: collect_inherited_environment(&default_environment_allowlist()),
            shell_excluded_environment_names: vec![
                "CODEX_API_KEY".into(),
                "OPENAI_API_KEY".into(),
                run_token_env_name.clone(),
            ],
            run_token_env_name,
            managed_profile_homes_root: PathBuf::from(".relay-agent-trigger")
                .join("codex-profiles")
                .join("homes"),
            managed_cli_home: PathBuf::from(".relay-agent-trigger")
                .join("codex-cli")
                .join("home"),
        })
    }

    pub fn executable_path(&self) -> &Path {
        &self.executable
    }

    pub fn executable_source(&self) -> &str {
        &self.executable_source
    }

    pub fn detect_version(&self) -> Option<String> {
        let mut command = std::process::Command::new(&self.executable);
        command
            .args(&self.prefix_args)
            .arg("--version")
            .env_clear()
            .envs(&self.inherited_environment);
        let output = command.output().ok()?;
        if !output.status.success() {
            return None;
        }
        let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
        (!version.is_empty()).then(|| truncate(&version, 200))
    }

    pub async fn probe_default_auth(&self) -> AppResult<CodexDefaultAuthProbe> {
        let mut command = Command::new(&self.executable);
        command
            .args(&self.prefix_args)
            .arg("login")
            .arg("status")
            .env_clear()
            .envs(&self.inherited_environment)
            .stdin(Stdio::null())
            .kill_on_drop(true);
        let output = timeout(Duration::from_secs(15), command.output())
            .await
            .map_err(|_| AppError::Internal("Codex default login status timed out".into()))?
            .map_err(|error| {
                AppError::Internal(format!("failed to start Codex login status: {error}"))
            })?;
        let mut probe = classify_default_auth_probe(
            output.status.success(),
            &String::from_utf8_lossy(&output.stdout),
            &String::from_utf8_lossy(&output.stderr),
        );
        probe.config = self.read_default_config_summary(probe.config.credential_hint.clone());
        Ok(probe)
    }

    fn read_default_config_summary(
        &self,
        credential_hint: Option<String>,
    ) -> CodexDefaultConfigSummary {
        let codex_home = self
            .inherited_environment
            .get("CODEX_HOME")
            .filter(|value| !value.trim().is_empty())
            .map(PathBuf::from)
            .or_else(|| {
                self.inherited_environment
                    .get("HOME")
                    .filter(|value| !value.trim().is_empty())
                    .map(|home| PathBuf::from(home).join(".codex"))
            });
        let Some(codex_home) = codex_home else {
            return CodexDefaultConfigSummary {
                credential_hint,
                ..CodexDefaultConfigSummary::default()
            };
        };
        let config_path = codex_home.join("config.toml");
        let auth_path = codex_home.join("auth.json");
        let mut summary = CodexDefaultConfigSummary {
            codex_home: Some(codex_home.display().to_string()),
            config_path: Some(config_path.display().to_string()),
            config_exists: config_path.is_file(),
            auth_path: Some(auth_path.display().to_string()),
            auth_exists: auth_path.is_file(),
            credential_hint,
            ..CodexDefaultConfigSummary::default()
        };
        let Ok(metadata) = std::fs::metadata(&config_path) else {
            return summary;
        };
        if metadata.len() > 2 * 1024 * 1024 {
            return summary;
        }
        let Ok(content) = std::fs::read_to_string(&config_path) else {
            return summary;
        };
        populate_default_config_summary(&content, &mut summary);
        summary
    }

    pub async fn provision_auth_profile(
        &self,
        profile_id: Uuid,
        api_key: Option<&str>,
        base_url: Option<&str>,
    ) -> AppResult<()> {
        let codex_home = self.managed_profile_homes_root.join(profile_id.to_string());
        tokio::fs::create_dir_all(&codex_home)
            .await
            .map_err(|error| {
                AppError::Internal(format!("cannot create managed Codex home: {error}"))
            })?;
        write_managed_openai_base_url(&codex_home, base_url).await?;
        if let Some(api_key) = api_key {
            if api_key.trim().is_empty() || api_key.chars().any(char::is_whitespace) {
                return Err(AppError::Validation("OpenAI API key is invalid".into()));
            }
            let mut command = Command::new(&self.executable);
            command
                .args(&self.prefix_args)
                .arg("login")
                .arg("--with-api-key")
                .env_clear()
                .envs(&self.inherited_environment)
                .env("CODEX_HOME", &codex_home)
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .kill_on_drop(true);
            let mut child = command.spawn().map_err(|error| {
                AppError::Internal(format!("failed to start Codex login: {error}"))
            })?;
            let mut stdin = child
                .stdin
                .take()
                .ok_or_else(|| AppError::Internal("Codex login stdin was not available".into()))?;
            stdin.write_all(api_key.as_bytes()).await.map_err(|error| {
                AppError::Internal(format!("failed to write Codex login input: {error}"))
            })?;
            stdin.write_all(b"\n").await.map_err(|error| {
                AppError::Internal(format!("failed to finish Codex login input: {error}"))
            })?;
            drop(stdin);
            let output = timeout(Duration::from_secs(90), child.wait_with_output())
                .await
                .map_err(|_| AppError::Internal("Codex API key login timed out".into()))?
                .map_err(|error| AppError::Internal(format!("Codex login failed: {error}")))?;
            if !output.status.success() {
                return Err(AppError::Validation(format!(
                    "Codex rejected the API key: {}",
                    truncate(
                        &sanitize_error(&String::from_utf8_lossy(&output.stderr)),
                        1_000
                    )
                )));
            }
        }

        let mut status = Command::new(&self.executable);
        status
            .args(&self.prefix_args)
            .arg("login")
            .arg("status")
            .env_clear()
            .envs(&self.inherited_environment)
            .env("CODEX_HOME", &codex_home)
            .stdin(Stdio::null())
            .kill_on_drop(true);
        let output = timeout(Duration::from_secs(30), status.output())
            .await
            .map_err(|_| AppError::Internal("Codex login status timed out".into()))?
            .map_err(|error| AppError::Internal(format!("Codex login status failed: {error}")))?;
        if !output.status.success() {
            return Err(AppError::Validation(
                "Codex did not persist the API key login".into(),
            ));
        }
        Ok(())
    }

    pub async fn update_cli(&self) -> AppResult<String> {
        let mut command = Command::new(&self.executable);
        command
            .args(&self.prefix_args)
            .arg("update")
            .env_clear()
            .envs(&self.inherited_environment)
            .stdin(Stdio::null())
            .kill_on_drop(true);
        if self.executable_source == "managed" {
            command.env("CODEX_HOME", &self.managed_cli_home);
        }
        let output = timeout(Duration::from_secs(300), command.output())
            .await
            .map_err(|_| AppError::Internal("Codex CLI update timed out".into()))?
            .map_err(|error| {
                AppError::Internal(format!("failed to start Codex update: {error}"))
            })?;
        if !output.status.success() {
            return Err(AppError::Internal(format!(
                "Codex CLI update failed: {}",
                truncate(
                    &sanitize_error(&String::from_utf8_lossy(&output.stderr)),
                    2_000
                )
            )));
        }
        self.detect_version().ok_or_else(|| {
            AppError::Internal(
                "Codex CLI update finished but its version cannot be detected".into(),
            )
        })
    }

    pub async fn discover_mcp_servers(
        &self,
        target_selector: &str,
    ) -> AppResult<Vec<CodexMcpServerView>> {
        let configured_names = self.configured_mcp_server_names(target_selector)?;
        let mut command = Command::new(&self.executable);
        command
            .args(&self.prefix_args)
            .env_clear()
            .envs(&self.inherited_environment);
        self.apply_profile_environment(&mut command, target_selector)?;
        self.apply_profile_arguments(&mut command, target_selector)?;
        command
            .arg("mcp")
            .arg("list")
            .arg("--json")
            .stdin(Stdio::null())
            .kill_on_drop(true);
        let output = timeout(Duration::from_secs(30), command.output())
            .await
            .map_err(|_| AppError::Internal("Codex MCP discovery timed out".into()))?
            .map_err(|error| {
                AppError::Internal(format!("failed to start Codex MCP discovery: {error}"))
            })?;
        if !output.status.success() {
            return Err(AppError::Internal(format!(
                "Codex MCP discovery failed: {}",
                truncate(
                    &sanitize_error(&String::from_utf8_lossy(&output.stderr)),
                    2_000
                )
            )));
        }
        let entries = serde_json::from_slice::<Vec<Value>>(&output.stdout).map_err(|error| {
            AppError::Internal(format!("Codex MCP list returned invalid JSON: {error}"))
        })?;
        let mut servers = entries
            .into_iter()
            .filter_map(|entry| safe_mcp_server_view(entry, &configured_names))
            .collect::<Vec<_>>();
        servers.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(servers)
    }

    pub async fn add_mcp_server(&self, input: &CodexMcpServerInput) -> AppResult<()> {
        let mut command = Command::new(&self.executable);
        command
            .args(&self.prefix_args)
            .env_clear()
            .envs(&self.inherited_environment);
        self.apply_profile_environment(&mut command, &input.target_selector)?;
        self.apply_profile_arguments(&mut command, &input.target_selector)?;
        command.arg("mcp").arg("add").arg(&input.name);
        match input.transport.as_str() {
            CODEX_MCP_TRANSPORT_HTTP => {
                if let Some(environment_name) = input.bearer_token_env_var.as_deref() {
                    command.arg("--bearer-token-env-var").arg(environment_name);
                }
                command
                    .arg("--url")
                    .arg(input.url.as_deref().ok_or_else(|| {
                        AppError::Validation("Streamable HTTP MCP requires a URL".into())
                    })?);
            }
            CODEX_MCP_TRANSPORT_STDIO => {
                command
                    .arg("--")
                    .arg(input.command.as_deref().ok_or_else(|| {
                        AppError::Validation("stdio MCP requires a command".into())
                    })?)
                    .args(&input.args);
            }
            _ => {
                return Err(AppError::Validation(
                    "unsupported Codex MCP transport".into(),
                ))
            }
        }
        command.stdin(Stdio::null()).kill_on_drop(true);
        let output = timeout(Duration::from_secs(60), command.output())
            .await
            .map_err(|_| AppError::Internal("Codex MCP add timed out".into()))?
            .map_err(|error| {
                AppError::Internal(format!("failed to start Codex MCP add: {error}"))
            })?;
        if !output.status.success() {
            return Err(AppError::Validation(format!(
                "Codex rejected the MCP configuration: {}",
                truncate(
                    &sanitize_error(&String::from_utf8_lossy(&output.stderr)),
                    2_000
                )
            )));
        }
        Ok(())
    }

    pub async fn remove_mcp_server(
        &self,
        target_selector: &str,
        server_name: &str,
    ) -> AppResult<()> {
        let mut command = Command::new(&self.executable);
        command
            .args(&self.prefix_args)
            .env_clear()
            .envs(&self.inherited_environment);
        self.apply_profile_environment(&mut command, target_selector)?;
        self.apply_profile_arguments(&mut command, target_selector)?;
        command
            .arg("mcp")
            .arg("remove")
            .arg(server_name)
            .stdin(Stdio::null())
            .kill_on_drop(true);
        let output = timeout(Duration::from_secs(30), command.output())
            .await
            .map_err(|_| AppError::Internal("Codex MCP removal timed out".into()))?
            .map_err(|error| {
                AppError::Internal(format!("failed to start Codex MCP removal: {error}"))
            })?;
        if !output.status.success() {
            return Err(AppError::Validation(format!(
                "Codex could not remove the MCP server: {}",
                truncate(
                    &sanitize_error(&String::from_utf8_lossy(&output.stderr)),
                    2_000
                )
            )));
        }
        Ok(())
    }

    pub async fn discover_models(
        &self,
        codex_profile: &str,
        bundled: bool,
    ) -> AppResult<CodexModelCatalogSnapshot> {
        validate_config_key(codex_profile, "Codex profile")?;
        let mut command = Command::new(&self.executable);
        command
            .args(&self.prefix_args)
            .env_clear()
            .envs(&self.inherited_environment);
        self.apply_profile_environment(&mut command, codex_profile)?;
        self.apply_profile_arguments(&mut command, codex_profile)?;
        command.arg("debug").arg("models");
        if bundled {
            command.arg("--bundled");
        }
        command.kill_on_drop(true);
        let output = timeout(Duration::from_secs(20), command.output())
            .await
            .map_err(|_| AppError::Internal("local Codex model discovery timed out".into()))?
            .map_err(|error| {
                AppError::Internal(format!(
                    "failed to start local Codex model discovery: {error}"
                ))
            })?;
        if !output.status.success() {
            let stderr = sanitize_error(&String::from_utf8_lossy(&output.stderr));
            return Err(AppError::Internal(format!(
                "local Codex model discovery failed: {}",
                truncate(&stderr, 1_000)
            )));
        }
        let catalog: Value = serde_json::from_slice(&output.stdout).map_err(|error| {
            AppError::Internal(format!(
                "local Codex returned an invalid model catalog: {error}"
            ))
        })?;
        let mut models = BTreeMap::new();
        collect_codex_models(&catalog, &mut models);
        if models.is_empty() {
            return Err(AppError::Internal(
                "local Codex model catalog contained no selectable models".into(),
            ));
        }
        Ok(CodexModelCatalogSnapshot {
            codex_profile: codex_profile.to_string(),
            bundled,
            models: models
                .into_iter()
                .map(|(id, mut info)| {
                    info.id = id;
                    info
                })
                .collect(),
            source: if bundled {
                "local_codex_bundled".into()
            } else {
                "local_codex".into()
            },
            discovered_at: chrono::Utc::now(),
        })
    }

    pub async fn discover_plugins(
        &self,
        target_selector: &str,
    ) -> AppResult<CodexPluginCatalogDiscovery> {
        validate_config_key(target_selector, "Codex plugin target environment")?;
        let plugins = self
            .run_plugin_json(
                target_selector,
                &["plugin", "list", "--available", "--json"],
                30,
            )
            .await?;
        let marketplaces = self
            .run_plugin_json(
                target_selector,
                &["plugin", "marketplace", "list", "--json"],
                30,
            )
            .await?;
        Ok(CodexPluginCatalogDiscovery {
            installed: plugins
                .get("installed")
                .cloned()
                .unwrap_or_else(|| json!([])),
            available: plugins
                .get("available")
                .cloned()
                .unwrap_or_else(|| json!([])),
            marketplaces: marketplaces
                .get("marketplaces")
                .cloned()
                .unwrap_or_else(|| json!([])),
        })
    }

    pub async fn apply_plugin_operation(
        &self,
        target_selector: &str,
        operation: &str,
        plugin_id: Option<&str>,
    ) -> AppResult<Value> {
        validate_config_key(target_selector, "Codex plugin target environment")?;
        match operation {
            "refresh" => Ok(json!({ "refreshed": true })),
            "install" | "remove" => {
                let plugin_id = plugin_id.ok_or_else(|| {
                    AppError::Validation("plugin_id is required for this operation".into())
                })?;
                validate_plugin_id(plugin_id)?;
                let cli_operation = plugin_cli_operation(operation)?;
                self.run_plugin_json(
                    target_selector,
                    &["plugin", cli_operation, plugin_id, "--json"],
                    120,
                )
                .await
            }
            _ => Err(AppError::Validation(
                "unsupported Codex plugin operation".into(),
            )),
        }
    }

    async fn run_plugin_json(
        &self,
        target_selector: &str,
        arguments: &[&str],
        timeout_seconds: u64,
    ) -> AppResult<Value> {
        let mut command = Command::new(&self.executable);
        command
            .args(&self.prefix_args)
            .env_clear()
            .envs(&self.inherited_environment);
        self.apply_profile_environment(&mut command, target_selector)?;
        self.apply_profile_arguments(&mut command, target_selector)?;
        command
            .args(arguments)
            .stdin(Stdio::null())
            .kill_on_drop(true);
        let output = timeout(Duration::from_secs(timeout_seconds), command.output())
            .await
            .map_err(|_| AppError::Internal("local Codex plugin command timed out".into()))?
            .map_err(|error| {
                AppError::Internal(format!(
                    "failed to start local Codex plugin command: {error}"
                ))
            })?;
        if !output.status.success() {
            let stderr = sanitize_error(&String::from_utf8_lossy(&output.stderr));
            return Err(AppError::Internal(format!(
                "local Codex plugin command failed: {}",
                truncate(&stderr, 2_000)
            )));
        }
        if output.stdout.is_empty() {
            return Ok(json!({ "ok": true }));
        }
        serde_json::from_slice(&output.stdout).map_err(|error| {
            AppError::Internal(format!(
                "local Codex plugin command returned invalid JSON: {error}"
            ))
        })
    }
}

pub(super) fn plugin_cli_operation(operation: &str) -> AppResult<&'static str> {
    match operation {
        "install" => Ok("add"),
        "remove" => Ok("remove"),
        _ => Err(AppError::Validation(
            "unsupported Codex plugin operation".into(),
        )),
    }
}
