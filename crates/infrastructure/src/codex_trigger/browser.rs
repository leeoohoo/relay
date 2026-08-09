use super::*;

pub(super) const MANAGED_BROWSER_MCP_NAME: &str = "chrome-devtools";
const DEFAULT_BROWSER_MCP_IMAGE: &str = "relay/chrome-devtools-mcp:1.6.0";
const BROWSER_PROFILE_CONTAINER_PATH: &str = "/relay-browser-profile";
pub(super) const BROWSER_ARTIFACTS_RELATIVE_PATH: &str = ".relay/browser-artifacts";
pub(super) const CHROMIUM_RUNTIME_FILES: [&str; 4] = [
    "SingletonLock",
    "SingletonSocket",
    "SingletonCookie",
    "DevToolsActivePort",
];

#[derive(Debug, Clone)]
pub(super) struct BrowserMcpConfig {
    enabled: bool,
    command: String,
    image: String,
    profile_root: PathBuf,
}

impl BrowserMcpConfig {
    pub(super) fn disabled() -> Self {
        Self {
            enabled: false,
            command: "docker".into(),
            image: DEFAULT_BROWSER_MCP_IMAGE.into(),
            profile_root: PathBuf::from(".relay-agent-trigger/browser-profiles"),
        }
    }

    pub(super) fn from_env() -> AppResult<Self> {
        let enabled = std::env::var("RELAY_CHROME_DEVTOOLS_MCP_ENABLED")
            .ok()
            .map(|value| parse_enabled(&value))
            .unwrap_or(true);
        let command = std::env::var("RELAY_CHROME_DEVTOOLS_MCP_COMMAND")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "docker".into());
        let image = std::env::var("RELAY_CHROME_DEVTOOLS_MCP_IMAGE")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_BROWSER_MCP_IMAGE.into());
        let state_root = std::env::var("AGENT_TRIGGER_STATE_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(".relay-agent-trigger"));
        let profile_root = std::env::var("RELAY_CHROME_PROFILE_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|_| state_root.join("browser-profiles"));
        validate_safe_value(&command, "Chrome DevTools MCP command", 1_024)?;
        validate_safe_value(&image, "Chrome DevTools MCP image", 256)?;
        if image.chars().any(char::is_whitespace) {
            return Err(AppError::Validation(
                "Chrome DevTools MCP image cannot contain whitespace".into(),
            ));
        }
        Ok(Self {
            enabled,
            command,
            image,
            profile_root,
        })
    }

    #[cfg(test)]
    pub(super) fn for_test(profile_root: PathBuf) -> Self {
        Self {
            enabled: true,
            command: "docker".into(),
            image: DEFAULT_BROWSER_MCP_IMAGE.into(),
            profile_root,
        }
    }

    fn server(
        &self,
        company_id: Uuid,
        agent_id: Uuid,
        project_id: Uuid,
        workspace: &Path,
    ) -> AppResult<Option<ManagedCodexMcpServer>> {
        if !self.enabled {
            return Ok(None);
        }
        let profile = self
            .profile_root
            .join(company_id.to_string())
            .join(agent_id.to_string())
            .join(project_id.to_string());
        create_browser_profile(&profile)?;
        let profile = profile.canonicalize().map_err(|error| {
            AppError::Internal(format!("cannot resolve managed browser profile: {error}"))
        })?;
        let workspace = workspace.canonicalize().map_err(|error| {
            AppError::Internal(format!("cannot resolve managed browser workspace: {error}"))
        })?;
        if !workspace.is_dir() {
            return Err(AppError::Validation(
                "managed browser workspace must be a directory".into(),
            ));
        }
        let artifacts = create_browser_artifacts(&workspace)?;
        let mut args = vec![
            "run".into(),
            "--rm".into(),
            "--interactive".into(),
            "--init".into(),
            "--network=bridge".into(),
            "--add-host=host.docker.internal:host-gateway".into(),
            "--security-opt=no-new-privileges".into(),
            "--cap-drop=ALL".into(),
            "--pids-limit=512".into(),
            "--shm-size=1g".into(),
            "--env=HOME=/tmp".into(),
            "--env=CHROME_DEVTOOLS_MCP_NO_UPDATE_CHECKS=1".into(),
            "--env=CHROME_DEVTOOLS_MCP_NO_USAGE_STATISTICS=1".into(),
        ];
        if let Some(user) = browser_container_user(&profile) {
            args.push(format!("--user={user}"));
        }
        args.push(format!(
            "--volume={}:{}",
            profile.to_string_lossy(),
            BROWSER_PROFILE_CONTAINER_PATH
        ));
        args.push(format!("--volume={0}:{0}:ro", workspace.to_string_lossy()));
        args.push(format!("--volume={0}:{0}:rw", artifacts.to_string_lossy()));
        args.push(format!("--workdir={}", workspace.to_string_lossy()));
        args.extend([
            self.image.clone(),
            "--headless".into(),
            "--executable-path=/usr/bin/chromium".into(),
            format!("--user-data-dir={BROWSER_PROFILE_CONTAINER_PATH}"),
            "--viewport=1440x900".into(),
            "--chrome-arg=--no-sandbox".into(),
            "--chrome-arg=--host-resolver-rules=MAP 127.0.0.1 host.docker.internal, MAP localhost host.docker.internal".into(),
            "--chrome-arg=--force-prefers-reduced-motion=reduce".into(),
            "--no-usage-statistics".into(),
            "--no-performance-crux".into(),
            "--allow-unrestricted-paths".into(),
        ]);
        let tool_approval_modes = ["navigate_page", "new_page", "upload_file"]
            .into_iter()
            .map(|tool| (tool.to_string(), "prompt".to_string()))
            .collect();
        Ok(Some(ManagedCodexMcpServer {
            name: MANAGED_BROWSER_MCP_NAME.into(),
            command: self.command.clone(),
            args,
            env: BTreeMap::new(),
            disabled_plugin_ids: vec![
                "browser@openai-bundled".into(),
                "chrome@openai-bundled".into(),
            ],
            required: false,
            startup_timeout_sec: Some(90),
            tool_timeout_sec: Some(180),
            default_tools_approval_mode: "approve".into(),
            tool_approval_modes,
        }))
    }
}

impl CodexTriggerRunner {
    pub fn managed_browser_mcp_server(
        &self,
        company_id: Uuid,
        agent_id: Uuid,
        project_id: Uuid,
        workspace: &Path,
    ) -> AppResult<Option<ManagedCodexMcpServer>> {
        self.browser_mcp
            .server(company_id, agent_id, project_id, workspace)
    }

    pub(super) async fn managed_browser_mcp_view(&self) -> Option<CodexMcpServerView> {
        if !self.browser_mcp.enabled {
            return None;
        }
        let mut enabled = true;
        let mut disabled_reason = None;
        if self.browser_mcp.command == "docker" {
            let status = timeout(
                Duration::from_secs(10),
                Command::new(&self.browser_mcp.command)
                    .arg("image")
                    .arg("inspect")
                    .arg(&self.browser_mcp.image)
                    .env_clear()
                    .envs(&self.inherited_environment)
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status(),
            )
            .await;
            enabled = matches!(status, Ok(Ok(status)) if status.success());
            if !enabled {
                disabled_reason = Some(
                    "Relay 托管浏览器运行时尚未准备完成；重新执行 ./start.sh 即可自动安装".into(),
                );
            }
        }
        Some(CodexMcpServerView {
            name: MANAGED_BROWSER_MCP_NAME.into(),
            transport: CODEX_MCP_TRANSPORT_STDIO.into(),
            enabled,
            auth_status: Some("unsupported".into()),
            address: None,
            command: Some(self.browser_mcp.command.clone()),
            argument_count: 0,
            bearer_token_env_var: None,
            startup_timeout_sec: Some(90),
            tool_timeout_sec: Some(180),
            disabled_reason,
            configured_by_user: false,
            managed_by_relay: true,
        })
    }
}

fn parse_enabled(value: &str) -> bool {
    !matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "0" | "false" | "no" | "off"
    )
}

fn create_browser_profile(path: &Path) -> AppResult<()> {
    std::fs::create_dir_all(path).map_err(|error| {
        AppError::Internal(format!("cannot create managed browser profile: {error}"))
    })?;
    remove_stale_chromium_runtime_files(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).map_err(
            |error| AppError::Internal(format!("cannot protect managed browser profile: {error}")),
        )?;
    }
    Ok(())
}

fn create_browser_artifacts(workspace: &Path) -> AppResult<PathBuf> {
    let path = workspace.join(BROWSER_ARTIFACTS_RELATIVE_PATH);
    std::fs::create_dir_all(&path).map_err(|error| {
        AppError::Internal(format!(
            "cannot create managed browser artifact directory: {error}"
        ))
    })?;
    let path = path.canonicalize().map_err(|error| {
        AppError::Internal(format!(
            "cannot resolve managed browser artifact directory: {error}"
        ))
    })?;
    if !path.starts_with(workspace) {
        return Err(AppError::Validation(
            "managed browser artifact directory must remain inside the project workspace".into(),
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).map_err(
            |error| {
                AppError::Internal(format!(
                    "cannot protect managed browser artifact directory: {error}"
                ))
            },
        )?;
    }
    Ok(path)
}

fn remove_stale_chromium_runtime_files(profile: &Path) -> AppResult<()> {
    for name in CHROMIUM_RUNTIME_FILES {
        let path = profile.join(name);
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(AppError::Internal(format!(
                    "cannot remove stale Chromium runtime file {}: {error}",
                    path.display()
                )));
            }
        }
    }
    Ok(())
}

#[cfg(unix)]
fn browser_container_user(profile: &Path) -> Option<String> {
    use std::os::unix::fs::MetadataExt;
    let metadata = std::fs::metadata(profile).ok()?;
    Some(format!("{}:{}", metadata.uid(), metadata.gid()))
}

#[cfg(not(unix))]
fn browser_container_user(_profile: &Path) -> Option<String> {
    None
}
