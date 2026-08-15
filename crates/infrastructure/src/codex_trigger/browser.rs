use super::*;
use std::{
    collections::HashSet,
    process::{Child, Command as StdCommand},
    thread,
};

mod debug_client;
mod proxy;
mod support;

use debug_client::*;
use proxy::*;
pub(super) use support::host_mcp_args;
use support::{
    browser_container_user, configured_or_discovered_browser, configured_or_discovered_executable,
    create_browser_artifacts, create_browser_profile, managed_server, parse_enabled,
    read_reusable_endpoint, remove_stale_chromium_runtime_files, reserve_loopback_port,
};

pub(super) const MANAGED_BROWSER_MCP_NAME: &str = "chrome-devtools";
const DEFAULT_BROWSER_MCP_IMAGE: &str = "relay/chrome-devtools-mcp:1.6.0";
pub(super) const DEFAULT_BROWSER_MCP_PACKAGE: &str = "chrome-devtools-mcp@1.6.0";
const BROWSER_PROFILE_CONTAINER_PATH: &str = "/relay-browser-profile";
const HOST_BROWSER_ENDPOINT_FILE: &str = ".relay-devtools-endpoint";
const HOST_BROWSER_START_TIMEOUT: Duration = Duration::from_secs(12);
const MAX_BROWSER_DEBUG_RESPONSE_BYTES: u64 = 1024 * 1024;
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
    proxy: Option<BrowserProxyRuntime>,
    last_used_at: std::time::Instant,
    agent_pages: HashMap<Uuid, AgentBrowserPage>,
}

#[derive(Debug, Clone)]
struct AgentBrowserPage {
    page_id: String,
    last_used_at: Arc<Mutex<std::time::Instant>>,
}

fn disposable_browser_page_url(url: &str) -> bool {
    matches!(
        url,
        "about:blank" | "chrome://new-tab-page/" | "chrome://newtab/"
    )
}

impl Drop for HostBrowserProcess {
    fn drop(&mut self) {
        self.proxy.take();
        if let Some(child) = self.child.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn reclaimable_agent_pages(
    process: &HostBrowserProcess,
    active_agent_ids: &HashSet<Uuid>,
    now: std::time::Instant,
    idle_timeout: Duration,
    max_idle_pages: usize,
) -> Vec<(Uuid, String)> {
    let mut inactive_pages = process
        .agent_pages
        .iter()
        .filter(|(agent_id, _)| !active_agent_ids.contains(agent_id))
        .map(|(agent_id, page)| {
            let last_used_at = page
                .last_used_at
                .lock()
                .map(|last_used_at| *last_used_at)
                .unwrap_or(now);
            (*agent_id, page.page_id.clone(), last_used_at)
        })
        .collect::<Vec<_>>();
    inactive_pages.sort_by_key(|(agent_id, _, last_used_at)| (*last_used_at, *agent_id));
    let overflow = inactive_pages.len().saturating_sub(max_idle_pages);
    inactive_pages
        .into_iter()
        .enumerate()
        .filter(|(index, (_, _, last_used_at))| {
            *index < overflow || now.saturating_duration_since(*last_used_at) >= idle_timeout
        })
        .map(|(_, (agent_id, page_id, _))| (agent_id, page_id))
        .collect()
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
    host_page_idle_timeout: Duration,
    host_max_idle_pages: usize,
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
            host_page_idle_timeout: Duration::from_secs(15 * 60),
            host_max_idle_pages: 2,
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
        let host_page_idle_timeout = Duration::from_secs(
            std::env::var("RELAY_CHROME_PAGE_IDLE_TIMEOUT_SECONDS")
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
                .map(|value| value.clamp(300, 86_400))
                .unwrap_or(15 * 60),
        );
        let host_max_idle_pages = std::env::var("RELAY_CHROME_MAX_IDLE_PAGES")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .map(|value| value.min(32))
            .unwrap_or(2);
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
            host_page_idle_timeout,
            host_max_idle_pages,
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
            host_page_idle_timeout: Duration::from_secs(15 * 60),
            host_max_idle_pages: 2,
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
            host_page_idle_timeout: Duration::from_secs(15 * 60),
            host_max_idle_pages: 2,
            pool: Arc::new(Mutex::new(None)),
        };
        config
            .host_browser_executable
            .as_deref()
            .is_some_and(test_safe_headless_browser)
            .then_some(config)
    }

    #[cfg(test)]
    pub(super) fn host_page_urls_for_test(&self) -> AppResult<Vec<String>> {
        let endpoint = self
            .pool
            .lock()
            .map_err(|_| {
                AppError::Internal("managed browser process pool lock was poisoned".into())
            })?
            .as_ref()
            .map(|process| process.endpoint.clone())
            .ok_or_else(|| AppError::Internal("managed browser process is not running".into()))?;
        Ok(browser_debug_json(&endpoint, "GET", "/json/list")?
            .as_array()
            .into_iter()
            .flatten()
            .filter(|page| page.get("type").and_then(Value::as_str) == Some("page"))
            .filter_map(|page| page.get("url").and_then(Value::as_str).map(str::to_string))
            .collect())
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
        run_token: &str,
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
            return self.host_server(agent_id, &workspace, run_token).map(Some);
        }
        self.docker_server(company_id, agent_id, project_id, &workspace)
            .map(Some)
    }

    fn host_server(
        &self,
        agent_id: Uuid,
        workspace: &Path,
        run_token: &str,
    ) -> AppResult<ManagedCodexMcpServer> {
        let endpoint = self.ensure_host_browser(agent_id)?;
        let page = self.ensure_agent_browser_page(&endpoint, agent_id)?;
        let proxy_url = self.ensure_host_proxy(&endpoint)?;
        self.close_unmanaged_disposable_pages(&endpoint);
        let mut server = managed_server(String::new(), Vec::new(), 20);
        server.url = Some(proxy_url);
        server.env.clear();
        server.env_http_headers.insert(
            BROWSER_PROXY_TOKEN_HEADER.into(),
            DEFAULT_RUN_TOKEN_ENV.into(),
        );
        server.prompt_hint = Some(
            "此 Trigger 设备上的所有 Agent 共用一个 Chrome 和一个 Chrome DevTools MCP。Relay 已将你的工具调用强制绑定到专属标签页；无需传 pageId，也无法读取或操作其他 Agent 的标签页。"
                .into(),
        );
        let mut pool = self.pool.lock().map_err(|_| {
            AppError::Internal("managed browser process pool lock was poisoned".into())
        })?;
        let process = pool.as_mut().ok_or_else(|| {
            AppError::Internal("managed browser process disappeared during proxy setup".into())
        })?;
        let proxy = process.proxy.as_ref().ok_or_else(|| {
            AppError::Internal("managed browser MCP proxy disappeared during setup".into())
        })?;
        proxy.register(
            run_token,
            BrowserProxyGrant::new(
                agent_id,
                endpoint,
                page.page_id,
                page.last_used_at,
                workspace.to_path_buf(),
            ),
        )?;
        Ok(server)
    }

    fn ensure_host_proxy(&self, endpoint: &str) -> AppResult<String> {
        let mut pool = self.pool.lock().map_err(|_| {
            AppError::Internal("managed browser process pool lock was poisoned".into())
        })?;
        let process = pool.as_mut().ok_or_else(|| {
            AppError::Internal("managed browser process disappeared during proxy startup".into())
        })?;
        if process
            .proxy
            .as_ref()
            .is_some_and(BrowserProxyRuntime::is_running)
        {
            return Ok(process
                .proxy
                .as_ref()
                .expect("running proxy exists")
                .url()
                .to_string());
        }
        process.proxy.take();
        let command = self
            .host_mcp_command
            .as_ref()
            .expect("host mode checks the MCP command")
            .clone();
        let proxy = BrowserProxyRuntime::start(command.clone(), host_mcp_args(&command, endpoint))?;
        let url = proxy.url().to_string();
        process.proxy = Some(proxy);
        Ok(url)
    }

    fn close_unmanaged_disposable_pages(&self, endpoint: &str) {
        let managed_page_ids = match self.pool.lock() {
            Ok(pool) => pool
                .as_ref()
                .map(|process| {
                    process
                        .agent_pages
                        .values()
                        .map(|page| page.page_id.clone())
                        .collect::<HashSet<_>>()
                })
                .unwrap_or_default(),
            Err(_) => return,
        };
        let Ok(pages) = browser_debug_json(endpoint, "GET", "/json/list") else {
            return;
        };
        for page_id in pages
            .as_array()
            .into_iter()
            .flatten()
            .filter(|page| {
                page.get("type").and_then(Value::as_str) == Some("page")
                    && page
                        .get("url")
                        .and_then(Value::as_str)
                        .is_some_and(disposable_browser_page_url)
            })
            .filter_map(|page| page.get("id").and_then(Value::as_str))
            .filter(|page_id| !managed_page_ids.contains(*page_id))
        {
            if let Err(error) = close_browser_page(endpoint, page_id) {
                tracing::debug!(
                    page_id,
                    error = %error,
                    "could not close an unused browser startup page"
                );
            }
        }
    }

    fn ensure_host_browser(&self, first_agent_id: Uuid) -> AppResult<String> {
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
                proxy: None,
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
            // Mark the browser's initial page for the first Agent instead of
            // creating a second blank renderer immediately after startup.
            .arg(agent_browser_page_url(first_agent_id))
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
                    proxy: None,
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

    fn ensure_agent_browser_page(
        &self,
        endpoint: &str,
        agent_id: Uuid,
    ) -> AppResult<AgentBrowserPage> {
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
        if let Some(page) = process.agent_pages.get_mut(&agent_id).filter(|page| {
            browser_pages
                .iter()
                .any(|item| item.get("id").and_then(Value::as_str) == Some(page.page_id.as_str()))
        }) {
            if let Ok(mut last_used_at) = page.last_used_at.lock() {
                *last_used_at = std::time::Instant::now();
            }
            return Ok(page.clone());
        }
        if let Some(page_id) = browser_pages.iter().find_map(|page| {
            (page.get("url").and_then(Value::as_str) == Some(page_url.as_str()))
                .then(|| page.get("id").and_then(Value::as_str))
                .flatten()
        }) {
            validate_browser_page_id(page_id)?;
            let page = AgentBrowserPage {
                page_id: page_id.to_string(),
                last_used_at: Arc::new(Mutex::new(std::time::Instant::now())),
            };
            process.agent_pages.insert(agent_id, page.clone());
            return Ok(page);
        }

        let encoded_url = page_url.replace('#', "%23");
        let page = browser_debug_json(endpoint, "PUT", &format!("/json/new?{encoded_url}"))?;
        let page_id = page
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| AppError::Internal("managed browser did not return a page ID".into()))?;
        validate_browser_page_id(page_id)?;
        let page = AgentBrowserPage {
            page_id: page_id.to_string(),
            last_used_at: Arc::new(Mutex::new(std::time::Instant::now())),
        };
        process.agent_pages.insert(agent_id, page.clone());
        Ok(page)
    }

    fn prune_idle_host_browsers(
        &self,
        active_agent_ids: &HashSet<Uuid>,
        aggressive: bool,
    ) -> AppResult<(usize, usize)> {
        let mut pool = self.pool.lock().map_err(|_| {
            AppError::Internal("managed browser process pool lock was poisoned".into())
        })?;
        let now = std::time::Instant::now();
        let browser_idle_timeout = if aggressive {
            Duration::ZERO
        } else {
            self.host_browser_idle_timeout
        };
        let page_idle_timeout = if aggressive {
            Duration::ZERO
        } else {
            self.host_page_idle_timeout
        };
        let expired = pool.as_ref().is_some_and(|process| {
            now.saturating_duration_since(process.last_used_at) >= browser_idle_timeout
                && !process
                    .agent_pages
                    .keys()
                    .any(|agent_id| active_agent_ids.contains(agent_id))
        });
        if expired {
            pool.take();
            return Ok((1, 0));
        }
        let Some(process) = pool.as_ref() else {
            return Ok((0, 0));
        };
        let endpoint = process.endpoint.clone();
        let expired_pages = reclaimable_agent_pages(
            process,
            active_agent_ids,
            now,
            page_idle_timeout,
            self.host_max_idle_pages,
        );
        drop(pool);
        let mut closed_pages = 0;
        for (agent_id, page_id) in expired_pages {
            match close_browser_page(&endpoint, &page_id) {
                Ok(()) => {
                    let mut pool = self.pool.lock().map_err(|_| {
                        AppError::Internal("managed browser process pool lock was poisoned".into())
                    })?;
                    if let Some(process) =
                        pool.as_mut().filter(|process| process.endpoint == endpoint)
                    {
                        let still_same_page = process
                            .agent_pages
                            .get(&agent_id)
                            .is_some_and(|page| page.page_id == page_id);
                        if still_same_page {
                            process.agent_pages.remove(&agent_id);
                            if let Some(proxy) = process.proxy.as_ref() {
                                proxy.revoke_agents(std::iter::once(agent_id));
                            }
                        }
                    }
                    closed_pages += 1;
                }
                Err(error) => tracing::warn!(
                    agent_id = %agent_id,
                    error = %error,
                    "failed to close an idle managed browser page; will retry"
                ),
            }
        }
        Ok((0, closed_pages))
    }

    fn revoke_run_token(&self, run_token: &str) {
        if let Ok(pool) = self.pool.lock() {
            if let Some(proxy) = pool.as_ref().and_then(|process| process.proxy.as_ref()) {
                proxy.revoke_run_token(run_token);
            }
        }
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

#[cfg(test)]
fn test_safe_headless_browser(path: &Path) -> bool {
    std::env::var("RELAY_RUN_REAL_BROWSER_TESTS")
        .ok()
        .is_some_and(|value| parse_enabled(&value))
        || path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.to_ascii_lowercase().contains("headless"))
}

impl CodexTriggerRunner {
    pub fn managed_browser_mcp_server(
        &self,
        company_id: Uuid,
        agent_id: Uuid,
        project_id: Uuid,
        workspace: &Path,
        run_token: &str,
    ) -> AppResult<Option<ManagedCodexMcpServer>> {
        self.browser_mcp
            .server(company_id, agent_id, project_id, workspace, run_token)
    }

    pub fn prune_idle_managed_browsers(
        &self,
        active_agent_ids: &HashSet<Uuid>,
        aggressive: bool,
    ) -> AppResult<(usize, usize)> {
        self.browser_mcp
            .prune_idle_host_browsers(active_agent_ids, aggressive)
    }

    pub fn revoke_managed_browser_run_token(&self, run_token: &str) {
        self.browser_mcp.revoke_run_token(run_token);
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
            transport: if host_available {
                CODEX_MCP_TRANSPORT_HTTP.into()
            } else {
                CODEX_MCP_TRANSPORT_STDIO.into()
            },
            enabled,
            auth_status: Some(
                if host_available {
                    "active"
                } else {
                    "unsupported"
                }
                .into(),
            ),
            address: host_available
                .then(|| "由本地 Trigger 设备共享浏览器与 MCP；按 Agent 运行令牌隔离".into()),
            command: (!host_available).then_some(command).flatten(),
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

#[cfg(test)]
mod idle_cleanup_tests;
