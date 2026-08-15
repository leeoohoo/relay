use super::*;
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    process::{Child, Command as StdCommand},
    thread,
};

pub(super) const MANAGED_BROWSER_MCP_NAME: &str = "chrome-devtools";
const DEFAULT_BROWSER_MCP_IMAGE: &str = "relay/chrome-devtools-mcp:1.6.0";
pub(super) const DEFAULT_BROWSER_MCP_PACKAGE: &str = "chrome-devtools-mcp@1.6.0";
const BROWSER_PROFILE_CONTAINER_PATH: &str = "/relay-browser-profile";
const HOST_BROWSER_ENDPOINT_FILE: &str = ".relay-devtools-endpoint";
const HOST_BROWSER_START_TIMEOUT: Duration = Duration::from_secs(12);
const MAX_BROWSER_DEBUG_RESPONSE_BYTES: u64 = 1024 * 1024;
pub(super) const RELAY_BROWSER_PAGE_ID_ENV: &str = "RELAY_BROWSER_PAGE_ID";
pub(super) const BROWSER_ARTIFACTS_RELATIVE_PATH: &str = ".relay/browser-artifacts";
pub(super) const CHROMIUM_RUNTIME_FILES: [&str; 4] = [
    "SingletonLock",
    "SingletonSocket",
    "SingletonCookie",
    "DevToolsActivePort",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BrowserMcpMode {
    Auto,
    Host,
    Docker,
}

impl BrowserMcpMode {
    fn parse(value: &str) -> AppResult<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "auto" | "" => Ok(Self::Auto),
            "host" | "native" => Ok(Self::Host),
            "docker" => Ok(Self::Docker),
            _ => Err(AppError::Validation(
                "RELAY_CHROME_DEVTOOLS_MCP_MODE must be auto, host, or docker".into(),
            )),
        }
    }
}

#[derive(Debug)]
struct HostBrowserProcess {
    endpoint: String,
    child: Option<Child>,
    last_used_at: std::time::Instant,
    agent_pages: HashMap<Uuid, String>,
}

impl Drop for HostBrowserProcess {
    fn drop(&mut self) {
        if let Some(child) = self.child.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct BrowserMcpConfig {
    enabled: bool,
    mode: BrowserMcpMode,
    docker_command: String,
    image: String,
    host_mcp_command: Option<PathBuf>,
    host_browser_executable: Option<PathBuf>,
    profile_root: PathBuf,
    docker_cpus: String,
    docker_memory: String,
    host_browser_idle_timeout: Duration,
    pool: Arc<Mutex<Option<HostBrowserProcess>>>,
}

impl BrowserMcpConfig {
    pub(super) fn disabled() -> Self {
        Self {
            enabled: false,
            mode: BrowserMcpMode::Auto,
            docker_command: "docker".into(),
            image: DEFAULT_BROWSER_MCP_IMAGE.into(),
            host_mcp_command: None,
            host_browser_executable: None,
            profile_root: PathBuf::from(".relay-agent-trigger/browser-profiles"),
            docker_cpus: "1.0".into(),
            docker_memory: "768m".into(),
            host_browser_idle_timeout: Duration::from_secs(15 * 60),
            pool: Arc::new(Mutex::new(None)),
        }
    }

    pub(super) fn from_env() -> AppResult<Self> {
        let enabled = std::env::var("RELAY_CHROME_DEVTOOLS_MCP_ENABLED")
            .ok()
            .map(|value| parse_enabled(&value))
            .unwrap_or(true);
        let mode = BrowserMcpMode::parse(
            &std::env::var("RELAY_CHROME_DEVTOOLS_MCP_MODE").unwrap_or_else(|_| "auto".into()),
        )?;
        let docker_command = std::env::var("RELAY_CHROME_DEVTOOLS_MCP_DOCKER_COMMAND")
            .or_else(|_| std::env::var("RELAY_CHROME_DEVTOOLS_MCP_COMMAND"))
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "docker".into());
        let image = std::env::var("RELAY_CHROME_DEVTOOLS_MCP_IMAGE")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_BROWSER_MCP_IMAGE.into());
        let host_mcp_command = configured_or_discovered_executable(
            "RELAY_CHROME_DEVTOOLS_MCP_HOST_COMMAND",
            &["npx", "npx.cmd", "npx.exe"],
        );
        let host_browser_executable = configured_or_discovered_browser();
        let state_root = std::env::var("AGENT_TRIGGER_STATE_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(".relay-agent-trigger"));
        let profile_root = std::env::var("RELAY_CHROME_PROFILE_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|_| state_root.join("browser-profiles"));
        let docker_cpus = std::env::var("RELAY_CHROME_DOCKER_CPUS")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "1.0".into());
        let docker_memory = std::env::var("RELAY_CHROME_DOCKER_MEMORY")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "768m".into());
        let host_browser_idle_timeout = Duration::from_secs(
            std::env::var("RELAY_CHROME_IDLE_TIMEOUT_SECONDS")
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
                .map(|value| value.clamp(60, 86_400))
                .unwrap_or(15 * 60),
        );
        validate_safe_value(&docker_command, "Chrome DevTools MCP Docker command", 1_024)?;
        validate_safe_value(&image, "Chrome DevTools MCP image", 256)?;
        validate_safe_value(&docker_cpus, "Chrome Docker CPU limit", 32)?;
        validate_safe_value(&docker_memory, "Chrome Docker memory limit", 32)?;
        if image.chars().any(char::is_whitespace) {
            return Err(AppError::Validation(
                "Chrome DevTools MCP image cannot contain whitespace".into(),
            ));
        }
        if enabled
            && mode == BrowserMcpMode::Host
            && (host_mcp_command.is_none() || host_browser_executable.is_none())
        {
            return Err(AppError::Validation(
                "host browser mode requires both Chrome/Chromium and npx; install them or use RELAY_CHROME_DEVTOOLS_MCP_MODE=docker".into(),
            ));
        }
        Ok(Self {
            enabled,
            mode,
            docker_command,
            image,
            host_mcp_command,
            host_browser_executable,
            profile_root,
            docker_cpus,
            docker_memory,
            host_browser_idle_timeout,
            pool: Arc::new(Mutex::new(None)),
        })
    }

    #[cfg(test)]
    pub(super) fn for_test(profile_root: PathBuf) -> Self {
        Self {
            enabled: true,
            mode: BrowserMcpMode::Docker,
            docker_command: "docker".into(),
            image: DEFAULT_BROWSER_MCP_IMAGE.into(),
            host_mcp_command: None,
            host_browser_executable: None,
            profile_root,
            docker_cpus: "1.0".into(),
            docker_memory: "768m".into(),
            host_browser_idle_timeout: Duration::from_secs(15 * 60),
            pool: Arc::new(Mutex::new(None)),
        }
    }

    #[cfg(test)]
    pub(super) fn for_test_host(profile_root: PathBuf) -> Option<Self> {
        let config = Self {
            enabled: true,
            mode: BrowserMcpMode::Host,
            docker_command: "docker".into(),
            image: DEFAULT_BROWSER_MCP_IMAGE.into(),
            host_mcp_command: configured_or_discovered_executable(
                "RELAY_CHROME_DEVTOOLS_MCP_HOST_COMMAND",
                &["npx", "npx.cmd", "npx.exe"],
            ),
            host_browser_executable: configured_or_discovered_browser(),
            profile_root,
            docker_cpus: "1.0".into(),
            docker_memory: "768m".into(),
            host_browser_idle_timeout: Duration::from_secs(15 * 60),
            pool: Arc::new(Mutex::new(None)),
        };
        config.uses_host().then_some(config)
    }

    fn uses_host(&self) -> bool {
        self.mode != BrowserMcpMode::Docker
            && self.host_mcp_command.is_some()
            && self.host_browser_executable.is_some()
    }

    fn profile(&self) -> PathBuf {
        self.profile_root.join("shared").join("native")
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
        let workspace = workspace.canonicalize().map_err(|error| {
            AppError::Internal(format!("cannot resolve managed browser workspace: {error}"))
        })?;
        if !workspace.is_dir() {
            return Err(AppError::Validation(
                "managed browser workspace must be a directory".into(),
            ));
        }
        create_browser_artifacts(&workspace)?;

        if self.uses_host() {
            return self.host_server(agent_id).map(Some);
        }
        self.docker_server(company_id, agent_id, project_id, &workspace)
            .map(Some)
    }

    fn host_server(&self, agent_id: Uuid) -> AppResult<ManagedCodexMcpServer> {
        let endpoint = self.ensure_host_browser()?;
        let page_id = self.ensure_agent_browser_page(&endpoint, agent_id)?;
        let host_command = self
            .host_mcp_command
            .as_ref()
            .expect("host mode checks the MCP command");
        let command = host_command.to_string_lossy().into_owned();
        let mut server = managed_server(command, host_mcp_args(host_command, &endpoint), 45);
        server
            .env
            .insert(RELAY_BROWSER_PAGE_ID_ENV.into(), page_id.clone());
        server
            .env
            .insert("CHROME_DEVTOOLS_MCP_NO_UPDATE_CHECKS".into(), "1".into());
        server
            .env
            .insert("CHROME_DEVTOOLS_MCP_NO_USAGE_STATISTICS".into(), "1".into());
        server.prompt_hint = Some(format!(
            "此 Trigger 设备上的所有 Agent 共用一个 Chrome。你的专属标签页 pageId 是 `{page_id}`。所有支持 pageId 的 chrome-devtools 页面工具都必须显式传入该 pageId；不要读取或操作其他 pageId。"
        ));
        Ok(server)
    }

    fn ensure_host_browser(&self) -> AppResult<String> {
        let profile = self.profile();
        create_browser_profile(&profile)?;
        let profile = profile.canonicalize().map_err(|error| {
            AppError::Internal(format!("cannot resolve managed browser profile: {error}"))
        })?;
        let mut pool = self.pool.lock().map_err(|_| {
            AppError::Internal("managed browser process pool lock was poisoned".into())
        })?;
        if let Some(process) = pool.as_mut() {
            if browser_endpoint_ready(&process.endpoint) {
                process.last_used_at = std::time::Instant::now();
                if process
                    .child
                    .as_mut()
                    .is_some_and(|child| child.try_wait().ok().flatten().is_some())
                {
                    process.child = None;
                }
                return Ok(process.endpoint.clone());
            }
            let child_still_running = process
                .child
                .as_mut()
                .is_some_and(|child| child.try_wait().ok().flatten().is_none());
            if child_still_running {
                return Ok(process.endpoint.clone());
            }
            pool.take();
        }

        if let Some(endpoint) = read_reusable_endpoint(&profile) {
            *pool = Some(HostBrowserProcess {
                endpoint: endpoint.clone(),
                child: None,
                last_used_at: std::time::Instant::now(),
                agent_pages: HashMap::new(),
            });
            return Ok(endpoint);
        }

        remove_stale_chromium_runtime_files(&profile)?;
        let port = reserve_loopback_port()?;
        let endpoint = format!("http://127.0.0.1:{port}");
        let executable = self
            .host_browser_executable
            .as_ref()
            .expect("host mode checks the browser executable");
        let mut command = StdCommand::new(executable);
        command
            .arg("--headless=new")
            .arg("--remote-debugging-address=127.0.0.1")
            .arg(format!("--remote-debugging-port={port}"))
            .arg(format!("--user-data-dir={}", profile.to_string_lossy()))
            .arg("--no-first-run")
            .arg("--no-default-browser-check")
            .arg("--disable-background-networking")
            .arg("--disable-component-update")
            .arg("--disable-sync")
            .arg("--force-prefers-reduced-motion")
            .arg("--remote-allow-origins=*")
            .arg("--window-size=1440,900")
            .arg("about:blank")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(unix)]
        if std::env::var("USER").ok().as_deref() == Some("root") {
            command.arg("--no-sandbox");
        }
        let mut child = command.spawn().map_err(|error| {
            AppError::Internal(format!(
                "cannot start managed host browser {}: {error}",
                executable.display()
            ))
        })?;
        let deadline = std::time::Instant::now() + HOST_BROWSER_START_TIMEOUT;
        while std::time::Instant::now() < deadline {
            if browser_endpoint_ready(&endpoint) {
                std::fs::write(profile.join(HOST_BROWSER_ENDPOINT_FILE), &endpoint).map_err(
                    |error| {
                        AppError::Internal(format!(
                            "cannot persist managed browser endpoint: {error}"
                        ))
                    },
                )?;
                *pool = Some(HostBrowserProcess {
                    endpoint: endpoint.clone(),
                    child: Some(child),
                    last_used_at: std::time::Instant::now(),
                    agent_pages: HashMap::new(),
                });
                return Ok(endpoint);
            }
            if child.try_wait().ok().flatten().is_some() {
                return Err(AppError::Internal(
                    "managed host browser exited before DevTools became ready".into(),
                ));
            }
            thread::sleep(Duration::from_millis(100));
        }
        let _ = child.kill();
        let _ = child.wait();
        Err(AppError::Internal(format!(
            "managed host browser did not become ready within {} seconds",
            HOST_BROWSER_START_TIMEOUT.as_secs()
        )))
    }

    fn ensure_agent_browser_page(&self, endpoint: &str, agent_id: Uuid) -> AppResult<String> {
        let page_url = agent_browser_page_url(agent_id);
        let browser_pages = browser_debug_json(endpoint, "GET", "/json/list")?;
        let browser_pages = browser_pages.as_array().ok_or_else(|| {
            AppError::Internal("managed browser returned an invalid page list".into())
        })?;
        let mut pool = self.pool.lock().map_err(|_| {
            AppError::Internal("managed browser process pool lock was poisoned".into())
        })?;
        let process = pool.as_mut().ok_or_else(|| {
            AppError::Internal("managed browser process disappeared during page setup".into())
        })?;
        if let Some(page_id) = process.agent_pages.get(&agent_id).filter(|page_id| {
            browser_pages
                .iter()
                .any(|page| page.get("id").and_then(Value::as_str) == Some(page_id.as_str()))
        }) {
            return Ok(page_id.clone());
        }
        if let Some(page_id) = browser_pages.iter().find_map(|page| {
            (page.get("url").and_then(Value::as_str) == Some(page_url.as_str()))
                .then(|| page.get("id").and_then(Value::as_str))
                .flatten()
        }) {
            validate_safe_value(page_id, "managed browser page ID", 256)?;
            process.agent_pages.insert(agent_id, page_id.to_string());
            return Ok(page_id.to_string());
        }

        let encoded_url = page_url.replace('#', "%23");
        let page = browser_debug_json(endpoint, "PUT", &format!("/json/new?{encoded_url}"))?;
        let page_id = page
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| AppError::Internal("managed browser did not return a page ID".into()))?;
        validate_safe_value(page_id, "managed browser page ID", 256)?;
        process.agent_pages.insert(agent_id, page_id.to_string());
        Ok(page_id.to_string())
    }

    fn prune_idle_host_browsers(&self) -> AppResult<usize> {
        let mut pool = self.pool.lock().map_err(|_| {
            AppError::Internal("managed browser process pool lock was poisoned".into())
        })?;
        let now = std::time::Instant::now();
        let expired = pool.as_ref().is_some_and(|process| {
            now.saturating_duration_since(process.last_used_at) >= self.host_browser_idle_timeout
        });
        if expired {
            pool.take();
        }
        Ok(usize::from(expired))
    }

    fn docker_server(
        &self,
        company_id: Uuid,
        agent_id: Uuid,
        project_id: Uuid,
        workspace: &Path,
    ) -> AppResult<ManagedCodexMcpServer> {
        let profile = self
            .profile_root
            .join(company_id.to_string())
            .join(agent_id.to_string())
            .join(project_id.to_string());
        create_browser_profile(&profile)?;
        remove_stale_chromium_runtime_files(&profile)?;
        let profile = profile.canonicalize().map_err(|error| {
            AppError::Internal(format!("cannot resolve managed browser profile: {error}"))
        })?;
        let artifacts = create_browser_artifacts(workspace)?;
        let mut args = vec![
            "run".into(),
            "--rm".into(),
            "--interactive".into(),
            "--init".into(),
            "--network=bridge".into(),
            "--add-host=host.docker.internal:host-gateway".into(),
            "--security-opt=no-new-privileges".into(),
            "--cap-drop=ALL".into(),
            "--pids-limit=256".into(),
            "--shm-size=256m".into(),
            format!("--cpus={}", self.docker_cpus),
            format!("--memory={}", self.docker_memory),
            format!("--memory-swap={}", self.docker_memory),
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
        Ok(managed_server(self.docker_command.clone(), args, 90))
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

    pub fn prune_idle_managed_browsers(&self) -> AppResult<usize> {
        self.browser_mcp.prune_idle_host_browsers()
    }

    pub(super) async fn managed_browser_mcp_view(&self) -> Option<CodexMcpServerView> {
        if !self.browser_mcp.enabled {
            return None;
        }
        let host_available = self.browser_mcp.uses_host();
        let mut enabled = host_available;
        let mut disabled_reason = None;
        let command = if host_available {
            self.browser_mcp
                .host_mcp_command
                .as_ref()
                .map(|path| path.to_string_lossy().into_owned())
        } else {
            let status = timeout(
                Duration::from_secs(3),
                Command::new(&self.browser_mcp.docker_command)
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
                disabled_reason =
                    Some("未找到宿主机 Chrome/npx，Docker 兜底浏览器也尚未准备完成".into());
            }
            Some(self.browser_mcp.docker_command.clone())
        };
        Some(CodexMcpServerView {
            name: MANAGED_BROWSER_MCP_NAME.into(),
            transport: CODEX_MCP_TRANSPORT_STDIO.into(),
            enabled,
            auth_status: Some("unsupported".into()),
            address: host_available.then(|| "由本地 Trigger 设备共享宿主机浏览器".into()),
            command,
            argument_count: 0,
            bearer_token_env_var: None,
            startup_timeout_sec: Some(if host_available { 45 } else { 90 }),
            tool_timeout_sec: Some(180),
            disabled_reason,
            configured_by_user: false,
            managed_by_relay: true,
        })
    }
}

fn managed_server(
    command: String,
    args: Vec<String>,
    startup_timeout_sec: u64,
) -> ManagedCodexMcpServer {
    let tool_approval_modes = ["navigate_page", "new_page", "upload_file"]
        .into_iter()
        .map(|tool| (tool.to_string(), "prompt".to_string()))
        .collect();
    ManagedCodexMcpServer {
        name: MANAGED_BROWSER_MCP_NAME.into(),
        command,
        args,
        env: BTreeMap::from([
            ("CHROME_DEVTOOLS_MCP_NO_UPDATE_CHECKS".into(), "1".into()),
            ("CHROME_DEVTOOLS_MCP_NO_USAGE_STATISTICS".into(), "1".into()),
        ]),
        disabled_plugin_ids: vec![
            "browser@openai-bundled".into(),
            "chrome@openai-bundled".into(),
        ],
        required: false,
        startup_timeout_sec: Some(startup_timeout_sec),
        tool_timeout_sec: Some(180),
        default_tools_approval_mode: "approve".into(),
        tool_approval_modes,
        prompt_hint: None,
    }
}

pub(super) fn host_mcp_args(command: &Path, endpoint: &str) -> Vec<String> {
    let mut args = Vec::new();
    if command
        .file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.to_ascii_lowercase().starts_with("npx"))
    {
        args.extend(["--yes".into(), DEFAULT_BROWSER_MCP_PACKAGE.into()]);
    }
    args.extend([
        format!("--browserUrl={endpoint}"),
        "--experimentalPageIdRouting".into(),
        "--no-usage-statistics".into(),
        "--no-performance-crux".into(),
        "--allowUnrestrictedPaths".into(),
    ]);
    args
}

fn parse_enabled(value: &str) -> bool {
    !matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "0" | "false" | "no" | "off"
    )
}

fn configured_or_discovered_executable(variable: &str, candidates: &[&str]) -> Option<PathBuf> {
    std::env::var_os(variable)
        .filter(|value| !value.is_empty())
        .and_then(|value| executable_in_path(&value.to_string_lossy()))
        .or_else(|| {
            candidates
                .iter()
                .find_map(|candidate| executable_in_path(candidate))
        })
}

fn configured_or_discovered_browser() -> Option<PathBuf> {
    if let Some(configured) = std::env::var_os("RELAY_CHROME_EXECUTABLE")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
    {
        return executable_in_path(&configured.to_string_lossy());
    }
    let fixed = [
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        "/Applications/Chromium.app/Contents/MacOS/Chromium",
        "/Applications/Google Chrome Canary.app/Contents/MacOS/Google Chrome Canary",
        "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
        "C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe",
    ];
    let fixed_browser = fixed
        .into_iter()
        .map(PathBuf::from)
        .find(|path| path.is_file());
    if fixed_browser.is_some() {
        return fixed_browser;
    }
    for root in ["LOCALAPPDATA", "PROGRAMFILES", "PROGRAMFILES(X86)"] {
        if let Some(path) = std::env::var_os(root)
            .map(PathBuf::from)
            .map(|path| path.join("Google/Chrome/Application/chrome.exe"))
            .filter(|path| path.is_file())
        {
            return Some(path);
        }
    }
    [
        "google-chrome",
        "google-chrome-stable",
        "chromium",
        "chromium-browser",
        "chrome",
        "chrome.exe",
    ]
    .into_iter()
    .find_map(executable_in_path)
}

fn executable_in_path(name: &str) -> Option<PathBuf> {
    let candidate = Path::new(name);
    if candidate.components().count() > 1 {
        return candidate.is_file().then(|| candidate.to_path_buf());
    }
    let direct = std::env::var_os("PATH")
        .into_iter()
        .flat_map(|path| std::env::split_paths(&path).collect::<Vec<_>>())
        .map(|directory| directory.join(name))
        .find(|path| path.is_file());
    if direct.is_some() || !cfg!(windows) {
        return direct;
    }
    let extensions = std::env::var_os("PATHEXT")
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|| ".COM;.EXE;.BAT;.CMD".into());
    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|path| std::env::split_paths(&path).collect::<Vec<_>>())
        .flat_map(|directory| {
            extensions
                .split(';')
                .map(move |extension| directory.join(format!("{name}{extension}")))
                .collect::<Vec<_>>()
        })
        .find(|path| path.is_file())
}

fn reserve_loopback_port() -> AppResult<u16> {
    TcpListener::bind(("127.0.0.1", 0))
        .and_then(|listener| listener.local_addr())
        .map(|address| address.port())
        .map_err(|error| AppError::Internal(format!("cannot reserve browser port: {error}")))
}

fn browser_endpoint_ready(endpoint: &str) -> bool {
    let Some(port) = endpoint
        .rsplit(':')
        .next()
        .and_then(|value| value.parse().ok())
    else {
        return false;
    };
    let Ok(mut stream) = TcpStream::connect_timeout(
        &(std::net::Ipv4Addr::LOCALHOST, port).into(),
        Duration::from_millis(250),
    ) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_millis(250)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(250)));
    if stream
        .write_all(b"GET /json/version HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .is_err()
    {
        return false;
    }
    let mut response = [0_u8; 256];
    stream
        .read(&mut response)
        .ok()
        .is_some_and(|read| read > 0 && response[..read].starts_with(b"HTTP/1.1 200"))
}

fn agent_browser_page_url(agent_id: Uuid) -> String {
    format!("about:blank#relay-agent-{agent_id}")
}

fn browser_debug_json(endpoint: &str, method: &str, path: &str) -> AppResult<Value> {
    let port = endpoint
        .rsplit(':')
        .next()
        .and_then(|value| value.parse::<u16>().ok())
        .ok_or_else(|| AppError::Internal("managed browser endpoint is invalid".into()))?;
    let mut stream = TcpStream::connect_timeout(
        &(std::net::Ipv4Addr::LOCALHOST, port).into(),
        Duration::from_secs(2),
    )
    .map_err(|error| AppError::Internal(format!("cannot connect to managed browser: {error}")))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|error| AppError::Internal(format!("cannot configure browser read: {error}")))?;
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .map_err(|error| AppError::Internal(format!("cannot configure browser write: {error}")))?;
    write!(
        stream,
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n"
    )
    .map_err(|error| AppError::Internal(format!("cannot request managed browser: {error}")))?;

    let mut response = Vec::new();
    let mut chunk = [0_u8; 8192];
    loop {
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(read) => {
                response.extend_from_slice(&chunk[..read]);
                if response.len() as u64 > MAX_BROWSER_DEBUG_RESPONSE_BYTES {
                    return Err(AppError::Internal(
                        "managed browser response exceeded the safe limit".into(),
                    ));
                }
                if browser_http_response_is_complete(&response) {
                    break;
                }
            }
            Err(error)
                if !response.is_empty()
                    && matches!(
                        error.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) =>
            {
                break;
            }
            Err(error) => {
                return Err(AppError::Internal(format!(
                    "cannot read managed browser: {error}"
                )))
            }
        }
    }
    let separator = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| AppError::Internal("managed browser returned invalid HTTP".into()))?;
    let headers = String::from_utf8_lossy(&response[..separator]);
    if !headers
        .lines()
        .next()
        .is_some_and(|line| line.contains(" 200 "))
    {
        return Err(AppError::Internal(format!(
            "managed browser request failed: {}",
            headers.lines().next().unwrap_or("unknown response")
        )));
    }
    serde_json::from_slice(&response[separator + 4..]).map_err(|error| {
        AppError::Internal(format!("managed browser returned invalid JSON: {error}"))
    })
}

fn browser_http_response_is_complete(response: &[u8]) -> bool {
    let Some(separator) = response.windows(4).position(|window| window == b"\r\n\r\n") else {
        return false;
    };
    let headers = String::from_utf8_lossy(&response[..separator]);
    let Some(content_length) = headers.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.eq_ignore_ascii_case("content-length")
            .then(|| value.trim().parse::<usize>().ok())
            .flatten()
    }) else {
        return false;
    };
    response.len().saturating_sub(separator + 4) >= content_length
}

fn read_reusable_endpoint(profile: &Path) -> Option<String> {
    let endpoint = std::fs::read_to_string(profile.join(HOST_BROWSER_ENDPOINT_FILE)).ok()?;
    let endpoint = endpoint.trim().to_string();
    for _ in 0..3 {
        if browser_endpoint_ready(&endpoint) {
            return Some(endpoint);
        }
        thread::sleep(Duration::from_millis(100));
    }
    None
}

fn create_browser_profile(path: &Path) -> AppResult<()> {
    std::fs::create_dir_all(path).map_err(|error| {
        AppError::Internal(format!("cannot create managed browser profile: {error}"))
    })?;
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
    let _ = std::fs::remove_file(profile.join(HOST_BROWSER_ENDPOINT_FILE));
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

#[cfg(test)]
mod idle_cleanup_tests {
    use super::*;

    #[test]
    fn idle_browser_cleanup_keeps_recent_profiles() {
        let mut config = BrowserMcpConfig::disabled();
        config.host_browser_idle_timeout = Duration::from_secs(60);
        let now = std::time::Instant::now();
        {
            let mut pool = config.pool.lock().expect("browser pool");
            *pool = Some(HostBrowserProcess {
                endpoint: "http://127.0.0.1:19001".into(),
                child: None,
                last_used_at: now - Duration::from_secs(61),
                agent_pages: HashMap::new(),
            });
        }

        assert_eq!(config.prune_idle_host_browsers().expect("prune"), 1);
        assert!(config.pool.lock().expect("browser pool").is_none());
        {
            let mut pool = config.pool.lock().expect("browser pool");
            *pool = Some(HostBrowserProcess {
                endpoint: "http://127.0.0.1:19002".into(),
                child: None,
                last_used_at: now,
                agent_pages: HashMap::new(),
            });
        }
        assert_eq!(config.prune_idle_host_browsers().expect("prune"), 0);
        let pool = config.pool.lock().expect("browser pool");
        assert!(pool.is_some());
    }
}
