use super::*;

pub(super) fn safe_mcp_server_view(
    entry: Value,
    configured_names: &std::collections::HashSet<String>,
) -> Option<CodexMcpServerView> {
    let name = entry.get("name")?.as_str()?.to_string();
    if name.is_empty() || name.len() > 160 || name.chars().any(char::is_control) {
        return None;
    }
    let transport = entry.get("transport")?;
    let transport_type = transport.get("type")?.as_str()?.to_string();
    let (address, command, argument_count, bearer_token_env_var) = match transport_type.as_str() {
        CODEX_MCP_TRANSPORT_HTTP => (
            transport
                .get("url")
                .and_then(Value::as_str)
                .and_then(safe_mcp_url),
            None,
            0,
            transport
                .get("bearer_token_env_var")
                .and_then(Value::as_str)
                .filter(|value| value.len() <= 128 && !value.chars().any(char::is_control))
                .map(str::to_string),
        ),
        CODEX_MCP_TRANSPORT_STDIO => (
            None,
            transport
                .get("command")
                .and_then(Value::as_str)
                .and_then(|value| {
                    Path::new(value)
                        .file_name()
                        .and_then(|value| value.to_str())
                })
                .filter(|value| value.len() <= 256 && !value.chars().any(char::is_control))
                .map(str::to_string),
            transport
                .get("args")
                .and_then(Value::as_array)
                .map(Vec::len)
                .unwrap_or_default(),
            None,
        ),
        _ => (None, None, 0, None),
    };
    Some(CodexMcpServerView {
        configured_by_user: configured_names.contains(&name),
        name,
        transport: transport_type,
        enabled: entry
            .get("enabled")
            .and_then(Value::as_bool)
            .unwrap_or(true),
        auth_status: entry
            .get("auth_status")
            .and_then(Value::as_str)
            .map(str::to_string),
        address,
        command,
        argument_count,
        bearer_token_env_var,
        startup_timeout_sec: entry.get("startup_timeout_sec").and_then(Value::as_u64),
        tool_timeout_sec: entry.get("tool_timeout_sec").and_then(Value::as_u64),
        disabled_reason: entry
            .get("disabled_reason")
            .and_then(Value::as_str)
            .filter(|value| value.len() <= 500 && !value.chars().any(char::is_control))
            .map(str::to_string),
    })
}

pub(super) fn safe_mcp_url(value: &str) -> Option<String> {
    let mut url = reqwest::Url::parse(value).ok()?;
    if !matches!(url.scheme(), "http" | "https") {
        return None;
    }
    let _ = url.set_username("");
    let _ = url.set_password(None);
    url.set_query(None);
    url.set_fragment(None);
    let result = url.to_string();
    (result.len() <= 2_048).then_some(result)
}

pub(super) fn classify_default_auth_probe(
    success: bool,
    stdout: &str,
    stderr: &str,
) -> CodexDefaultAuthProbe {
    if !success {
        return CodexDefaultAuthProbe {
            status: "logged_out".into(),
            method: None,
            config: CodexDefaultConfigSummary::default(),
        };
    }
    let combined = format!("{stdout}\n{stderr}").to_ascii_lowercase();
    let method = if combined.contains("chatgpt") {
        "chatgpt"
    } else if combined.contains("api key") || combined.contains("api_key") {
        "api_key"
    } else {
        "configured"
    };
    CodexDefaultAuthProbe {
        status: "active".into(),
        method: Some(method.into()),
        config: CodexDefaultConfigSummary {
            credential_hint: extract_masked_credential_hint(&combined),
            ..CodexDefaultConfigSummary::default()
        },
    }
}

pub(super) fn extract_masked_credential_hint(status_output: &str) -> Option<String> {
    status_output.lines().find_map(|line| {
        let candidate = line.rsplit_once(" - ")?.1.trim();
        (candidate.contains("***")
            && candidate.len() <= 96
            && candidate
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || "-_*".contains(character)))
        .then(|| candidate.to_string())
    })
}

pub(super) fn populate_default_config_summary(
    content: &str,
    summary: &mut CodexDefaultConfigSummary,
) {
    let mut section = String::new();
    let mut mcp_servers = std::collections::BTreeSet::new();
    let mut named_profiles = std::collections::BTreeSet::new();
    let mut trusted_projects = std::collections::BTreeSet::new();
    let mut plugins = std::collections::BTreeSet::new();
    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = line[1..line.len() - 1].trim().to_string();
            if let Some(name) = first_section_name(&section, "mcp_servers.") {
                mcp_servers.insert(name);
            }
            if let Some(name) = first_section_name(&section, "profiles.") {
                named_profiles.insert(name);
            }
            if section.starts_with("projects.") {
                trusted_projects.insert(section.clone());
            }
            if let Some(name) = first_section_name(&section, "plugins.") {
                plugins.insert(name);
            }
            continue;
        }
        if !section.is_empty() {
            continue;
        }
        let Some((key, raw_value)) = line.split_once('=') else {
            continue;
        };
        let value = parse_safe_toml_scalar(raw_value);
        match key.trim() {
            "openai_base_url" => summary.openai_base_url = value,
            "model_provider" => summary.model_provider = value,
            "model" => summary.model = value,
            "model_reasoning_effort" => summary.reasoning_effort = value,
            "sandbox_mode" => summary.sandbox_mode = value,
            "approval_policy" => summary.approval_policy = value,
            _ => {}
        }
    }
    summary.mcp_servers = mcp_servers.into_iter().collect();
    summary.named_profiles = named_profiles.into_iter().collect();
    summary.trusted_project_count = trusted_projects.len();
    summary.plugin_count = plugins.len();
}

pub(super) async fn write_managed_openai_base_url(
    codex_home: &Path,
    base_url: Option<&str>,
) -> AppResult<()> {
    let base_url = validate_managed_openai_base_url(base_url)?;
    let config_path = codex_home.join("config.toml");
    let existing = match tokio::fs::read_to_string(&config_path).await {
        Ok(content) if content.len() <= 2 * 1024 * 1024 => content,
        Ok(_) => {
            return Err(AppError::Validation(
                "managed Codex config.toml is too large to update safely".into(),
            ))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => {
            return Err(AppError::Internal(format!(
                "cannot read managed Codex config: {error}"
            )))
        }
    };
    let rendered = render_openai_base_url_config(&existing, base_url.as_deref());
    let temporary_path = codex_home.join(format!(".config-{}.toml.tmp", Uuid::new_v4()));
    tokio::fs::write(&temporary_path, rendered)
        .await
        .map_err(|error| {
            AppError::Internal(format!("cannot write managed Codex config: {error}"))
        })?;
    #[cfg(unix)]
    tokio::fs::set_permissions(
        &temporary_path,
        <std::fs::Permissions as std::os::unix::fs::PermissionsExt>::from_mode(0o600),
    )
    .await
    .map_err(|error| AppError::Internal(format!("cannot protect managed Codex config: {error}")))?;
    activate_managed_codex_config(&temporary_path, &config_path).await
}

pub(super) async fn activate_managed_codex_config(
    temporary_path: &Path,
    config_path: &Path,
) -> AppResult<()> {
    let backup_path = config_path.with_file_name(format!(".config-{}.toml.bak", Uuid::new_v4()));
    let had_existing = tokio::fs::metadata(config_path).await.is_ok();
    if had_existing {
        tokio::fs::rename(config_path, &backup_path)
            .await
            .map_err(|error| {
                AppError::Internal(format!("cannot back up managed Codex config: {error}"))
            })?;
    }
    if let Err(error) = tokio::fs::rename(temporary_path, config_path).await {
        if had_existing {
            let _ = tokio::fs::rename(&backup_path, config_path).await;
        }
        let _ = tokio::fs::remove_file(temporary_path).await;
        return Err(AppError::Internal(format!(
            "cannot activate managed Codex config: {error}"
        )));
    }
    if had_existing {
        let _ = tokio::fs::remove_file(backup_path).await;
    }
    Ok(())
}

pub(super) fn validate_managed_openai_base_url(value: Option<&str>) -> AppResult<Option<String>> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let parsed = reqwest::Url::parse(value)
        .map_err(|_| AppError::Validation("Codex Base URL is invalid".into()))?;
    if !matches!(parsed.scheme(), "http" | "https")
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || value.len() > 2_048
        || value.chars().any(char::is_control)
    {
        return Err(AppError::Validation("Codex Base URL is invalid".into()));
    }
    Ok(Some(value.trim_end_matches('/').to_string()))
}

pub(super) fn render_openai_base_url_config(content: &str, base_url: Option<&str>) -> String {
    let mut lines = Vec::new();
    let mut in_root = true;
    let mut inserted = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if in_root && trimmed.starts_with('[') {
            if let Some(base_url) = base_url {
                lines.push(format!(
                    "openai_base_url = {}",
                    serde_json::to_string(base_url).expect("Base URL JSON string")
                ));
                inserted = true;
            }
            in_root = false;
        }
        if in_root
            && trimmed
                .split_once('=')
                .is_some_and(|(key, _)| key.trim() == "openai_base_url")
        {
            continue;
        }
        lines.push(line.to_string());
    }
    if !inserted {
        if let Some(base_url) = base_url {
            lines.push(format!(
                "openai_base_url = {}",
                serde_json::to_string(base_url).expect("Base URL JSON string")
            ));
        }
    }
    let mut rendered = lines.join("\n");
    if !rendered.is_empty() {
        rendered.push('\n');
    }
    rendered
}

pub(super) fn first_section_name(section: &str, prefix: &str) -> Option<String> {
    let remainder = section.strip_prefix(prefix)?;
    let first = remainder.split('.').next()?.trim().trim_matches('"');
    (!first.is_empty() && first.len() <= 128 && !first.chars().any(char::is_control))
        .then(|| first.to_string())
}

pub(super) fn parse_safe_toml_scalar(raw: &str) -> Option<String> {
    let value = raw.trim().trim_matches('"').trim_matches('\'').trim();
    (!value.is_empty() && value.len() <= 256 && !value.chars().any(char::is_control))
        .then(|| value.to_string())
}

pub(super) fn validate_request(request: &CodexRunRequest) -> AppResult<()> {
    if !request.cwd.is_absolute() || !request.cwd.is_dir() {
        return Err(AppError::Validation(
            "Codex working directory must be an existing absolute directory".into(),
        ));
    }
    validate_config_key(&request.codex_profile, "codex_profile")?;
    if let Some(model) = request.model.as_deref() {
        validate_safe_value(model, "Codex model", 128)?;
    }
    if let Some(reasoning_effort) = request.reasoning_effort.as_deref() {
        if !matches!(
            reasoning_effort,
            "minimal" | "low" | "medium" | "high" | "xhigh" | "max" | "ultra"
        ) {
            return Err(AppError::Validation(
                "Codex reasoning effort must be minimal, low, medium, high, xhigh, max, or ultra"
                    .into(),
            ));
        }
    }
    if !matches!(request.approval_policy.as_str(), "never" | "on-request") {
        return Err(AppError::Validation(
            "Codex approval policy must be never or on-request".into(),
        ));
    }
    if request.approval_policy == "on-request" && request.approval_handler.is_none() {
        return Err(AppError::Validation(
            "Codex on-request approval policy requires an approval handler".into(),
        ));
    }
    if request.max_run_seconds == 0 {
        return Err(AppError::Validation(
            "Codex max_run_seconds must be positive".into(),
        ));
    }
    validate_prompt(&request.prompt)?;
    if request.run_token.trim().is_empty() || request.run_token.chars().any(char::is_control) {
        return Err(AppError::Validation("Codex run token is invalid".into()));
    }
    if let Some(thread_id) = request.existing_thread_id.as_deref() {
        validate_safe_value(thread_id, "Codex thread ID", 200)?;
    }
    for (key, value) in &request.environment {
        validate_environment_name(key)?;
        if value
            .chars()
            .any(|character| matches!(character, '\0' | '\r' | '\n'))
        {
            return Err(AppError::Validation(format!(
                "Codex environment value for {key} contains control characters"
            )));
        }
    }
    Ok(())
}

pub(super) fn codex_sandbox_mode(value: &str) -> AppResult<&'static str> {
    match value {
        "read_only" => Ok("read-only"),
        "workspace_write" => Ok("workspace-write"),
        _ => Err(AppError::Validation(
            "Codex sandbox mode must be read_only or workspace_write".into(),
        )),
    }
}

pub(super) fn validate_safe_value(
    value: &str,
    field: &str,
    max_characters: usize,
) -> AppResult<()> {
    if value.trim().is_empty()
        || value.chars().count() > max_characters
        || value
            .chars()
            .any(|character| matches!(character, '\0' | '\r' | '\n'))
    {
        return Err(AppError::Validation(format!("{field} is invalid")));
    }
    Ok(())
}

pub(super) fn validate_prompt(value: &str) -> AppResult<()> {
    if value.trim().is_empty()
        || value.chars().count() > 20_000
        || value
            .chars()
            .any(|character| character.is_control() && !matches!(character, '\n' | '\t'))
    {
        return Err(AppError::Validation("Codex prompt is invalid".into()));
    }
    Ok(())
}

pub(super) fn validate_config_key(value: &str, field: &str) -> AppResult<()> {
    if value.is_empty()
        || value.chars().count() > 80
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err(AppError::Validation(format!(
            "{field} contains unsupported characters"
        )));
    }
    Ok(())
}

pub(super) fn validate_plugin_id(value: &str) -> AppResult<()> {
    let Some((name, marketplace)) = value.split_once('@') else {
        return Err(AppError::Validation(
            "Codex plugin id must use name@marketplace format".into(),
        ));
    };
    let valid = |part: &str| {
        !part.is_empty()
            && part.chars().count() <= 100
            && part.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
            })
    };
    if value.chars().count() > 201 || !valid(name) || !valid(marketplace) {
        return Err(AppError::Validation(
            "Codex plugin id contains unsupported characters".into(),
        ));
    }
    Ok(())
}

pub(super) fn validate_environment_name(value: &str) -> AppResult<()> {
    let mut characters = value.chars();
    let valid_first = characters
        .next()
        .is_some_and(|character| character == '_' || character.is_ascii_alphabetic());
    if !valid_first
        || !characters.all(|character| character == '_' || character.is_ascii_alphanumeric())
    {
        return Err(AppError::Validation(format!(
            "environment variable name {value} is invalid"
        )));
    }
    Ok(())
}

pub(super) fn toml_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".into())
}

pub(super) fn apply_managed_cli_settings(
    command: &mut Command,
    request: &CodexRunRequest,
    auto_compact_token_limit: u64,
) {
    let mut set_string = |key: &str, value: Option<&str>| {
        if let Some(value) = value {
            command
                .arg("--config")
                .arg(format!("{key}={}", toml_string(value)));
        }
    };
    set_string(
        "model_reasoning_effort",
        request.reasoning_effort.as_deref(),
    );
    set_string(
        "model_reasoning_summary",
        request.reasoning_summary.as_deref(),
    );
    set_string("model_verbosity", request.verbosity.as_deref());
    set_string("personality", request.personality.as_deref());
    set_string("service_tier", request.service_tier.as_deref());
    set_string("web_search", Some(&request.web_search));
    command
        .arg("--config")
        .arg(format!(
            "model_auto_compact_token_limit={auto_compact_token_limit}"
        ))
        .arg("--config")
        .arg("model_auto_compact_token_limit_scope=\"total\"")
        .arg("--config")
        .arg(format!(
            "sandbox_workspace_write.network_access={}",
            request.network_access
        ))
        .arg("--config")
        .arg(format!(
            "features.multi_agent={}",
            request.feature_multi_agent
        ))
        .arg("--config")
        .arg(format!(
            "features.remote_plugin={}",
            request.feature_remote_plugin
        ))
        .arg("--config")
        .arg(format!("features.hooks={}", request.feature_hooks))
        .arg("--config")
        .arg(format!("features.goals={}", request.feature_goals))
        .arg("--config")
        .arg(format!(
            "features.shell_tool={}",
            request.feature_shell_tool
        ));
}

pub(super) fn toml_string_array(values: &[String]) -> String {
    let values = values
        .iter()
        .map(|value| toml_string(value))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{values}]")
}

pub(super) fn split_csv(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect()
}

pub(super) fn resolve_codex_executable(
    configured: Option<&str>,
    managed_executable: &Path,
) -> (PathBuf, String) {
    if let Some(configured) = configured {
        let candidate = PathBuf::from(configured);
        if candidate.components().count() > 1 || candidate.is_absolute() {
            return (candidate, "explicit".into());
        }
        if executable_in_path(configured).is_some() {
            return (candidate, "explicit".into());
        }
        if configured != "codex" {
            return (candidate, "explicit".into());
        }
    }
    if let Some(system) = executable_in_path("codex") {
        return (system, "system".into());
    }
    (managed_executable.to_path_buf(), "managed".into())
}

pub(super) fn executable_in_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for directory in std::env::split_paths(&path) {
        let candidate = directory.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
        #[cfg(windows)]
        {
            let candidate = directory.join(format!("{name}.exe"));
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

pub(super) fn default_environment_allowlist() -> Vec<String> {
    [
        "PATH",
        "HOME",
        "USER",
        "LOGNAME",
        "SHELL",
        "TMPDIR",
        "TMP",
        "TEMP",
        "LANG",
        "LC_ALL",
        "LC_CTYPE",
        "TERM",
        "COLORTERM",
        "CODEX_HOME",
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "ALL_PROXY",
        "NO_PROXY",
        "http_proxy",
        "https_proxy",
        "all_proxy",
        "no_proxy",
        "SSL_CERT_FILE",
        "SSL_CERT_DIR",
        "SYSTEMROOT",
        "COMSPEC",
        "PATHEXT",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

pub(super) fn collect_inherited_environment(allowlist: &[String]) -> HashMap<String, String> {
    allowlist
        .iter()
        .filter_map(|name| std::env::var(name).ok().map(|value| (name.clone(), value)))
        .collect()
}

pub(super) fn process_error(error: std::io::Error) -> AppError {
    AppError::Validation(format!(
        "Codex process I/O error: {}",
        sanitize_error(&error.to_string())
    ))
}

pub(super) fn join_error(error: tokio::task::JoinError) -> AppError {
    AppError::Validation(format!("Codex output reader failed: {error}"))
}

pub(super) fn sanitize_error(value: &str) -> String {
    value.replace(['\r', '\n'], " ")
}

pub(super) fn truncate(value: &str, max_characters: usize) -> String {
    value.chars().take(max_characters).collect()
}

#[cfg(unix)]
pub(super) fn terminate_process_tree(process_id: Option<u32>) {
    if let Some(process_id) = process_id {
        unsafe {
            libc::kill(-(process_id as i32), libc::SIGTERM);
        }
    }
}

#[cfg(not(unix))]
pub(super) fn terminate_process_tree(_process_id: Option<u32>) {}

#[cfg(unix)]
pub(super) fn kill_process_tree(process_id: Option<u32>) {
    if let Some(process_id) = process_id {
        unsafe {
            libc::kill(-(process_id as i32), libc::SIGKILL);
        }
        std::thread::yield_now();
    }
}

#[cfg(not(unix))]
pub(super) fn kill_process_tree(_process_id: Option<u32>) {}
