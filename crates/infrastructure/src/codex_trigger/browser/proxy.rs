use super::*;
use std::{
    borrow::Cow,
    collections::HashMap,
    process::Stdio,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex as StdMutex, RwLock,
    },
    thread::JoinHandle,
};

use axum::Router;
use http::request::Parts;
use rmcp::{
    model::{
        CallToolRequestParams, CallToolResult, ContentBlock, Implementation, JsonObject,
        ListToolsResult, PaginatedRequestParams, ServerCapabilities, ServerInfo, Tool,
    },
    service::RequestContext,
    transport::{
        streamable_http_server::{
            session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
        },
        TokioChildProcess,
    },
    ErrorData, Peer, RoleClient, RoleServer, ServerHandler, ServiceExt,
};

pub(super) const BROWSER_PROXY_TOKEN_HEADER: &str = "x-agent-run-token";
const BROWSER_PROXY_START_TIMEOUT: Duration = Duration::from_secs(20);
const HIDDEN_PAGE_TOOLS: [&str; 2] = ["close_page", "select_page"];
const PATH_ARGUMENTS: [&str; 8] = [
    "baseFilePath",
    "currentFilePath",
    "filePath",
    "outputDirPath",
    "path",
    "requestFilePath",
    "responseFilePath",
    "userDataDir",
];

#[derive(Debug, Clone)]
pub(super) struct BrowserProxyGrant {
    pub agent_id: Uuid,
    pub browser_endpoint: String,
    pub cdp_page_id: String,
    pub marker_url: String,
    pub workspace: PathBuf,
    upstream_page_id: Arc<StdMutex<Option<u64>>>,
}

impl BrowserProxyGrant {
    pub(super) fn new(
        agent_id: Uuid,
        browser_endpoint: String,
        cdp_page_id: String,
        workspace: PathBuf,
    ) -> Self {
        Self {
            agent_id,
            browser_endpoint,
            cdp_page_id,
            marker_url: agent_browser_page_url(agent_id),
            workspace,
            upstream_page_id: Arc::new(StdMutex::new(None)),
        }
    }
}

pub(super) struct BrowserProxyRuntime {
    url: String,
    grants: Arc<RwLock<HashMap<String, BrowserProxyGrant>>>,
    healthy: Arc<AtomicBool>,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<JoinHandle<()>>,
}

impl std::fmt::Debug for BrowserProxyRuntime {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BrowserProxyRuntime")
            .field("url", &self.url)
            .field("running", &self.is_running())
            .finish()
    }
}

impl BrowserProxyRuntime {
    pub(super) fn start(command: PathBuf, args: Vec<String>) -> AppResult<Self> {
        let grants = Arc::new(RwLock::new(HashMap::new()));
        let thread_grants = grants.clone();
        let healthy = Arc::new(AtomicBool::new(false));
        let thread_healthy = healthy.clone();
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        let thread = std::thread::Builder::new()
            .name("relay-browser-mcp-proxy".into())
            .spawn(move || {
                let runtime = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime,
                    Err(error) => {
                        let _ = ready_tx.send(Err(format!(
                            "cannot create browser MCP proxy runtime: {error}"
                        )));
                        return;
                    }
                };
                runtime.block_on(run_proxy_server(
                    command,
                    args,
                    thread_grants,
                    thread_healthy,
                    ready_tx,
                    shutdown_rx,
                ));
            })
            .map_err(|error| {
                AppError::Internal(format!("cannot start browser MCP proxy thread: {error}"))
            })?;
        let port = match ready_rx.recv_timeout(BROWSER_PROXY_START_TIMEOUT) {
            Ok(Ok(port)) => port,
            Ok(Err(error)) => {
                let _ = thread.join();
                return Err(AppError::Internal(error));
            }
            Err(error) => {
                return Err(AppError::Internal(format!(
                    "browser MCP proxy did not start in time: {error}"
                )))
            }
        };
        Ok(Self {
            url: format!("http://127.0.0.1:{port}/mcp"),
            grants,
            healthy,
            shutdown: Some(shutdown_tx),
            thread: Some(thread),
        })
    }

    pub(super) fn url(&self) -> &str {
        &self.url
    }

    pub(super) fn is_running(&self) -> bool {
        self.healthy.load(Ordering::Acquire)
            && self
                .thread
                .as_ref()
                .is_some_and(|thread| !thread.is_finished())
    }

    pub(super) fn register(&self, run_token: &str, grant: BrowserProxyGrant) -> AppResult<()> {
        let mut grants = self
            .grants
            .write()
            .map_err(|_| AppError::Internal("browser MCP proxy grant lock was poisoned".into()))?;
        grants.insert(run_token.to_string(), grant);
        Ok(())
    }

    pub(super) fn revoke_agents(&self, agent_ids: impl Iterator<Item = Uuid>) {
        let agent_ids = agent_ids.collect::<HashSet<_>>();
        if agent_ids.is_empty() {
            return;
        }
        if let Ok(mut grants) = self.grants.write() {
            grants.retain(|_, grant| !agent_ids.contains(&grant.agent_id));
        }
    }

    pub(super) fn revoke_run_token(&self, run_token: &str) {
        if let Ok(mut grants) = self.grants.write() {
            grants.remove(run_token);
        }
    }
}

impl Drop for BrowserProxyRuntime {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[derive(Clone)]
struct BrowserProxyHandler {
    upstream: Peer<RoleClient>,
    tools: Arc<Vec<Tool>>,
    upstream_tools: Arc<HashMap<String, Tool>>,
    grants: Arc<RwLock<HashMap<String, BrowserProxyGrant>>>,
    calls: Arc<tokio::sync::Mutex<()>>,
    healthy: Arc<AtomicBool>,
}

impl ServerHandler for BrowserProxyHandler {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(
                Implementation::new("relay-chrome-devtools-proxy", env!("CARGO_PKG_VERSION"))
                    .with_title("Relay Chrome DevTools"),
            )
            .with_instructions(
                "Relay routes every browser tool call to this Agent's dedicated page. Other Agent pages are never exposed. Use list_pages only when the current page identity is needed.",
            )
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        self.authorize(&context)?;
        Ok(ListToolsResult::with_all_items((*self.tools).clone()))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let grant = self.authorize(&context)?;
        let _call = self.calls.lock().await;
        match self.call_tool_serialized(request, &grant).await {
            Ok(result) => Ok(result),
            Err(error) => {
                if self.upstream.is_transport_closed() {
                    self.healthy.store(false, Ordering::Release);
                }
                tracing::warn!(
                    agent_id = %grant.agent_id,
                    error = %error,
                    "shared browser MCP call failed"
                );
                Ok(CallToolResult::error(vec![ContentBlock::text(format!(
                    "Browser tool failed: {error}"
                ))]))
            }
        }
    }
}

impl BrowserProxyHandler {
    fn authorize(
        &self,
        context: &RequestContext<RoleServer>,
    ) -> Result<BrowserProxyGrant, ErrorData> {
        let parts = context.extensions.get::<Parts>().ok_or_else(|| {
            ErrorData::invalid_request("managed browser request metadata is missing", None)
        })?;
        let token = parts
            .headers
            .get(BROWSER_PROXY_TOKEN_HEADER)
            .and_then(|value| value.to_str().ok())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ErrorData::invalid_request("managed browser authentication is missing", None)
            })?;
        self.grants
            .read()
            .map_err(|_| ErrorData::internal_error("browser grant lock failed", None))?
            .get(token)
            .cloned()
            .ok_or_else(|| {
                ErrorData::invalid_request("managed browser authentication is invalid", None)
            })
    }

    async fn call_tool_serialized(
        &self,
        mut request: CallToolRequestParams,
        grant: &BrowserProxyGrant,
    ) -> Result<CallToolResult, String> {
        if self.upstream.is_transport_closed() {
            return Err(
                "shared Chrome MCP transport is unavailable; retry the work session".into(),
            );
        }
        if HIDDEN_PAGE_TOOLS.contains(&request.name.as_ref()) {
            return Ok(CallToolResult::error(vec![ContentBlock::text(
                "Relay keeps one dedicated browser page for this Agent; selecting or closing other pages is disabled.",
            )]));
        }
        let page = self.resolve_page(grant).await?;
        if request.name == "list_pages" {
            return Ok(single_page_result(&page));
        }
        normalize_path_arguments(&mut request.arguments, &grant.workspace)?;
        if request.name == "new_page" {
            request = rewrite_new_page(request, page.id)?;
        } else {
            self.select_page(page.id).await?;
            if self.tool_accepts_page_id(request.name.as_ref()) {
                request
                    .arguments
                    .get_or_insert_with(JsonObject::new)
                    .insert("pageId".into(), serde_json::json!(page.id));
            }
        }
        let result = self
            .upstream
            .call_tool(request)
            .await
            .map_err(|error| error.to_string())?;
        Ok(sanitize_result(result, &page))
    }

    fn tool_accepts_page_id(&self, name: &str) -> bool {
        self.upstream_tools
            .get(name)
            .and_then(|tool| tool.input_schema.get("properties"))
            .and_then(serde_json::Value::as_object)
            .is_some_and(|properties| properties.contains_key("pageId"))
    }

    async fn select_page(&self, page_id: u64) -> Result<(), String> {
        self.upstream
            .call_tool(CallToolRequestParams::new("select_page").with_arguments(
                JsonObject::from_iter([("pageId".into(), serde_json::json!(page_id))]),
            ))
            .await
            .map_err(|error| error.to_string())?
            .is_error
            .filter(|is_error| *is_error)
            .map_or(Ok(()), |_| {
                Err("cannot select the Agent browser page".into())
            })
    }

    async fn resolve_page(&self, grant: &BrowserProxyGrant) -> Result<ResolvedPage, String> {
        let result = self
            .upstream
            .call_tool(CallToolRequestParams::new("list_pages").with_arguments(JsonObject::new()))
            .await
            .map_err(|error| error.to_string())?;
        let mut pages = structured_pages(&result);
        if pages.is_empty() {
            pages = text_pages(&result);
        }
        let cached_id = grant
            .upstream_page_id
            .lock()
            .map_err(|_| "browser page cache lock failed".to_string())?
            .to_owned();
        let mut page = cached_id
            .and_then(|id| pages.iter().find(|page| page.id == id).cloned())
            .or_else(|| {
                pages
                    .iter()
                    .find(|page| page.url == grant.marker_url)
                    .cloned()
            });
        if page.is_none() {
            let endpoint = grant.browser_endpoint.clone();
            let cdp_page_id = grant.cdp_page_id.clone();
            let current_url = tokio::task::spawn_blocking(move || {
                browser_debug_json(&endpoint, "GET", "/json/list")
                    .ok()
                    .and_then(|value| value.as_array().cloned())
                    .and_then(|items| {
                        items.into_iter().find_map(|item| {
                            (item.get("id").and_then(Value::as_str) == Some(cdp_page_id.as_str()))
                                .then(|| {
                                    item.get("url").and_then(Value::as_str).map(str::to_string)
                                })
                                .flatten()
                        })
                    })
            })
            .await
            .ok()
            .flatten();
            if let Some(current_url) = current_url {
                let matching = pages
                    .iter()
                    .filter(|page| page.url == current_url)
                    .cloned()
                    .collect::<Vec<_>>();
                if matching.len() == 1 {
                    page = matching.into_iter().next();
                }
            }
        }
        let page = page.ok_or_else(|| {
            "Relay could not resolve this Agent's dedicated browser page; retry the work session"
                .to_string()
        })?;
        *grant
            .upstream_page_id
            .lock()
            .map_err(|_| "browser page cache lock failed".to_string())? = Some(page.id);
        Ok(page)
    }
}

#[derive(Debug, Clone)]
struct ResolvedPage {
    id: u64,
    url: String,
    title: String,
}

async fn run_proxy_server(
    command: PathBuf,
    args: Vec<String>,
    grants: Arc<RwLock<HashMap<String, BrowserProxyGrant>>>,
    healthy: Arc<AtomicBool>,
    ready: mpsc::SyncSender<Result<u16, String>>,
    shutdown: tokio::sync::oneshot::Receiver<()>,
) {
    let mut process = tokio::process::Command::new(&command);
    process
        .args(args)
        .env("CHROME_DEVTOOLS_MCP_NO_UPDATE_CHECKS", "1")
        .env("CHROME_DEVTOOLS_MCP_NO_USAGE_STATISTICS", "1");
    let transport = match TokioChildProcess::builder(process)
        .stderr(Stdio::null())
        .spawn()
        .map(|(transport, _)| transport)
    {
        Ok(transport) => transport,
        Err(error) => {
            let _ = ready.send(Err(format!("cannot start shared Chrome MCP: {error}")));
            return;
        }
    };
    let client = match ().serve(transport).await {
        Ok(client) => client,
        Err(error) => {
            let _ = ready.send(Err(format!("cannot initialize shared Chrome MCP: {error}")));
            return;
        }
    };
    let upstream = client.peer().clone();
    let upstream_tools = match upstream.list_all_tools().await {
        Ok(tools) => tools,
        Err(error) => {
            let _ = ready.send(Err(format!("cannot list shared Chrome MCP tools: {error}")));
            let _ = client.cancel().await;
            return;
        }
    };
    let tools = sanitize_tools(&upstream_tools);
    let handler = BrowserProxyHandler {
        upstream,
        tools: Arc::new(tools),
        upstream_tools: Arc::new(
            upstream_tools
                .into_iter()
                .map(|tool| (tool.name.to_string(), tool))
                .collect(),
        ),
        grants,
        calls: Arc::new(tokio::sync::Mutex::new(())),
        healthy: healthy.clone(),
    };
    let service: StreamableHttpService<BrowserProxyHandler, LocalSessionManager> =
        StreamableHttpService::new(
            move || Ok(handler.clone()),
            Default::default(),
            StreamableHttpServerConfig::default()
                .with_stateful_mode(false)
                .with_json_response(true)
                .with_sse_keep_alive(None)
                .with_allowed_hosts(["127.0.0.1", "localhost"]),
        );
    let listener = match tokio::net::TcpListener::bind("127.0.0.1:0").await {
        Ok(listener) => listener,
        Err(error) => {
            let _ = ready.send(Err(format!("cannot bind browser MCP proxy: {error}")));
            let _ = client.cancel().await;
            return;
        }
    };
    let port = match listener.local_addr() {
        Ok(address) => address.port(),
        Err(error) => {
            let _ = ready.send(Err(format!(
                "cannot inspect browser MCP proxy port: {error}"
            )));
            let _ = client.cancel().await;
            return;
        }
    };
    healthy.store(true, Ordering::Release);
    if ready.send(Ok(port)).is_err() {
        healthy.store(false, Ordering::Release);
        let _ = client.cancel().await;
        return;
    }
    tracing::info!(port, "shared Chrome DevTools MCP proxy listening");
    let app = Router::new().route_service("/mcp", service);
    let result = axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            let _ = shutdown.await;
        })
        .await;
    if let Err(error) = result {
        tracing::warn!(error = %error, "shared browser MCP proxy stopped unexpectedly");
    }
    healthy.store(false, Ordering::Release);
    let _ = client.cancel().await;
}

fn sanitize_tools(tools: &[Tool]) -> Vec<Tool> {
    tools
        .iter()
        .filter(|tool| !HIDDEN_PAGE_TOOLS.contains(&tool.name.as_ref()))
        .cloned()
        .map(|mut tool| {
            let mut schema = (*tool.input_schema).clone();
            if let Some(properties) = schema
                .get_mut("properties")
                .and_then(serde_json::Value::as_object_mut)
            {
                properties.remove("pageId");
            }
            if let Some(required) = schema
                .get_mut("required")
                .and_then(serde_json::Value::as_array_mut)
            {
                required.retain(|value| value.as_str() != Some("pageId"));
            }
            if tool.name == "new_page" {
                tool.description = Some(Cow::Borrowed(
                    "Load a URL in this Agent's dedicated browser page.",
                ));
            }
            tool.input_schema = Arc::new(schema);
            tool
        })
        .collect()
}

fn structured_pages(result: &CallToolResult) -> Vec<ResolvedPage> {
    result
        .structured_content
        .as_ref()
        .and_then(|value| value.get("pages"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|page| {
            Some(ResolvedPage {
                id: page.get("id")?.as_u64()?,
                url: page.get("url")?.as_str()?.to_string(),
                title: page
                    .get("title")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
            })
        })
        .collect()
}

fn text_pages(result: &CallToolResult) -> Vec<ResolvedPage> {
    result
        .content
        .iter()
        .filter_map(|content| match content {
            ContentBlock::Text(text) => Some(text.text.as_str()),
            _ => None,
        })
        .flat_map(|text| {
            let mut in_pages = false;
            text.lines()
                .filter_map(move |line| {
                    if line == "## Pages" {
                        in_pages = true;
                        return None;
                    }
                    if in_pages && line.starts_with("## ") {
                        in_pages = false;
                    }
                    in_pages.then(|| parse_page_line(line)).flatten()
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

fn parse_page_line(line: &str) -> Option<ResolvedPage> {
    let (id, description) = line.split_once(": ")?;
    let id = id.parse().ok()?;
    let description = description
        .strip_suffix(" [selected]")
        .unwrap_or(description);
    let (title, url) = description
        .strip_suffix(')')
        .and_then(|description| description.rsplit_once(" ("))
        .map(|(title, url)| (title.to_string(), url.to_string()))
        .unwrap_or_else(|| (String::new(), description.to_string()));
    Some(ResolvedPage { id, url, title })
}

fn rewrite_new_page(
    request: CallToolRequestParams,
    page_id: u64,
) -> Result<CallToolRequestParams, String> {
    let mut arguments = request.arguments.unwrap_or_default();
    let url = arguments
        .remove("url")
        .ok_or_else(|| "new_page requires a URL".to_string())?;
    let timeout = arguments.remove("timeout");
    let mut rewritten = JsonObject::from_iter([
        ("type".into(), Value::String("url".into())),
        ("url".into(), url),
        ("pageId".into(), serde_json::json!(page_id)),
    ]);
    if let Some(timeout) = timeout {
        rewritten.insert("timeout".into(), timeout);
    }
    Ok(CallToolRequestParams::new("navigate_page").with_arguments(rewritten))
}

fn single_page_result(page: &ResolvedPage) -> CallToolResult {
    let title = if page.title.is_empty() {
        page.url.clone()
    } else {
        format!("{} ({})", page.title, page.url)
    };
    let mut result = CallToolResult::success(vec![ContentBlock::text(format!(
        "## Pages\n{}: {} [selected]",
        page.id, title
    ))]);
    result.structured_content = Some(serde_json::json!({
        "pages": [{
            "id": page.id,
            "url": page.url,
            "title": page.title,
            "selected": true
        }]
    }));
    result
}

fn sanitize_result(mut result: CallToolResult, fallback: &ResolvedPage) -> CallToolResult {
    let mut page = fallback.clone();
    if let Some(structured) = result
        .structured_content
        .as_mut()
        .and_then(Value::as_object_mut)
    {
        if let Some(pages) = structured.get_mut("pages").and_then(Value::as_array_mut) {
            pages.retain(|entry| entry.get("id").and_then(Value::as_u64) == Some(fallback.id));
            if let Some(entry) = pages.first() {
                page.url = entry
                    .get("url")
                    .and_then(Value::as_str)
                    .unwrap_or(&page.url)
                    .to_string();
                page.title = entry
                    .get("title")
                    .and_then(Value::as_str)
                    .unwrap_or(&page.title)
                    .to_string();
            }
        }
        structured.remove("extensionPages");
        structured.remove("extensionServiceWorkers");
    }
    for content in &mut result.content {
        if let ContentBlock::Text(text) = content {
            text.text = replace_pages_section(&text.text, &page);
        }
    }
    result
}

fn replace_pages_section(text: &str, page: &ResolvedPage) -> String {
    let mut output = Vec::<String>::new();
    let mut skipping_pages = false;
    for line in text.lines() {
        if line == "## Pages" {
            skipping_pages = true;
            continue;
        }
        if skipping_pages && line.starts_with("## ") {
            skipping_pages = false;
        }
        if !skipping_pages {
            output.push(line.to_string());
        }
    }
    if text.contains("## Pages") {
        let title = if page.title.is_empty() {
            page.url.clone()
        } else {
            format!("{} ({})", page.title, page.url)
        };
        while output.last().is_some_and(|line| line.is_empty()) {
            output.pop();
        }
        output.push(String::new());
        output.push("## Pages".into());
        output.push(format!("{}: {} [selected]", page.id, title));
    }
    output.join("\n")
}

fn normalize_path_arguments(
    arguments: &mut Option<JsonObject>,
    workspace: &Path,
) -> Result<(), String> {
    let Some(arguments) = arguments.as_mut() else {
        return Ok(());
    };
    for key in PATH_ARGUMENTS {
        let Some(Value::String(raw_path)) = arguments.get_mut(key) else {
            continue;
        };
        let normalized = normalize_managed_path(workspace, raw_path)?;
        *raw_path = normalized.to_string_lossy().into_owned();
    }
    Ok(())
}

fn normalize_managed_path(workspace: &Path, raw_path: &str) -> Result<PathBuf, String> {
    if raw_path.is_empty() || raw_path.chars().any(|character| character == '\0') {
        return Err("browser file path is invalid".into());
    }
    let workspace = workspace
        .canonicalize()
        .map_err(|error| format!("cannot resolve browser workspace: {error}"))?;
    let requested = PathBuf::from(raw_path);
    let requested = if requested.is_absolute() {
        requested
    } else {
        workspace.join(requested)
    };
    let requested = lexically_normalize_absolute(&requested)?;
    let mut existing = requested.as_path();
    while !existing.exists() {
        existing = existing
            .parent()
            .ok_or_else(|| "browser file path has no existing parent".to_string())?;
    }
    let canonical_existing = existing
        .canonicalize()
        .map_err(|error| format!("cannot resolve browser file path: {error}"))?;
    if !canonical_existing.starts_with(&workspace) {
        return Err("browser file access is restricted to the current project workspace".into());
    }
    let suffix = requested
        .strip_prefix(existing)
        .map_err(|_| "browser file path is invalid".to_string())?;
    Ok(canonical_existing.join(suffix))
}

fn lexically_normalize_absolute(path: &Path) -> Result<PathBuf, String> {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            std::path::Component::RootDir => normalized.push(component.as_os_str()),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if !normalized.pop() {
                    return Err("browser file path escapes its filesystem root".into());
                }
            }
            std::path::Component::Normal(part) => normalized.push(part),
        }
    }
    normalized
        .is_absolute()
        .then_some(normalized)
        .ok_or_else(|| "browser file path must resolve to an absolute path".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concurrent_runs_for_one_agent_keep_independent_tokens() {
        let grants = Arc::new(RwLock::new(HashMap::new()));
        let runtime = BrowserProxyRuntime {
            url: "http://127.0.0.1:12345/mcp".into(),
            grants: grants.clone(),
            healthy: Arc::new(AtomicBool::new(true)),
            shutdown: None,
            thread: None,
        };
        let root = std::env::temp_dir();
        let agent_id = Uuid::new_v4();
        for token in ["run-one", "run-two"] {
            runtime
                .register(
                    token,
                    BrowserProxyGrant::new(
                        agent_id,
                        "http://127.0.0.1:9222".into(),
                        "page".into(),
                        root.clone(),
                    ),
                )
                .expect("register browser grant");
        }
        assert_eq!(grants.read().expect("browser grants").len(), 2);
        runtime.revoke_run_token("run-one");
        let grants = grants.read().expect("browser grants");
        assert_eq!(grants.len(), 1);
        assert!(grants.contains_key("run-two"));
    }

    #[test]
    fn tool_schemas_hide_page_routing_and_page_switching() {
        let page_schema = Arc::new(JsonObject::from_iter([
            ("type".into(), Value::String("object".into())),
            (
                "properties".into(),
                serde_json::json!({"pageId": {"type": "number"}, "url": {"type": "string"}}),
            ),
            ("required".into(), serde_json::json!(["pageId", "url"])),
        ]));
        let tools = vec![
            Tool::new("navigate_page", "navigate", page_schema.clone()),
            Tool::new("select_page", "select", page_schema),
        ];
        let sanitized = sanitize_tools(&tools);
        assert_eq!(sanitized.len(), 1);
        let schema = &sanitized[0].input_schema;
        assert!(schema["properties"].get("pageId").is_none());
        assert_eq!(schema["required"], serde_json::json!(["url"]));
    }

    #[test]
    fn result_page_lists_are_replaced_with_only_the_agent_page() {
        let page = ResolvedPage {
            id: 7,
            url: "https://agent.example/".into(),
            title: "Agent".into(),
        };
        let text = "Done\n## Pages\n7: agent\n8: other\n## Console\nclean";
        let sanitized = replace_pages_section(text, &page);
        assert!(sanitized.contains("7: Agent (https://agent.example/) [selected]"));
        assert!(!sanitized.contains("8: other"));
        assert!(sanitized.contains("## Console"));
    }

    #[test]
    fn text_page_lists_are_parsed_from_the_real_chrome_mcp_shape() {
        let result = CallToolResult::success(vec![ContentBlock::text(
            "## Pages\n1: about:blank#relay-agent-a [selected]\n2: New tab (chrome://new-tab-page/)",
        )]);
        let pages = text_pages(&result);
        assert_eq!(pages.len(), 2);
        assert_eq!(pages[0].id, 1);
        assert_eq!(pages[0].url, "about:blank#relay-agent-a");
        assert_eq!(pages[1].title, "New tab");
        assert_eq!(pages[1].url, "chrome://new-tab-page/");
    }

    #[test]
    fn browser_paths_cannot_escape_the_workspace() {
        let root = std::env::temp_dir().join(format!("relay-browser-path-{}", Uuid::new_v4()));
        std::fs::create_dir_all(root.join("artifacts")).expect("workspace");
        let allowed = normalize_managed_path(&root, "artifacts/screenshot.png")
            .expect("workspace path should be accepted");
        assert!(allowed.starts_with(root.canonicalize().expect("canonical workspace")));
        assert!(normalize_managed_path(&root, "../secret.txt").is_err());
        assert!(normalize_managed_path(&root, "missing/../../secret.txt").is_err());
        assert!(normalize_managed_path(&root, "artifacts/../inside.txt").is_ok());
        std::fs::remove_dir_all(root).expect("cleanup");
    }
}
