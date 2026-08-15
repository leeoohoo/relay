use super::*;

const MAX_CODEX_PROFILE_BYTES: u64 = 2 * 1024 * 1024;
const MANAGED_RUN_PROFILE_PREFIX: &str = "relay_managed_cli_";

impl CodexTriggerRunner {
    pub(super) async fn apply_run_profile_arguments(
        &self,
        command: &mut Command,
        request: &CodexRunRequest,
    ) -> AppResult<()> {
        let disabled_plugin_ids = disabled_plugin_ids(&request.managed_mcp_servers);
        if disabled_plugin_ids.is_empty() {
            return self.apply_profile_arguments(command, &request.codex_profile);
        }

        let codex_home = self.codex_home_for_run(request)?;
        let source = read_selected_profile(&codex_home, &request.codex_profile).await?;
        let rendered = render_disabled_plugins_profile(&source, &disabled_plugin_ids);
        let profile_name = managed_run_profile_name(&request.codex_profile, &rendered);
        write_immutable_managed_profile(&codex_home, &profile_name, &rendered).await?;
        command.arg("--profile").arg(profile_name);
        Ok(())
    }

    pub(super) fn apply_app_server_profile_settings(
        &self,
        command: &mut Command,
        request: &CodexRunRequest,
    ) -> AppResult<()> {
        validate_config_key(&request.codex_profile, "Codex profile")?;
        if request.codex_profile.starts_with("relay_") {
            managed_profile_id(&request.codex_profile).ok_or_else(|| {
                AppError::Validation("managed Codex profile selector is invalid".into())
            })?;
        }

        // Codex CLI 0.146 rejects `--profile` for `app-server`. Relay already
        // injects the effective runtime settings explicitly, so app-server only
        // needs the per-run plugin exclusions that the exec path stores in a
        // derived profile.
        for plugin_id in disabled_plugin_ids(&request.managed_mcp_servers) {
            command
                .arg("--config")
                .arg(format!("plugins.{}.enabled=false", toml_string(&plugin_id)));
        }
        Ok(())
    }

    fn codex_home_for_run(&self, request: &CodexRunRequest) -> AppResult<PathBuf> {
        if let Some(value) = request
            .environment
            .get("CODEX_HOME")
            .filter(|value| !value.trim().is_empty())
        {
            return Ok(PathBuf::from(value));
        }
        if request.codex_profile.starts_with("relay_") {
            let profile_id = managed_profile_id(&request.codex_profile).ok_or_else(|| {
                AppError::Validation("managed Codex profile selector is invalid".into())
            })?;
            return Ok(self.managed_profile_homes_root.join(profile_id.to_string()));
        }
        self.inherited_environment
            .get("CODEX_HOME")
            .filter(|value| !value.trim().is_empty())
            .map(PathBuf::from)
            .or_else(|| {
                self.inherited_environment
                    .get("HOME")
                    .filter(|value| !value.trim().is_empty())
                    .map(|home| PathBuf::from(home).join(".codex"))
            })
            .ok_or_else(|| {
                AppError::Validation(
                    "Codex home cannot be resolved for the Relay-managed browser profile".into(),
                )
            })
    }
}

fn disabled_plugin_ids(servers: &[ManagedCodexMcpServer]) -> Vec<String> {
    let mut plugin_ids = RELAY_UNSUPPORTED_CODEX_PLUGIN_IDS
        .iter()
        .map(|plugin_id| (*plugin_id).to_string())
        .chain(
            servers
                .iter()
                .flat_map(|server| server.disabled_plugin_ids.iter().cloned()),
        )
        .collect::<Vec<_>>();
    plugin_ids.sort();
    plugin_ids.dedup();
    plugin_ids
}

async fn read_selected_profile(codex_home: &Path, selector: &str) -> AppResult<String> {
    if selector == "default" || selector.starts_with("relay_") {
        return Ok(String::new());
    }
    let profile_path = codex_home.join(format!("{selector}.config.toml"));
    let metadata = tokio::fs::metadata(&profile_path).await.map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            AppError::Validation(format!(
                "selected Codex profile {selector} does not exist in {}",
                codex_home.display()
            ))
        } else {
            AppError::Internal(format!("cannot inspect selected Codex profile: {error}"))
        }
    })?;
    if metadata.len() > MAX_CODEX_PROFILE_BYTES {
        return Err(AppError::Validation(
            "selected Codex profile is too large to derive safely".into(),
        ));
    }
    tokio::fs::read_to_string(profile_path)
        .await
        .map_err(|error| AppError::Internal(format!("cannot read selected Codex profile: {error}")))
}

pub(super) fn render_disabled_plugins_profile(source: &str, plugin_ids: &[String]) -> String {
    let mut rendered = Vec::new();
    let mut active_plugin = None::<String>;
    let mut enabled_written = false;
    let mut seen = std::collections::BTreeSet::new();

    for raw_line in source.lines() {
        if let Some(plugin_id) = exact_plugin_table(raw_line, plugin_ids) {
            finish_plugin_table(&mut rendered, active_plugin.take(), enabled_written);
            seen.insert(plugin_id.clone());
            active_plugin = Some(plugin_id);
            enabled_written = false;
            rendered.push(raw_line.to_string());
            continue;
        }
        if is_toml_table(raw_line) {
            finish_plugin_table(&mut rendered, active_plugin.take(), enabled_written);
            enabled_written = false;
        }
        if active_plugin.is_some() && toml_assignment_key(raw_line) == Some("enabled") {
            if !enabled_written {
                rendered.push("enabled = false".into());
                enabled_written = true;
            }
            continue;
        }
        rendered.push(raw_line.to_string());
    }
    finish_plugin_table(&mut rendered, active_plugin, enabled_written);

    for plugin_id in plugin_ids {
        if seen.contains(plugin_id) {
            continue;
        }
        if rendered.last().is_some_and(|line| !line.is_empty()) {
            rendered.push(String::new());
        }
        rendered.push(format!("[plugins.{}]", toml_string(plugin_id)));
        rendered.push("enabled = false".into());
    }

    let mut content = rendered.join("\n");
    if !content.is_empty() {
        content.push('\n');
    }
    content
}

fn finish_plugin_table(
    rendered: &mut Vec<String>,
    active_plugin: Option<String>,
    enabled_written: bool,
) {
    if active_plugin.is_some() && !enabled_written {
        rendered.push("enabled = false".into());
    }
}

fn exact_plugin_table(line: &str, plugin_ids: &[String]) -> Option<String> {
    let section = toml_table_section(line)?;
    let remainder = section.strip_prefix("plugins.")?;
    plugin_ids
        .iter()
        .find(|plugin_id| {
            remainder == toml_string(plugin_id) || remainder == format!("'{plugin_id}'")
        })
        .cloned()
}

fn is_toml_table(line: &str) -> bool {
    toml_table_section(line).is_some()
}

fn toml_table_section(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    let body = trimmed.strip_prefix('[')?;
    let closing = body.find(']')?;
    let tail = body[closing + 1..].trim();
    if !tail.is_empty() && !tail.starts_with('#') {
        return None;
    }
    Some(body[..closing].trim())
}

fn toml_assignment_key(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }
    trimmed.split_once('=').map(|(key, _)| key.trim())
}

fn managed_run_profile_name(selector: &str, content: &str) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in selector
        .as_bytes()
        .iter()
        .copied()
        .chain(std::iter::once(0))
        .chain(content.as_bytes().iter().copied())
    {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{MANAGED_RUN_PROFILE_PREFIX}{hash:016x}")
}

async fn write_immutable_managed_profile(
    codex_home: &Path,
    profile_name: &str,
    content: &str,
) -> AppResult<()> {
    tokio::fs::create_dir_all(codex_home)
        .await
        .map_err(|error| AppError::Internal(format!("cannot create Codex home: {error}")))?;
    let profile_path = codex_home.join(format!("{profile_name}.config.toml"));
    if tokio::fs::read_to_string(&profile_path)
        .await
        .ok()
        .as_deref()
        == Some(content)
    {
        return Ok(());
    }

    let temporary_path = codex_home.join(format!(
        ".{profile_name}-{}.config.toml.tmp",
        Uuid::new_v4()
    ));
    tokio::fs::write(&temporary_path, content)
        .await
        .map_err(|error| {
            AppError::Internal(format!("cannot write managed run profile: {error}"))
        })?;
    #[cfg(unix)]
    tokio::fs::set_permissions(
        &temporary_path,
        <std::fs::Permissions as std::os::unix::fs::PermissionsExt>::from_mode(0o600),
    )
    .await
    .map_err(|error| AppError::Internal(format!("cannot protect managed run profile: {error}")))?;

    if let Err(error) = tokio::fs::rename(&temporary_path, &profile_path).await {
        let concurrent_write_matches = tokio::fs::read_to_string(&profile_path)
            .await
            .ok()
            .as_deref()
            == Some(content);
        let _ = tokio::fs::remove_file(&temporary_path).await;
        if !concurrent_write_matches {
            return Err(AppError::Internal(format!(
                "cannot activate managed run profile: {error}"
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conflicting_plugins() -> Vec<String> {
        vec![
            "browser@openai-bundled".into(),
            "chrome@openai-bundled".into(),
        ]
    }

    #[test]
    fn managed_profile_preserves_source_and_disables_conflicting_plugins() {
        let rendered = render_disabled_plugins_profile(
            r#"model = "gpt-test"

[plugins."browser@openai-bundled"]
enabled = true
custom_setting = "preserved"

[features]
multi_agent = true
"#,
            &conflicting_plugins(),
        );

        assert!(rendered.contains("model = \"gpt-test\""));
        assert!(rendered.contains("custom_setting = \"preserved\""));
        assert!(rendered.contains("multi_agent = true"));
        assert_eq!(
            rendered
                .lines()
                .filter(|line| *line == "enabled = false")
                .count(),
            2
        );
        assert!(!rendered.contains("enabled = true"));
        assert!(rendered.contains("[plugins.\"chrome@openai-bundled\"]"));
    }

    #[test]
    fn managed_profile_does_not_duplicate_existing_enabled_key() {
        let rendered = render_disabled_plugins_profile(
            r#"[plugins.'browser@openai-bundled'] # existing plugin
enabled = true
enabled = false
"#,
            &["browser@openai-bundled".into()],
        );

        assert_eq!(
            rendered
                .lines()
                .filter(|line| *line == "enabled = false")
                .count(),
            1
        );
    }

    #[test]
    fn selected_profile_is_copied_into_the_managed_run_profile() {
        let root = std::env::temp_dir().join(format!(
            "relay-managed-run-profile-test-{}",
            Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&root).expect("Codex home");
        std::fs::write(
            root.join("team.config.toml"),
            "model = \"gpt-team\"\nmodel_reasoning_effort = \"high\"\n",
        )
        .expect("source profile");
        let mut runner = CodexTriggerRunner::new(
            PathBuf::from("codex"),
            Vec::new(),
            "http://127.0.0.1:8080/mcp".into(),
            "relay_company".into(),
            DEFAULT_RUN_TOKEN_ENV.into(),
        )
        .expect("runner");
        runner
            .inherited_environment
            .insert("CODEX_HOME".into(), root.to_string_lossy().into_owned());
        let request = CodexRunRequest {
            cwd: root.clone(),
            codex_profile: "team".into(),
            model: None,
            reasoning_effort: None,
            reasoning_summary: None,
            verbosity: None,
            personality: None,
            service_tier: None,
            sandbox_mode: "workspace_write".into(),
            approval_policy: "never".into(),
            network_access: false,
            web_search: "disabled".into(),
            feature_multi_agent: false,
            feature_remote_plugin: false,
            feature_hooks: false,
            feature_goals: false,
            feature_shell_tool: true,
            max_run_seconds: 60,
            prompt: "test".into(),
            existing_thread_id: None,
            run_token: "test".into(),
            session_kind: "project".into(),
            environment: HashMap::new(),
            managed_mcp_servers: vec![ManagedCodexMcpServer {
                name: "chrome-devtools".into(),
                command: "docker".into(),
                args: Vec::new(),
                env: BTreeMap::new(),
                disabled_plugin_ids: conflicting_plugins(),
                required: false,
                startup_timeout_sec: None,
                tool_timeout_sec: None,
                default_tools_approval_mode: "auto".into(),
                tool_approval_modes: BTreeMap::new(),
                prompt_hint: None,
            }],
            approval_handler: None,
            progress_handler: None,
            cancellation_handler: None,
        };
        let mut command = Command::new("codex");
        tokio::runtime::Runtime::new()
            .expect("runtime")
            .block_on(runner.apply_run_profile_arguments(&mut command, &request))
            .expect("managed run profile");
        let args = command
            .as_std()
            .get_args()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert_eq!(args.first().map(String::as_str), Some("--profile"));
        let profile_name = args.get(1).expect("profile name");
        let content = std::fs::read_to_string(root.join(format!("{profile_name}.config.toml")))
            .expect("derived profile");
        assert!(content.contains("model = \"gpt-team\""));
        assert!(content.contains("model_reasoning_effort = \"high\""));
        assert!(content.contains("[plugins.\"browser@openai-bundled\"]"));
        std::fs::remove_dir_all(root).expect("cleanup");
    }
}
