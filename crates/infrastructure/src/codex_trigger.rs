use std::{
    collections::{BTreeMap, HashMap},
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
    time::Duration,
};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader},
    process::Command,
    time::{sleep, timeout},
};
use uuid::Uuid;

use ai_chat_domain::company::{
    AGENT_CODEX_APPROVAL_TOOL_COMMAND, AGENT_CODEX_APPROVAL_TOOL_FILE_CHANGE,
    AGENT_CODEX_APPROVAL_TOOL_PERMISSIONS,
};
use ai_chat_shared::{AppError, AppResult};

use crate::codex_control::{
    managed_profile_id, CodexControlStore, CodexDefaultConfigSummary, CodexMcpServerInput,
    CodexMcpServerView, CODEX_MCP_TRANSPORT_HTTP, CODEX_MCP_TRANSPORT_STDIO,
};

const DEFAULT_RUN_TOKEN_ENV: &str = "RELAY_AGENT_RUN_TOKEN";
const MAX_STDERR_BYTES: usize = 32 * 1024;
const MAX_JSONL_LINE_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CodexModelReasoningEffort {
    pub effort: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CodexModelInfo {
    pub id: String,
    pub display_name: String,
    pub default_reasoning_effort: Option<String>,
    pub reasoning_efforts: Vec<CodexModelReasoningEffort>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexModelCatalogSnapshot {
    pub codex_profile: String,
    pub bundled: bool,
    pub models: Vec<CodexModelInfo>,
    pub source: String,
    pub discovered_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CodexModelCatalogFile {
    pub catalogs: Vec<CodexModelCatalogSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexPluginCatalogDiscovery {
    pub installed: Value,
    pub available: Value,
    pub marketplaces: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexDefaultAuthProbe {
    pub status: String,
    pub method: Option<String>,
    pub config: CodexDefaultConfigSummary,
}

#[derive(Debug, Clone)]
pub struct CodexTriggerRunner {
    executable: PathBuf,
    executable_source: String,
    prefix_args: Vec<String>,
    mcp_url: String,
    mcp_server_name: String,
    run_token_env_name: String,
    inherited_environment: HashMap<String, String>,
    shell_excluded_environment_names: Vec<String>,
    managed_profile_homes_root: PathBuf,
    managed_cli_home: PathBuf,
}

#[derive(Clone)]
pub struct CodexRunRequest {
    pub cwd: PathBuf,
    pub codex_profile: String,
    pub model: Option<String>,
    pub reasoning_effort: Option<String>,
    pub reasoning_summary: Option<String>,
    pub verbosity: Option<String>,
    pub personality: Option<String>,
    pub service_tier: Option<String>,
    pub sandbox_mode: String,
    pub approval_policy: String,
    pub network_access: bool,
    pub web_search: String,
    pub feature_multi_agent: bool,
    pub feature_remote_plugin: bool,
    pub feature_hooks: bool,
    pub feature_goals: bool,
    pub feature_shell_tool: bool,
    pub max_run_seconds: u64,
    pub prompt: String,
    pub existing_thread_id: Option<String>,
    pub run_token: String,
    pub environment: HashMap<String, String>,
    pub approval_handler: Option<Arc<dyn CodexApprovalHandler>>,
    pub progress_handler: Option<Arc<dyn CodexProgressHandler>>,
    pub cancellation_handler: Option<Arc<dyn CodexCancellationHandler>>,
}

#[derive(Debug, Clone)]
pub struct CodexProgressEvent {
    pub phase: String,
    pub summary: String,
    pub thread_id: Option<String>,
}

pub trait CodexProgressHandler: Send + Sync {
    fn report(&self, event: CodexProgressEvent);
}

pub trait CodexCancellationHandler: Send + Sync {
    fn should_cancel(&self) -> bool;
}

async fn wait_for_cancellation(handler: Option<&Arc<dyn CodexCancellationHandler>>) {
    let Some(handler) = handler else {
        std::future::pending::<()>().await;
        return;
    };
    loop {
        sleep(Duration::from_millis(500)).await;
        if handler.should_cancel() {
            return;
        }
    }
}

#[derive(Debug, Clone)]
pub struct CodexApprovalRequest {
    pub tool_name: String,
    pub risk_level: String,
    pub reason: String,
    pub arguments: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodexApprovalDecision {
    Accept,
    Decline,
}

#[async_trait]
pub trait CodexApprovalHandler: Send + Sync {
    async fn request_approval(
        &self,
        request: CodexApprovalRequest,
    ) -> AppResult<CodexApprovalDecision>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodexRunStatus {
    Succeeded,
    Failed,
    TimedOut,
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct CodexRunResult {
    pub status: CodexRunStatus,
    pub thread_id: Option<String>,
    pub exit_code: Option<i32>,
    pub final_message: Option<String>,
    pub error_message: Option<String>,
    pub resumed_existing_session: bool,
    pub replaced_unresumable_session: bool,
}

#[derive(Debug)]
struct ProcessOutcome {
    status: CodexRunStatus,
    thread_id: Option<String>,
    exit_code: Option<i32>,
    final_message: Option<String>,
    error_message: Option<String>,
    turn_started: bool,
}

#[derive(Debug, Default)]
struct JsonlEvents {
    thread_id: Option<String>,
    turn_started: bool,
    turn_completed: bool,
    turn_failed: bool,
    final_message: Option<String>,
    error_message: Option<String>,
}

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

    pub async fn discover_plugins(&self) -> AppResult<CodexPluginCatalogDiscovery> {
        let plugins = self
            .run_plugin_json(&["plugin", "list", "--available", "--json"], 30)
            .await?;
        let marketplaces = self
            .run_plugin_json(&["plugin", "marketplace", "list", "--json"], 30)
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
        operation: &str,
        plugin_id: Option<&str>,
    ) -> AppResult<Value> {
        match operation {
            "refresh" => Ok(json!({ "refreshed": true })),
            "install" | "remove" => {
                let plugin_id = plugin_id.ok_or_else(|| {
                    AppError::Validation("plugin_id is required for this operation".into())
                })?;
                validate_plugin_id(plugin_id)?;
                self.run_plugin_json(&["plugin", operation, plugin_id, "--json"], 120)
                    .await
            }
            _ => Err(AppError::Validation(
                "unsupported Codex plugin operation".into(),
            )),
        }
    }

    async fn run_plugin_json(&self, arguments: &[&str], timeout_seconds: u64) -> AppResult<Value> {
        let mut command = Command::new(&self.executable);
        command
            .args(&self.prefix_args)
            .args(arguments)
            .env_clear()
            .envs(&self.inherited_environment)
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

    pub async fn run(&self, request: CodexRunRequest) -> AppResult<CodexRunResult> {
        validate_request(&request)?;
        if let Some(thread_id) = request.existing_thread_id.as_deref() {
            let resumed = self.run_once(&request, Some(thread_id)).await?;
            if should_replace_session(&resumed) {
                let created = self.run_once(&request, None).await?;
                return Ok(to_public_result(created, false, true));
            }
            return Ok(to_public_result(resumed, true, false));
        }
        let created = self.run_once(&request, None).await?;
        Ok(to_public_result(created, false, false))
    }

    async fn run_once(
        &self,
        request: &CodexRunRequest,
        resume_thread_id: Option<&str>,
    ) -> AppResult<ProcessOutcome> {
        if request.approval_policy == "on-request" {
            self.run_app_server_once(request, resume_thread_id).await
        } else {
            self.run_exec_once(request, resume_thread_id).await
        }
    }

    async fn run_exec_once(
        &self,
        request: &CodexRunRequest,
        resume_thread_id: Option<&str>,
    ) -> AppResult<ProcessOutcome> {
        let sandbox_mode = codex_sandbox_mode(&request.sandbox_mode)?;
        let mut command = Command::new(&self.executable);
        command
            .args(&self.prefix_args)
            .arg("--ask-for-approval")
            .arg("never");
        command
            .arg("exec")
            .arg("--skip-git-repo-check")
            .arg("--json");
        self.apply_profile_arguments(&mut command, &request.codex_profile)?;
        if let Some(model) = request.model.as_deref() {
            command.arg("--model").arg(model);
        }
        apply_managed_cli_settings(&mut command, request);
        command
            .arg("--sandbox")
            .arg(sandbox_mode)
            .arg("--config")
            .arg(format!(
                "mcp_servers.{}.url={}",
                self.mcp_server_name,
                toml_string(&self.mcp_url)
            ))
            .arg("--config")
            .arg(format!(
                "mcp_servers.{}.required=true",
                self.mcp_server_name
            ))
            .arg("--config")
            .arg(format!(
                "mcp_servers.{}.env_http_headers={{ \"x-agent-run-token\" = {} }}",
                self.mcp_server_name,
                toml_string(&self.run_token_env_name)
            ))
            .arg("--config")
            .arg(format!(
                "shell_environment_policy.exclude={}",
                toml_string_array(&self.shell_excluded_environment_names)
            ))
            .arg("--config")
            .arg("shell_environment_policy.ignore_default_excludes=true");
        if let Some(thread_id) = resume_thread_id {
            command.arg("resume").arg(thread_id);
        }
        command
            .arg(&request.prompt)
            .current_dir(&request.cwd)
            .env_clear()
            .envs(&self.inherited_environment)
            .env(&self.run_token_env_name, &request.run_token)
            .env("GIT_TERMINAL_PROMPT", "0")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        self.apply_profile_environment(&mut command, &request.codex_profile)?;
        for (key, value) in &request.environment {
            command.env(key, value);
        }
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.as_std_mut().process_group(0);
        }

        let mut child = command.spawn().map_err(|error| {
            AppError::Validation(format!(
                "failed to start Codex executable {}: {}",
                self.executable.display(),
                sanitize_error(&error.to_string())
            ))
        })?;
        let process_id = child.id();
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AppError::Validation("Codex stdout pipe was not available".into()))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| AppError::Validation("Codex stderr pipe was not available".into()))?;
        report_progress(
            request.progress_handler.as_ref(),
            "starting",
            "正在启动本地 Codex",
            resume_thread_id,
        );
        let stdout_task = tokio::spawn(read_jsonl_events(stdout, request.progress_handler.clone()));
        let stderr_task = tokio::spawn(read_limited_text(stderr, MAX_STDERR_BYTES));

        report_progress(
            request.progress_handler.as_ref(),
            "starting",
            "正在连接 Codex app-server",
            resume_thread_id,
        );

        let mut timed_out = false;
        let mut cancelled = false;
        let wait_result = tokio::select! {
            result = timeout(Duration::from_secs(request.max_run_seconds), child.wait()) => Some(result),
            _ = wait_for_cancellation(request.cancellation_handler.as_ref()) => {
                cancelled = true;
                None
            }
        };
        let exit_status = match wait_result {
            Some(Ok(result)) => result.map_err(process_error)?,
            Some(Err(_)) => {
                timed_out = true;
                terminate_process_tree(process_id);
                match timeout(Duration::from_secs(5), child.wait()).await {
                    Ok(result) => result.map_err(process_error)?,
                    Err(_) => {
                        kill_process_tree(process_id);
                        child.wait().await.map_err(process_error)?
                    }
                }
            }
            None => {
                terminate_process_tree(process_id);
                match timeout(Duration::from_secs(5), child.wait()).await {
                    Ok(result) => result.map_err(process_error)?,
                    Err(_) => {
                        kill_process_tree(process_id);
                        child.wait().await.map_err(process_error)?
                    }
                }
            }
        };
        let events = stdout_task.await.map_err(join_error)??;
        let stderr = stderr_task.await.map_err(join_error)??;
        let mut error_message = events.error_message.clone();
        if error_message.is_none() && !exit_status.success() && !stderr.trim().is_empty() {
            error_message = Some(truncate(&sanitize_error(&stderr), 2_000));
        }
        if cancelled {
            error_message = Some("Codex run cancelled because the project was paused".into());
        } else if timed_out {
            error_message = Some(format!(
                "Codex run exceeded {} seconds",
                request.max_run_seconds
            ));
        }
        let mut status = if cancelled {
            CodexRunStatus::Cancelled
        } else if timed_out {
            CodexRunStatus::TimedOut
        } else if exit_status.success() && events.turn_completed && !events.turn_failed {
            CodexRunStatus::Succeeded
        } else {
            CodexRunStatus::Failed
        };
        let thread_id = events
            .thread_id
            .or_else(|| resume_thread_id.map(str::to_string));
        if status == CodexRunStatus::Succeeded && thread_id.is_none() {
            status = CodexRunStatus::Failed;
            error_message = Some("Codex completed without emitting a thread.started event".into());
        }
        if status == CodexRunStatus::Succeeded {
            // Codex may emit transient transport errors while its own reconnect loop is
            // recovering. A completed turn is authoritative, so those intermediate
            // warnings must not be persisted as the final run error.
            error_message = None;
        }
        if status == CodexRunStatus::Failed && error_message.is_none() {
            error_message = Some("Codex run failed before completing the turn".into());
        }
        Ok(ProcessOutcome {
            status,
            thread_id,
            exit_code: exit_status.code(),
            final_message: events.final_message,
            error_message,
            turn_started: events.turn_started,
        })
    }

    async fn run_app_server_once(
        &self,
        request: &CodexRunRequest,
        resume_thread_id: Option<&str>,
    ) -> AppResult<ProcessOutcome> {
        let sandbox_mode = codex_sandbox_mode(&request.sandbox_mode)?;
        let approval_handler = request.approval_handler.as_ref().ok_or_else(|| {
            AppError::Validation(
                "Codex on-request approval policy requires an approval handler".into(),
            )
        })?;
        let mut command = Command::new(&self.executable);
        command.args(&self.prefix_args);
        self.apply_profile_arguments(&mut command, &request.codex_profile)?;
        apply_managed_cli_settings(&mut command, request);
        command
            .arg("--config")
            .arg(format!(
                "mcp_servers.{}.url={}",
                self.mcp_server_name,
                toml_string(&self.mcp_url)
            ))
            .arg("--config")
            .arg(format!(
                "mcp_servers.{}.required=true",
                self.mcp_server_name
            ))
            .arg("--config")
            .arg(format!(
                "mcp_servers.{}.env_http_headers={{ \"x-agent-run-token\" = {} }}",
                self.mcp_server_name,
                toml_string(&self.run_token_env_name)
            ))
            .arg("--config")
            .arg(format!(
                "shell_environment_policy.exclude={}",
                toml_string_array(&self.shell_excluded_environment_names)
            ))
            .arg("--config")
            .arg("shell_environment_policy.ignore_default_excludes=true");
        command
            .arg("app-server")
            .arg("--stdio")
            .current_dir(&request.cwd)
            .env_clear()
            .envs(&self.inherited_environment)
            .env(&self.run_token_env_name, &request.run_token)
            .env("GIT_TERMINAL_PROMPT", "0")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        self.apply_profile_environment(&mut command, &request.codex_profile)?;
        for (key, value) in &request.environment {
            command.env(key, value);
        }
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.as_std_mut().process_group(0);
        }

        let mut child = command.spawn().map_err(|error| {
            AppError::Validation(format!(
                "failed to start Codex app-server {}: {}",
                self.executable.display(),
                sanitize_error(&error.to_string())
            ))
        })?;
        let process_id = child.id();
        let mut stdin = child.stdin.take().ok_or_else(|| {
            AppError::Validation("Codex app-server stdin pipe was not available".into())
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            AppError::Validation("Codex app-server stdout pipe was not available".into())
        })?;
        let stderr = child.stderr.take().ok_or_else(|| {
            AppError::Validation("Codex app-server stderr pipe was not available".into())
        })?;
        let stderr_task = tokio::spawn(read_limited_text(stderr, MAX_STDERR_BYTES));

        let drive_result = tokio::select! {
            result = timeout(
                Duration::from_secs(request.max_run_seconds),
                drive_app_server(
                    &mut stdin,
                    stdout,
                    request,
                    resume_thread_id,
                    sandbox_mode,
                    approval_handler.as_ref(),
                ),
            ) => Some(result),
            _ = wait_for_cancellation(request.cancellation_handler.as_ref()) => None,
        };
        let mut outcome = match drive_result {
            Some(Ok(Ok(outcome))) => outcome,
            Some(Ok(Err(error))) => ProcessOutcome {
                status: CodexRunStatus::Failed,
                thread_id: resume_thread_id.map(str::to_string),
                exit_code: Some(1),
                final_message: None,
                error_message: Some(truncate(&sanitize_error(&error.to_string()), 2_000)),
                turn_started: false,
            },
            Some(Err(_)) => ProcessOutcome {
                status: CodexRunStatus::TimedOut,
                thread_id: resume_thread_id.map(str::to_string),
                exit_code: None,
                final_message: None,
                error_message: Some(format!(
                    "Codex run exceeded {} seconds while waiting for completion or approval",
                    request.max_run_seconds
                )),
                turn_started: true,
            },
            None => ProcessOutcome {
                status: CodexRunStatus::Cancelled,
                thread_id: resume_thread_id.map(str::to_string),
                exit_code: None,
                final_message: None,
                error_message: Some("Codex run cancelled because the project was paused".into()),
                turn_started: true,
            },
        };

        drop(stdin);
        let exit_status = match timeout(Duration::from_secs(3), child.wait()).await {
            Ok(result) => Some(result.map_err(process_error)?),
            Err(_) => {
                terminate_process_tree(process_id);
                match timeout(Duration::from_secs(3), child.wait()).await {
                    Ok(result) => Some(result.map_err(process_error)?),
                    Err(_) => {
                        kill_process_tree(process_id);
                        Some(child.wait().await.map_err(process_error)?)
                    }
                }
            }
        };
        let stderr = stderr_task.await.map_err(join_error)??;
        if outcome.status == CodexRunStatus::Failed
            && outcome.error_message.is_none()
            && !stderr.trim().is_empty()
        {
            outcome.error_message = Some(truncate(&sanitize_error(&stderr), 2_000));
        }
        if outcome.exit_code.is_none() {
            outcome.exit_code = exit_status.and_then(|status| status.code());
        }
        Ok(outcome)
    }

    fn apply_profile_arguments(&self, command: &mut Command, codex_profile: &str) -> AppResult<()> {
        validate_config_key(codex_profile, "Codex profile")?;
        if codex_profile.starts_with("relay_") {
            managed_profile_id(codex_profile).ok_or_else(|| {
                AppError::Validation("managed Codex profile selector is invalid".into())
            })?;
        } else if codex_profile != "default" {
            command.arg("--profile").arg(codex_profile);
        }
        Ok(())
    }

    fn apply_profile_environment(
        &self,
        command: &mut Command,
        codex_profile: &str,
    ) -> AppResult<()> {
        if codex_profile.starts_with("relay_") {
            let profile_id = managed_profile_id(codex_profile).ok_or_else(|| {
                AppError::Validation("managed Codex profile selector is invalid".into())
            })?;
            command.env(
                "CODEX_HOME",
                self.managed_profile_homes_root.join(profile_id.to_string()),
            );
        }
        Ok(())
    }

    fn codex_home_for_selector(&self, target_selector: &str) -> AppResult<Option<PathBuf>> {
        if target_selector.starts_with("relay_") {
            let profile_id = managed_profile_id(target_selector).ok_or_else(|| {
                AppError::Validation("managed Codex profile selector is invalid".into())
            })?;
            return Ok(Some(
                self.managed_profile_homes_root.join(profile_id.to_string()),
            ));
        }
        if target_selector != "default" {
            return Err(AppError::Validation(
                "Codex MCP target environment is invalid".into(),
            ));
        }
        Ok(self
            .inherited_environment
            .get("CODEX_HOME")
            .filter(|value| !value.trim().is_empty())
            .map(PathBuf::from)
            .or_else(|| {
                self.inherited_environment
                    .get("HOME")
                    .filter(|value| !value.trim().is_empty())
                    .map(|home| PathBuf::from(home).join(".codex"))
            }))
    }

    fn configured_mcp_server_names(
        &self,
        target_selector: &str,
    ) -> AppResult<std::collections::HashSet<String>> {
        let Some(codex_home) = self.codex_home_for_selector(target_selector)? else {
            return Ok(std::collections::HashSet::new());
        };
        let config_path = codex_home.join("config.toml");
        let Ok(metadata) = std::fs::metadata(&config_path) else {
            return Ok(std::collections::HashSet::new());
        };
        if metadata.len() > 2 * 1024 * 1024 {
            return Ok(std::collections::HashSet::new());
        }
        let Ok(content) = std::fs::read_to_string(config_path) else {
            return Ok(std::collections::HashSet::new());
        };
        Ok(content
            .lines()
            .map(str::trim)
            .filter(|line| line.starts_with('[') && line.ends_with(']'))
            .filter_map(|line| first_section_name(&line[1..line.len() - 1], "mcp_servers."))
            .collect())
    }
}

fn safe_mcp_server_view(
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

fn safe_mcp_url(value: &str) -> Option<String> {
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

fn classify_default_auth_probe(success: bool, stdout: &str, stderr: &str) -> CodexDefaultAuthProbe {
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

fn extract_masked_credential_hint(status_output: &str) -> Option<String> {
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

fn populate_default_config_summary(content: &str, summary: &mut CodexDefaultConfigSummary) {
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

async fn write_managed_openai_base_url(codex_home: &Path, base_url: Option<&str>) -> AppResult<()> {
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

async fn activate_managed_codex_config(temporary_path: &Path, config_path: &Path) -> AppResult<()> {
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

fn validate_managed_openai_base_url(value: Option<&str>) -> AppResult<Option<String>> {
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

fn render_openai_base_url_config(content: &str, base_url: Option<&str>) -> String {
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

fn first_section_name(section: &str, prefix: &str) -> Option<String> {
    let remainder = section.strip_prefix(prefix)?;
    let first = remainder.split('.').next()?.trim().trim_matches('"');
    (!first.is_empty() && first.len() <= 128 && !first.chars().any(char::is_control))
        .then(|| first.to_string())
}

fn parse_safe_toml_scalar(raw: &str) -> Option<String> {
    let value = raw.trim().trim_matches('"').trim_matches('\'').trim();
    (!value.is_empty() && value.len() <= 256 && !value.chars().any(char::is_control))
        .then(|| value.to_string())
}

pub fn collect_codex_models(value: &Value, models: &mut BTreeMap<String, CodexModelInfo>) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect_codex_models(item, models);
            }
        }
        Value::Object(object) => {
            let is_selectable = object
                .get("visibility")
                .and_then(Value::as_str)
                .map(|visibility| visibility.eq_ignore_ascii_case("list"))
                .unwrap_or(true);
            let id = object
                .get("slug")
                .or_else(|| object.get("model"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|id| !id.is_empty() && id.len() <= 128);
            if let Some(id) = id.filter(|_| is_selectable) {
                let display_name = object
                    .get("display_name")
                    .or_else(|| object.get("name"))
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|name| !name.is_empty())
                    .unwrap_or(id);
                let default_reasoning_effort = object
                    .get("default_reasoning_level")
                    .or_else(|| object.get("defaultReasoningEffort"))
                    .and_then(Value::as_str)
                    .filter(|effort| is_codex_reasoning_effort(effort))
                    .map(str::to_string);
                let mut reasoning_efforts = Vec::new();
                if let Some(values) = object
                    .get("supported_reasoning_levels")
                    .or_else(|| object.get("supportedReasoningEfforts"))
                    .and_then(Value::as_array)
                {
                    for reasoning in values {
                        let Some(effort) = reasoning
                            .get("effort")
                            .or_else(|| reasoning.get("reasoningEffort"))
                            .and_then(Value::as_str)
                            .filter(|effort| is_codex_reasoning_effort(effort))
                        else {
                            continue;
                        };
                        if reasoning_efforts
                            .iter()
                            .any(|item: &CodexModelReasoningEffort| item.effort == effort)
                        {
                            continue;
                        }
                        let description = reasoning
                            .get("description")
                            .and_then(Value::as_str)
                            .map(str::trim)
                            .filter(|description| !description.is_empty())
                            .unwrap_or(effort);
                        reasoning_efforts.push(CodexModelReasoningEffort {
                            effort: effort.to_string(),
                            description: description.to_string(),
                        });
                    }
                }
                models.insert(
                    id.to_string(),
                    CodexModelInfo {
                        id: id.to_string(),
                        display_name: display_name.to_string(),
                        default_reasoning_effort,
                        reasoning_efforts,
                    },
                );
            }
            for nested in object.values() {
                if nested.is_array() || nested.is_object() {
                    collect_codex_models(nested, models);
                }
            }
        }
        Value::String(id) if !id.trim().is_empty() && id.len() <= 128 => {
            if (id.contains("gpt-") || id.contains("codex"))
                && id.chars().all(|character| {
                    character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
                })
            {
                models.insert(
                    id.clone(),
                    CodexModelInfo {
                        id: id.clone(),
                        display_name: id.clone(),
                        default_reasoning_effort: None,
                        reasoning_efforts: Vec::new(),
                    },
                );
            }
        }
        _ => {}
    }
}

fn is_codex_reasoning_effort(value: &str) -> bool {
    matches!(
        value,
        "none" | "minimal" | "low" | "medium" | "high" | "xhigh" | "max" | "ultra"
    )
}

async fn drive_app_server<W, R>(
    writer: &mut W,
    reader: R,
    request: &CodexRunRequest,
    resume_thread_id: Option<&str>,
    sandbox_mode: &str,
    approval_handler: &dyn CodexApprovalHandler,
) -> AppResult<ProcessOutcome>
where
    W: AsyncWrite + Unpin,
    R: AsyncRead + Unpin,
{
    let mut lines = BufReader::new(reader).lines();
    send_json_rpc(
        writer,
        &json!({
            "method": "initialize",
            "id": 0,
            "params": {
                "clientInfo": {
                    "name": "relay_agent_trigger",
                    "title": "Relay Agent Trigger",
                    "version": env!("CARGO_PKG_VERSION")
                },
                "capabilities": { "experimentalApi": true }
            }
        }),
    )
    .await?;
    let initialize = wait_for_rpc_response(&mut lines, 0).await?;
    if let Some(error) = rpc_error_message(&initialize) {
        return Err(AppError::Validation(format!(
            "Codex app-server initialization failed: {error}"
        )));
    }
    send_json_rpc(writer, &json!({ "method": "initialized", "params": {} })).await?;

    let mut thread_params = json!({
        "cwd": request.cwd.to_string_lossy(),
        "sandbox": sandbox_mode,
        "approvalPolicy": request.approval_policy,
        "approvalsReviewer": "user"
    });
    if let Some(model) = request.model.as_deref() {
        thread_params["model"] = Value::String(model.to_string());
    }
    let thread_method = if let Some(thread_id) = resume_thread_id {
        thread_params["threadId"] = Value::String(thread_id.to_string());
        "thread/resume"
    } else {
        "thread/start"
    };
    send_json_rpc(
        writer,
        &json!({ "method": thread_method, "id": 1, "params": thread_params }),
    )
    .await?;
    let thread_response = wait_for_rpc_response(&mut lines, 1).await?;
    if let Some(error) = rpc_error_message(&thread_response) {
        return Ok(ProcessOutcome {
            status: CodexRunStatus::Failed,
            thread_id: resume_thread_id.map(str::to_string),
            exit_code: Some(1),
            final_message: None,
            error_message: Some(truncate(&sanitize_error(&error), 2_000)),
            turn_started: false,
        });
    }
    let thread_id = thread_response
        .pointer("/result/thread/id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| resume_thread_id.map(str::to_string))
        .ok_or_else(|| AppError::Validation("Codex app-server returned no thread ID".into()))?;
    report_progress(
        request.progress_handler.as_ref(),
        "session",
        "Codex 固定会话已连接",
        Some(&thread_id),
    );

    let mut turn_params = json!({
        "threadId": thread_id,
        "input": [{ "type": "text", "text": request.prompt }],
        "cwd": request.cwd.to_string_lossy(),
        "approvalPolicy": request.approval_policy,
        "approvalsReviewer": "user"
    });
    if let Some(model) = request.model.as_deref() {
        turn_params["model"] = Value::String(model.to_string());
    }
    if let Some(reasoning_effort) = request.reasoning_effort.as_deref() {
        turn_params["effort"] = Value::String(reasoning_effort.to_string());
    }
    send_json_rpc(
        writer,
        &json!({ "method": "turn/start", "id": 2, "params": turn_params }),
    )
    .await?;

    let mut turn_started = false;
    let mut final_message = None;
    let mut error_message = None;
    while let Some(value) = next_json_rpc(&mut lines).await? {
        if json_rpc_id_matches(&value, 2) {
            if let Some(error) = rpc_error_message(&value) {
                return Ok(ProcessOutcome {
                    status: CodexRunStatus::Failed,
                    thread_id: Some(thread_id.clone()),
                    exit_code: Some(1),
                    final_message,
                    error_message: Some(truncate(&sanitize_error(&error), 2_000)),
                    turn_started,
                });
            }
            turn_started = true;
            continue;
        }
        if value.get("id").is_some() && value.get("method").is_some() {
            handle_app_server_request(writer, &value, approval_handler).await?;
            continue;
        }
        match value.get("method").and_then(Value::as_str) {
            Some("turn/started") => {
                turn_started = true;
                report_progress(
                    request.progress_handler.as_ref(),
                    "thinking",
                    "Codex 已开始分析待办",
                    Some(&thread_id),
                );
            }
            Some("item/started") => {
                if let Some(item) = value.pointer("/params/item") {
                    if let Some((phase, summary)) = summarize_codex_item(item, false) {
                        report_progress(
                            request.progress_handler.as_ref(),
                            &phase,
                            &summary,
                            Some(&thread_id),
                        );
                    }
                }
            }
            Some("item/completed") => {
                if let Some(item) = value.pointer("/params/item") {
                    if let Some(message) = extract_agent_message(item) {
                        final_message = Some(message);
                    }
                    if let Some((phase, summary)) = summarize_codex_item(item, true) {
                        report_progress(
                            request.progress_handler.as_ref(),
                            &phase,
                            &summary,
                            Some(&thread_id),
                        );
                    }
                }
            }
            Some("turn/completed") => {
                let turn = value.pointer("/params/turn");
                if final_message.is_none() {
                    final_message = turn.and_then(extract_final_turn_message);
                }
                let status = turn
                    .and_then(|turn| turn.get("status"))
                    .and_then(Value::as_str)
                    .unwrap_or("failed");
                let turn_error = turn
                    .and_then(|turn| turn.pointer("/error/message"))
                    .and_then(Value::as_str)
                    .map(str::to_string);
                let succeeded = status == "completed";
                return Ok(ProcessOutcome {
                    status: if succeeded {
                        CodexRunStatus::Succeeded
                    } else {
                        CodexRunStatus::Failed
                    },
                    thread_id: Some(thread_id),
                    exit_code: Some(if succeeded { 0 } else { 1 }),
                    final_message,
                    error_message: if succeeded {
                        None
                    } else {
                        turn_error
                            .or(error_message)
                            .or_else(|| Some(format!("Codex turn completed with status {status}")))
                    },
                    turn_started: true,
                });
            }
            Some("error") => {
                error_message = value
                    .pointer("/params/message")
                    .or_else(|| value.pointer("/params/error/message"))
                    .and_then(Value::as_str)
                    .map(|message| truncate(&sanitize_error(message), 2_000));
            }
            _ => {}
        }
    }
    if let Some(reasoning_summary) = request.reasoning_summary.as_deref() {
        if !matches!(reasoning_summary, "auto" | "concise" | "detailed" | "none") {
            return Err(AppError::Validation(
                "Codex reasoning summary must be auto, concise, detailed, or none".into(),
            ));
        }
    }
    if let Some(verbosity) = request.verbosity.as_deref() {
        if !matches!(verbosity, "low" | "medium" | "high") {
            return Err(AppError::Validation(
                "Codex verbosity must be low, medium, or high".into(),
            ));
        }
    }
    if let Some(personality) = request.personality.as_deref() {
        if !matches!(personality, "none" | "friendly" | "pragmatic") {
            return Err(AppError::Validation(
                "Codex personality must be none, friendly, or pragmatic".into(),
            ));
        }
    }
    if request
        .service_tier
        .as_deref()
        .is_some_and(|value| value != "fast")
    {
        return Err(AppError::Validation(
            "Codex service tier must be fast when configured".into(),
        ));
    }
    if !matches!(
        request.web_search.as_str(),
        "disabled" | "cached" | "indexed" | "live"
    ) {
        return Err(AppError::Validation(
            "Codex web search must be disabled, cached, indexed, or live".into(),
        ));
    }

    Ok(ProcessOutcome {
        status: CodexRunStatus::Failed,
        thread_id: Some(thread_id),
        exit_code: Some(1),
        final_message,
        error_message: error_message
            .or_else(|| Some("Codex app-server closed before turn/completed".into())),
        turn_started,
    })
}

async fn handle_app_server_request<W>(
    writer: &mut W,
    value: &Value,
    approval_handler: &dyn CodexApprovalHandler,
) -> AppResult<()>
where
    W: AsyncWrite + Unpin,
{
    let rpc_id = value
        .get("id")
        .cloned()
        .ok_or_else(|| AppError::Validation("Codex server request had no ID".into()))?;
    let method = value
        .get("method")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::Validation("Codex server request had no method".into()))?;
    let params = value.get("params").cloned().unwrap_or_else(|| json!({}));
    let (tool_name, default_reason) = match method {
        "item/commandExecution/requestApproval" | "execCommandApproval" => {
            (AGENT_CODEX_APPROVAL_TOOL_COMMAND, "Codex 请求执行受限命令")
        }
        "item/fileChange/requestApproval" | "applyPatchApproval" => (
            AGENT_CODEX_APPROVAL_TOOL_FILE_CHANGE,
            "Codex 请求修改受保护文件",
        ),
        "item/permissions/requestApproval" => (
            AGENT_CODEX_APPROVAL_TOOL_PERMISSIONS,
            "Codex 请求额外文件系统或网络权限",
        ),
        _ => {
            return send_json_rpc(
                writer,
                &json!({
                    "id": rpc_id,
                    "error": { "code": -32601, "message": format!("Unsupported server request: {method}") }
                }),
            )
            .await;
        }
    };
    let reason = params
        .get("reason")
        .and_then(Value::as_str)
        .filter(|reason| !reason.trim().is_empty())
        .unwrap_or(default_reason)
        .to_string();
    let approval = approval_handler
        .request_approval(CodexApprovalRequest {
            tool_name: tool_name.into(),
            risk_level: "high".into(),
            reason,
            arguments: json!({ "method": method, "params": params.clone() }),
        })
        .await;
    let decision = match approval {
        Ok(decision) => decision,
        Err(error) => {
            let _ = send_approval_response(
                writer,
                rpc_id.clone(),
                method,
                &params,
                CodexApprovalDecision::Decline,
            )
            .await;
            return Err(error);
        }
    };
    send_approval_response(writer, rpc_id, method, &params, decision).await
}

async fn send_approval_response<W>(
    writer: &mut W,
    rpc_id: Value,
    method: &str,
    params: &Value,
    decision: CodexApprovalDecision,
) -> AppResult<()>
where
    W: AsyncWrite + Unpin,
{
    let result = match method {
        "item/commandExecution/requestApproval" | "item/fileChange/requestApproval" => {
            json!({ "decision": if decision == CodexApprovalDecision::Accept { "accept" } else { "decline" } })
        }
        "item/permissions/requestApproval" => {
            let permissions = if decision == CodexApprovalDecision::Accept {
                params
                    .get("permissions")
                    .cloned()
                    .unwrap_or_else(|| json!({}))
            } else {
                json!({})
            };
            json!({ "permissions": permissions, "scope": "turn" })
        }
        "execCommandApproval" | "applyPatchApproval" => {
            if decision == CodexApprovalDecision::Accept {
                json!({ "decision": "approved" })
            } else {
                json!({ "decision": { "denied": { "rejection": "Rejected by Human" } } })
            }
        }
        _ => json!({}),
    };
    send_json_rpc(writer, &json!({ "id": rpc_id, "result": result })).await
}

async fn send_json_rpc<W>(writer: &mut W, value: &Value) -> AppResult<()>
where
    W: AsyncWrite + Unpin,
{
    let mut line = serde_json::to_vec(value).map_err(|error| {
        AppError::Validation(format!("failed to encode Codex JSON-RPC message: {error}"))
    })?;
    line.push(b'\n');
    writer.write_all(&line).await.map_err(process_error)?;
    writer.flush().await.map_err(process_error)
}

async fn wait_for_rpc_response<R>(
    lines: &mut tokio::io::Lines<BufReader<R>>,
    expected_id: i64,
) -> AppResult<Value>
where
    R: AsyncRead + Unpin,
{
    while let Some(value) = next_json_rpc(lines).await? {
        if json_rpc_id_matches(&value, expected_id) {
            return Ok(value);
        }
    }
    Err(AppError::Validation(format!(
        "Codex app-server closed before JSON-RPC response {expected_id}"
    )))
}

async fn next_json_rpc<R>(lines: &mut tokio::io::Lines<BufReader<R>>) -> AppResult<Option<Value>>
where
    R: AsyncRead + Unpin,
{
    while let Some(line) = lines.next_line().await.map_err(process_error)? {
        if line.len() > MAX_JSONL_LINE_BYTES {
            return Err(AppError::Validation(
                "Codex emitted an oversized JSON-RPC message".into(),
            ));
        }
        if let Ok(value) = serde_json::from_str::<Value>(&line) {
            return Ok(Some(value));
        }
    }
    Ok(None)
}

fn json_rpc_id_matches(value: &Value, expected_id: i64) -> bool {
    value
        .get("id")
        .and_then(Value::as_i64)
        .is_some_and(|id| id == expected_id)
}

fn rpc_error_message(value: &Value) -> Option<String> {
    value
        .pointer("/error/message")
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn extract_agent_message(item: &Value) -> Option<String> {
    (item.get("type").and_then(Value::as_str) == Some("agentMessage"))
        .then(|| item.get("text").and_then(Value::as_str))
        .flatten()
        .map(|message| truncate(message, 4_000))
}

fn extract_final_turn_message(turn: &Value) -> Option<String> {
    turn.get("items")
        .and_then(Value::as_array)
        .and_then(|items| items.iter().rev().find_map(extract_agent_message))
}

fn validate_request(request: &CodexRunRequest) -> AppResult<()> {
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

fn codex_sandbox_mode(value: &str) -> AppResult<&'static str> {
    match value {
        "read_only" => Ok("read-only"),
        "workspace_write" => Ok("workspace-write"),
        _ => Err(AppError::Validation(
            "Codex sandbox mode must be read_only or workspace_write".into(),
        )),
    }
}

async fn read_jsonl_events<R>(
    reader: R,
    progress_handler: Option<Arc<dyn CodexProgressHandler>>,
) -> AppResult<JsonlEvents>
where
    R: AsyncRead + Unpin,
{
    let mut lines = BufReader::new(reader).lines();
    let mut events = JsonlEvents::default();
    while let Some(line) = lines.next_line().await.map_err(process_error)? {
        if line.len() > MAX_JSONL_LINE_BYTES {
            return Err(AppError::Validation(
                "Codex emitted an oversized JSONL event".into(),
            ));
        }
        let Ok(value) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        match value.get("type").and_then(Value::as_str) {
            Some("thread.started") => {
                events.thread_id = value
                    .get("thread_id")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                report_progress(
                    progress_handler.as_ref(),
                    "session",
                    "Codex 固定会话已连接",
                    events.thread_id.as_deref(),
                );
            }
            Some("turn.started") => {
                events.turn_started = true;
                report_progress(
                    progress_handler.as_ref(),
                    "thinking",
                    "Codex 已开始分析待办",
                    events.thread_id.as_deref(),
                );
            }
            Some("turn.completed") => {
                events.turn_completed = true;
                report_progress(
                    progress_handler.as_ref(),
                    "finishing",
                    "Codex 正在整理本轮结果",
                    events.thread_id.as_deref(),
                );
            }
            Some("turn.failed") => {
                events.turn_failed = true;
                events.error_message = event_error_message(&value).or(events.error_message);
            }
            Some("error") => {
                events.error_message = event_error_message(&value).or(events.error_message);
            }
            Some("item.started") => {
                if let Some(item) = value.get("item") {
                    if let Some((phase, summary)) = summarize_codex_item(item, false) {
                        report_progress(
                            progress_handler.as_ref(),
                            &phase,
                            &summary,
                            events.thread_id.as_deref(),
                        );
                    }
                }
            }
            Some("item.completed") => {
                let item = value.get("item");
                if item
                    .and_then(|item| item.get("type"))
                    .and_then(Value::as_str)
                    == Some("agent_message")
                {
                    events.final_message = item
                        .and_then(|item| item.get("text"))
                        .and_then(Value::as_str)
                        .map(|text| truncate(text, 4_000));
                }
                if let Some(item) = item {
                    if let Some((phase, summary)) = summarize_codex_item(item, true) {
                        report_progress(
                            progress_handler.as_ref(),
                            &phase,
                            &summary,
                            events.thread_id.as_deref(),
                        );
                    }
                }
            }
            _ => {}
        }
    }
    Ok(events)
}

fn report_progress(
    handler: Option<&Arc<dyn CodexProgressHandler>>,
    phase: &str,
    summary: &str,
    thread_id: Option<&str>,
) {
    if let Some(handler) = handler {
        handler.report(CodexProgressEvent {
            phase: phase.to_string(),
            summary: truncate(&sanitize_error(summary), 500),
            thread_id: thread_id.map(str::to_string),
        });
    }
}

fn summarize_codex_item(item: &Value, completed: bool) -> Option<(String, String)> {
    let item_type = item.get("type").and_then(Value::as_str)?;
    let completion = if completed { "完成" } else { "正在" };
    let summary = match item_type {
        "command_execution" | "commandExecution" => {
            let command = item
                .get("command")
                .and_then(Value::as_str)
                .or_else(|| item.pointer("/command/text").and_then(Value::as_str))
                .unwrap_or("命令");
            format!("{completion}执行命令：{}", truncate(command, 240))
        }
        "mcp_tool_call" | "mcpToolCall" => {
            let server = item
                .get("server")
                .or_else(|| item.get("server_name"))
                .and_then(Value::as_str);
            let tool = item
                .get("tool")
                .or_else(|| item.get("tool_name"))
                .or_else(|| item.get("name"))
                .and_then(Value::as_str)
                .unwrap_or("MCP 工具");
            let label =
                server.map_or_else(|| tool.to_string(), |server| format!("{server}.{tool}"));
            let action = mcp_tool_action(item)
                .map(|action| format!("（action: {action}）"))
                .unwrap_or_default();
            let progress = if completed && codex_item_failed(item) {
                "工具调用失败："
            } else if completed {
                "完成调用工具："
            } else {
                "正在调用工具："
            };
            format!("{progress}{label}{action}")
        }
        "file_change" | "fileChange" => format!("{completion}修改项目文件"),
        "reasoning" => format!("{completion}分析问题和下一步"),
        "agent_message" | "agentMessage" => format!("{completion}整理回复和执行结果"),
        "web_search" | "webSearch" => format!("{completion}搜索资料"),
        "todo_list" | "todoList" => format!("{completion}更新执行计划"),
        _ => return None,
    };
    let phase = match item_type {
        "command_execution" | "commandExecution" => "command",
        "mcp_tool_call" | "mcpToolCall" => "tool",
        "file_change" | "fileChange" => "files",
        "reasoning" => "thinking",
        "agent_message" | "agentMessage" => "reporting",
        "web_search" | "webSearch" => "searching",
        "todo_list" | "todoList" => "planning",
        _ => "running",
    };
    Some((phase.into(), summary))
}

fn mcp_tool_action(item: &Value) -> Option<String> {
    ["arguments", "input", "params", "arguments_json"]
        .into_iter()
        .filter_map(|key| item.get(key))
        .find_map(|arguments| {
            arguments
                .get("action")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| {
                    arguments.as_str().and_then(|text| {
                        serde_json::from_str::<Value>(text).ok().and_then(|value| {
                            value
                                .get("action")
                                .and_then(Value::as_str)
                                .map(str::to_string)
                        })
                    })
                })
        })
}

fn codex_item_failed(item: &Value) -> bool {
    item.get("status")
        .and_then(Value::as_str)
        .is_some_and(|status| matches!(status, "failed" | "error"))
        || item
            .get("error")
            .is_some_and(|error| !error.is_null() && error.as_str() != Some(""))
        || item
            .pointer("/result/isError")
            .and_then(Value::as_bool)
            .unwrap_or(false)
}

async fn read_limited_text<R>(mut reader: R, max_bytes: usize) -> AppResult<String>
where
    R: AsyncRead + Unpin,
{
    use tokio::io::AsyncReadExt;

    let mut output = Vec::new();
    let mut chunk = [0_u8; 4_096];
    loop {
        let read = reader.read(&mut chunk).await.map_err(process_error)?;
        if read == 0 {
            break;
        }
        let remaining = max_bytes.saturating_sub(output.len());
        if remaining > 0 {
            output.extend_from_slice(&chunk[..read.min(remaining)]);
        }
    }
    Ok(String::from_utf8_lossy(&output).into_owned())
}

fn event_error_message(value: &Value) -> Option<String> {
    value
        .get("message")
        .and_then(Value::as_str)
        .or_else(|| {
            value
                .get("error")
                .and_then(|error| error.get("message"))
                .and_then(Value::as_str)
        })
        .map(|message| truncate(&sanitize_error(message), 2_000))
}

fn should_replace_session(outcome: &ProcessOutcome) -> bool {
    if outcome.status != CodexRunStatus::Failed || outcome.turn_started {
        return false;
    }
    let message = outcome
        .error_message
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    ["resume", "session", "thread", "rollout", "conversation"]
        .iter()
        .any(|marker| message.contains(marker))
        && [
            "not found",
            "missing",
            "invalid",
            "cannot",
            "could not",
            "failed",
        ]
        .iter()
        .any(|marker| message.contains(marker))
}

fn to_public_result(
    outcome: ProcessOutcome,
    resumed_existing_session: bool,
    replaced_unresumable_session: bool,
) -> CodexRunResult {
    CodexRunResult {
        status: outcome.status,
        thread_id: outcome.thread_id,
        exit_code: outcome.exit_code,
        final_message: outcome.final_message,
        error_message: outcome.error_message,
        resumed_existing_session,
        replaced_unresumable_session,
    }
}

fn validate_safe_value(value: &str, field: &str, max_characters: usize) -> AppResult<()> {
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

fn validate_prompt(value: &str) -> AppResult<()> {
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

fn validate_config_key(value: &str, field: &str) -> AppResult<()> {
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

fn validate_plugin_id(value: &str) -> AppResult<()> {
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

fn validate_environment_name(value: &str) -> AppResult<()> {
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

fn toml_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".into())
}

fn apply_managed_cli_settings(command: &mut Command, request: &CodexRunRequest) {
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

fn toml_string_array(values: &[String]) -> String {
    let values = values
        .iter()
        .map(|value| toml_string(value))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{values}]")
}

fn split_csv(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect()
}

fn resolve_codex_executable(
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

fn executable_in_path(name: &str) -> Option<PathBuf> {
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

fn default_environment_allowlist() -> Vec<String> {
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

fn collect_inherited_environment(allowlist: &[String]) -> HashMap<String, String> {
    allowlist
        .iter()
        .filter_map(|name| std::env::var(name).ok().map(|value| (name.clone(), value)))
        .collect()
}

fn process_error(error: std::io::Error) -> AppError {
    AppError::Validation(format!(
        "Codex process I/O error: {}",
        sanitize_error(&error.to_string())
    ))
}

fn join_error(error: tokio::task::JoinError) -> AppError {
    AppError::Validation(format!("Codex output reader failed: {error}"))
}

fn sanitize_error(value: &str) -> String {
    value.replace(['\r', '\n'], " ")
}

fn truncate(value: &str, max_characters: usize) -> String {
    value.chars().take(max_characters).collect()
}

#[cfg(unix)]
fn terminate_process_tree(process_id: Option<u32>) {
    if let Some(process_id) = process_id {
        unsafe {
            libc::kill(-(process_id as i32), libc::SIGTERM);
        }
    }
}

#[cfg(not(unix))]
fn terminate_process_tree(_process_id: Option<u32>) {}

#[cfg(unix)]
fn kill_process_tree(process_id: Option<u32>) {
    if let Some(process_id) = process_id {
        unsafe {
            libc::kill(-(process_id as i32), libc::SIGKILL);
        }
        std::thread::yield_now();
    }
}

#[cfg(not(unix))]
fn kill_process_tree(_process_id: Option<u32>) {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn model_catalog_excludes_hidden_models_and_nested_ids() {
        let catalog = json!({
            "models": [
                {
                    "slug": "gpt-5.6-sol",
                    "display_name": "GPT-5.6-Sol",
                    "visibility": "list",
                    "default_reasoning_level": "low",
                    "supported_reasoning_levels": [
                        { "effort": "low", "description": "Fast" },
                        { "effort": "high", "description": "Deep" }
                    ],
                    "service_tiers": [{ "id": "priority", "name": "Fast" }]
                },
                {
                    "slug": "codex-auto-review",
                    "display_name": "Codex Auto Review",
                    "visibility": "hide"
                }
            ]
        });
        let mut models = BTreeMap::new();

        collect_codex_models(&catalog, &mut models);

        assert_eq!(models.len(), 1);
        let model = models.get("gpt-5.6-sol").expect("selectable model");
        assert_eq!(model.display_name, "GPT-5.6-Sol");
        assert_eq!(model.default_reasoning_effort.as_deref(), Some("low"));
        assert_eq!(
            model
                .reasoning_efforts
                .iter()
                .map(|effort| effort.effort.as_str())
                .collect::<Vec<_>>(),
            vec!["low", "high"]
        );
    }

    #[test]
    fn managed_cli_settings_are_injected_as_cli_overrides() {
        let request = CodexRunRequest {
            cwd: PathBuf::from("/tmp"),
            codex_profile: "default".into(),
            model: Some("gpt-test".into()),
            reasoning_effort: Some("high".into()),
            reasoning_summary: Some("concise".into()),
            verbosity: Some("medium".into()),
            personality: Some("pragmatic".into()),
            service_tier: Some("fast".into()),
            sandbox_mode: "workspace_write".into(),
            approval_policy: "never".into(),
            network_access: false,
            web_search: "live".into(),
            feature_multi_agent: true,
            feature_remote_plugin: false,
            feature_hooks: true,
            feature_goals: false,
            feature_shell_tool: true,
            max_run_seconds: 60,
            prompt: "test".into(),
            existing_thread_id: None,
            run_token: "token".into(),
            environment: HashMap::new(),
            approval_handler: None,
            progress_handler: None,
            cancellation_handler: None,
        };
        let mut command = Command::new("codex");
        apply_managed_cli_settings(&mut command, &request);
        let args = command
            .as_std()
            .get_args()
            .map(|value| value.to_string_lossy().to_string())
            .collect::<Vec<_>>();

        assert!(args.contains(&"model_reasoning_summary=\"concise\"".into()));
        assert!(args.contains(&"model_verbosity=\"medium\"".into()));
        assert!(args.contains(&"service_tier=\"fast\"".into()));
        assert!(args.contains(&"web_search=\"live\"".into()));
        assert!(args.contains(&"sandbox_workspace_write.network_access=false".into()));
        assert!(args.contains(&"features.remote_plugin=false".into()));
        assert!(args.contains(&"features.shell_tool=true".into()));
    }

    #[derive(Default)]
    struct AcceptingApprovalHandler {
        calls: AtomicUsize,
    }

    struct AlwaysCancelHandler;

    impl CodexCancellationHandler for AlwaysCancelHandler {
        fn should_cancel(&self) -> bool {
            true
        }
    }

    #[async_trait]
    impl CodexApprovalHandler for AcceptingApprovalHandler {
        async fn request_approval(
            &self,
            request: CodexApprovalRequest,
        ) -> AppResult<CodexApprovalDecision> {
            assert_eq!(request.tool_name, AGENT_CODEX_APPROVAL_TOOL_COMMAND);
            assert_eq!(
                request
                    .arguments
                    .pointer("/params/command")
                    .and_then(Value::as_str),
                Some("git push origin relay/test")
            );
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(CodexApprovalDecision::Accept)
        }
    }

    #[test]
    fn parser_tracks_thread_and_final_message() {
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        let input = concat!(
            "{\"type\":\"thread.started\",\"thread_id\":\"thread-1\"}\n",
            "{\"type\":\"turn.started\"}\n",
            "{\"type\":\"item.completed\",\"item\":{\"type\":\"agent_message\",\"text\":\"done\"}}\n",
            "{\"type\":\"turn.completed\"}\n"
        );
        let events = runtime
            .block_on(read_jsonl_events(input.as_bytes(), None))
            .expect("events");
        assert_eq!(events.thread_id.as_deref(), Some("thread-1"));
        assert!(events.turn_started);
        assert!(events.turn_completed);
        assert_eq!(events.final_message.as_deref(), Some("done"));
    }

    #[test]
    fn mcp_progress_summary_includes_the_action() {
        let item = json!({
            "type": "mcp_tool_call",
            "server": "relay_company",
            "tool": "company.project",
            "arguments": { "action": "create", "name": "WMS" }
        });
        let (_, started) = summarize_codex_item(&item, false).expect("summary should exist");
        let (_, completed) = summarize_codex_item(&item, true).expect("summary should exist");
        assert_eq!(
            started,
            "正在调用工具：relay_company.company.project（action: create）"
        );
        assert_eq!(
            completed,
            "完成调用工具：relay_company.company.project（action: create）"
        );

        let failed_item = json!({
            "type": "mcp_tool_call",
            "server": "relay_company",
            "tool": "company.project",
            "arguments": "{\"action\":\"member_add\"}",
            "error": { "message": "Agent is still provisioning" }
        });
        let (_, failed) =
            summarize_codex_item(&failed_item, true).expect("failure summary should exist");
        assert_eq!(
            failed,
            "工具调用失败：relay_company.company.project（action: member_add）"
        );
    }

    #[test]
    fn mcp_discovery_view_removes_secrets_and_argument_contents() {
        let configured_names = std::collections::HashSet::from(["private-http".to_string()]);
        let http = safe_mcp_server_view(
            json!({
                "name": "private-http",
                "enabled": true,
                "auth_status": "bearer_token",
                "transport": {
                    "type": "streamable_http",
                    "url": "https://user:secret@example.com/mcp?token=secret#fragment",
                    "bearer_token_env_var": "PRIVATE_MCP_TOKEN",
                    "http_headers": {"Authorization": "Bearer secret"}
                }
            }),
            &configured_names,
        )
        .expect("HTTP MCP view");
        assert_eq!(http.address.as_deref(), Some("https://example.com/mcp"));
        assert_eq!(
            http.bearer_token_env_var.as_deref(),
            Some("PRIVATE_MCP_TOKEN")
        );
        assert!(http.configured_by_user);

        let stdio = safe_mcp_server_view(
            json!({
                "name": "local",
                "enabled": true,
                "transport": {
                    "type": "stdio",
                    "command": "/usr/local/bin/npx",
                    "args": ["-y", "@example/mcp", "--token", "secret"],
                    "env": {"PRIVATE_TOKEN": "secret"}
                }
            }),
            &std::collections::HashSet::new(),
        )
        .expect("stdio MCP view");
        assert_eq!(stdio.command.as_deref(), Some("npx"));
        assert_eq!(stdio.argument_count, 4);
        assert!(!stdio.configured_by_user);

        let serialized = serde_json::to_string(&(http, stdio)).expect("serialize safe views");
        assert!(!serialized.contains("secret"));
        assert!(!serialized.contains("Authorization"));
        assert!(!serialized.contains("PRIVATE_TOKEN"));
        assert!(!serialized.contains("@example/mcp"));
    }

    #[test]
    fn only_pre_turn_resume_errors_replace_a_session() {
        assert!(should_replace_session(&ProcessOutcome {
            status: CodexRunStatus::Failed,
            thread_id: None,
            exit_code: Some(1),
            final_message: None,
            error_message: Some("session not found; cannot resume".into()),
            turn_started: false,
        }));
        assert!(!should_replace_session(&ProcessOutcome {
            status: CodexRunStatus::Failed,
            thread_id: Some("thread-1".into()),
            exit_code: Some(1),
            final_message: None,
            error_message: Some("turn failed after command execution".into()),
            turn_started: true,
        }));
    }

    #[test]
    fn codex_prompt_accepts_multiline_instructions_but_rejects_unsafe_controls() {
        assert!(validate_prompt("先读取 Inbox。\n然后处理任务。\n\t没有任务时结束。").is_ok());
        assert!(validate_prompt("unsafe\rprompt").is_err());
        assert!(validate_prompt("unsafe\0prompt").is_err());
    }

    #[test]
    fn sandbox_mode_mapping_is_independent_from_approval_policy() {
        assert_eq!(
            codex_sandbox_mode("workspace_write").expect("workspace-write policy"),
            "workspace-write"
        );
        assert_eq!(
            codex_sandbox_mode("read_only").expect("read-only policy"),
            "read-only"
        );
    }

    #[test]
    fn default_auth_probe_reports_login_without_persisting_account_details() {
        assert_eq!(
            classify_default_auth_probe(
                true,
                "Logged in using an API key - sk-example***masked",
                ""
            ),
            CodexDefaultAuthProbe {
                status: "active".into(),
                method: Some("api_key".into()),
                config: CodexDefaultConfigSummary {
                    credential_hint: Some("sk-example***masked".into()),
                    ..CodexDefaultConfigSummary::default()
                },
            }
        );
        assert_eq!(
            classify_default_auth_probe(true, "Logged in using ChatGPT", ""),
            CodexDefaultAuthProbe {
                status: "active".into(),
                method: Some("chatgpt".into()),
                config: CodexDefaultConfigSummary::default(),
            }
        );
        assert_eq!(
            classify_default_auth_probe(false, "", "Not logged in"),
            CodexDefaultAuthProbe {
                status: "logged_out".into(),
                method: None,
                config: CodexDefaultConfigSummary::default(),
            }
        );
    }

    #[test]
    fn default_config_summary_exposes_only_safe_operational_metadata() {
        let mut summary = CodexDefaultConfigSummary::default();
        populate_default_config_summary(
            r#"
openai_base_url = "https://proxy.example.com/v1"
model_provider = "codex"
model = "gpt-5.6-sol"
model_reasoning_effort = "high"

[mcp_servers.relay]
url = "http://127.0.0.1:48181/mcp"
[mcp_servers.relay.env]
OPENAI_API_KEY = "must-not-be-returned"
[profiles.team]
model = "gpt-5.5"
[projects."/private/work"]
trust_level = "trusted"
[plugins."browser@openai-bundled"]
enabled = true
"#,
            &mut summary,
        );
        assert_eq!(summary.model_provider.as_deref(), Some("codex"));
        assert_eq!(
            summary.openai_base_url.as_deref(),
            Some("https://proxy.example.com/v1")
        );
        assert_eq!(summary.model.as_deref(), Some("gpt-5.6-sol"));
        assert_eq!(summary.reasoning_effort.as_deref(), Some("high"));
        assert_eq!(summary.mcp_servers, vec!["relay"]);
        assert_eq!(summary.named_profiles, vec!["team"]);
        assert_eq!(summary.trusted_project_count, 1);
        assert_eq!(summary.plugin_count, 1);
        assert!(!format!("{summary:?}").contains("must-not-be-returned"));
    }

    #[test]
    fn managed_base_url_config_replaces_only_the_root_codex_setting() {
        let rendered = render_openai_base_url_config(
            r#"model = "gpt-5.6-sol"
openai_base_url = "https://old.example.com/v1"

[profiles.team]
openai_base_url = "keep-this-profile-value"
model = "gpt-5.5"
"#,
            Some("https://new.example.com/v1"),
        );
        assert!(rendered.contains("openai_base_url = \"https://new.example.com/v1\""));
        assert!(!rendered.contains("https://old.example.com/v1"));
        assert!(rendered.contains("openai_base_url = \"keep-this-profile-value\""));
        assert_eq!(
            rendered
                .lines()
                .filter(|line| *line == "openai_base_url = \"https://new.example.com/v1\"")
                .count(),
            1
        );
    }

    #[test]
    fn managed_base_url_validation_rejects_credentials_and_query_tokens() {
        assert!(validate_managed_openai_base_url(Some("https://proxy.example.com/v1")).is_ok());
        assert!(
            validate_managed_openai_base_url(Some("https://user:secret@proxy.example.com/v1"))
                .is_err()
        );
        assert!(validate_managed_openai_base_url(Some(
            "https://proxy.example.com/v1?token=secret"
        ))
        .is_err());
    }

    #[test]
    fn managed_profile_uses_an_independent_codex_home_without_profile_argument() {
        let runner = CodexTriggerRunner::new(
            PathBuf::from("codex"),
            Vec::new(),
            "http://127.0.0.1:8080/mcp".into(),
            "relay_company".into(),
            DEFAULT_RUN_TOKEN_ENV.into(),
        )
        .expect("runner");
        let profile_id = Uuid::new_v4();
        let selector = format!("relay_{profile_id}");
        let mut command = Command::new("codex");
        runner
            .apply_profile_arguments(&mut command, &selector)
            .expect("managed arguments");
        runner
            .apply_profile_environment(&mut command, &selector)
            .expect("managed environment");
        assert!(!command
            .as_std()
            .get_args()
            .any(|argument| argument == "--profile"));
        let codex_home = command
            .as_std()
            .get_envs()
            .find(|(name, _)| *name == "CODEX_HOME")
            .and_then(|(_, value)| value)
            .expect("CODEX_HOME");
        assert!(codex_home
            .to_string_lossy()
            .ends_with(&profile_id.to_string()));
    }

    #[test]
    fn ordinary_codex_profile_still_uses_profile_argument() {
        let runner = CodexTriggerRunner::new(
            PathBuf::from("codex"),
            Vec::new(),
            "http://127.0.0.1:8080/mcp".into(),
            "relay_company".into(),
            DEFAULT_RUN_TOKEN_ENV.into(),
        )
        .expect("runner");
        let mut command = Command::new("codex");
        runner
            .apply_profile_arguments(&mut command, "team")
            .expect("ordinary arguments");
        assert_eq!(
            command
                .as_std()
                .get_args()
                .map(|argument| argument.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            vec!["--profile", "team"]
        );
    }

    #[cfg(unix)]
    #[test]
    fn running_codex_process_is_cancelled_when_the_project_pauses() {
        let workspace = std::env::temp_dir().join(format!(
            "relay-fake-codex-cancel-{}",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&workspace).expect("workspace");
        let runner = CodexTriggerRunner::new(
            PathBuf::from("/bin/sh"),
            vec!["-c".into(), "sleep 30".into(), "--".into()],
            "http://127.0.0.1:8080/mcp".into(),
            "relay_company".into(),
            DEFAULT_RUN_TOKEN_ENV.into(),
        )
        .expect("runner");
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        let result = runtime
            .block_on(runner.run(CodexRunRequest {
                cwd: workspace.clone(),
                codex_profile: "default".into(),
                model: None,
                reasoning_effort: None,
                reasoning_summary: Some("auto".into()),
                verbosity: None,
                personality: Some("pragmatic".into()),
                service_tier: None,
                sandbox_mode: "workspace_write".into(),
                approval_policy: "never".into(),
                network_access: true,
                web_search: "cached".into(),
                feature_multi_agent: true,
                feature_remote_plugin: true,
                feature_hooks: true,
                feature_goals: true,
                feature_shell_tool: true,
                max_run_seconds: 30,
                prompt: "work on project".into(),
                existing_thread_id: None,
                run_token: "art_test".into(),
                environment: HashMap::new(),
                approval_handler: None,
                progress_handler: None,
                cancellation_handler: Some(Arc::new(AlwaysCancelHandler)),
            }))
            .expect("fake Codex cancellation");
        assert_eq!(result.status, CodexRunStatus::Cancelled);
        assert!(result
            .error_message
            .as_deref()
            .is_some_and(|message| message.contains("project was paused")));
        std::fs::remove_dir_all(workspace).expect("cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn transient_reconnect_error_is_cleared_after_the_turn_completes() {
        let workspace = std::env::temp_dir().join(format!(
            "relay-fake-codex-reconnect-{}",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&workspace).expect("workspace");
        let script = r#"printf '%s\n' '{"type":"thread.started","thread_id":"thread-reconnect"}' '{"type":"turn.started"}' '{"type":"error","message":"Reconnecting... 1/5 (stream disconnected before completion)"}' '{"type":"item.completed","item":{"type":"agent_message","text":"work completed after reconnect"}}' '{"type":"turn.completed"}'"#;
        let runner = CodexTriggerRunner::new(
            PathBuf::from("/bin/sh"),
            vec!["-c".into(), script.into(), "--".into()],
            "http://127.0.0.1:8080/mcp".into(),
            "relay_company".into(),
            DEFAULT_RUN_TOKEN_ENV.into(),
        )
        .expect("runner");
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        let result = runtime
            .block_on(runner.run(CodexRunRequest {
                cwd: workspace.clone(),
                codex_profile: "default".into(),
                model: None,
                reasoning_effort: None,
                reasoning_summary: Some("auto".into()),
                verbosity: None,
                personality: Some("pragmatic".into()),
                service_tier: None,
                sandbox_mode: "workspace_write".into(),
                approval_policy: "never".into(),
                network_access: true,
                web_search: "cached".into(),
                feature_multi_agent: true,
                feature_remote_plugin: true,
                feature_hooks: true,
                feature_goals: true,
                feature_shell_tool: true,
                max_run_seconds: 10,
                prompt: "check Relay inbox".into(),
                existing_thread_id: None,
                run_token: "art_test".into(),
                environment: HashMap::new(),
                approval_handler: None,
                progress_handler: None,
                cancellation_handler: None,
            }))
            .expect("fake Codex run");
        assert_eq!(result.status, CodexRunStatus::Succeeded);
        assert_eq!(
            result.final_message.as_deref(),
            Some("work completed after reconnect")
        );
        assert_eq!(result.error_message, None);
        std::fs::remove_dir_all(workspace).expect("cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn fake_codex_replaces_only_an_unresumable_thread() {
        let workspace = std::env::temp_dir().join(format!(
            "relay-fake-codex-{}",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&workspace).expect("workspace");
        let script = r#"if [ "$1" = "--version" ]; then echo fake-codex-1.0; exit 0; fi; for arg in "$@"; do if [ "$arg" = "resume" ]; then echo 'session not found; cannot resume' >&2; exit 1; fi; done; printf '%s\n' '{"type":"thread.started","thread_id":"new-thread"}' '{"type":"turn.started"}' '{"type":"item.completed","item":{"type":"agent_message","text":"handled"}}' '{"type":"turn.completed"}'"#;
        let runner = CodexTriggerRunner::new(
            PathBuf::from("/bin/sh"),
            vec!["-c".into(), script.into(), "--".into()],
            "http://127.0.0.1:8080/mcp".into(),
            "relay_company".into(),
            DEFAULT_RUN_TOKEN_ENV.into(),
        )
        .expect("runner");
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        let result = runtime
            .block_on(runner.run(CodexRunRequest {
                cwd: workspace.clone(),
                codex_profile: "relay-test".into(),
                model: Some("gpt-5.6-sol".into()),
                reasoning_effort: Some("high".into()),
                reasoning_summary: Some("auto".into()),
                verbosity: Some("medium".into()),
                personality: Some("pragmatic".into()),
                service_tier: None,
                sandbox_mode: "workspace_write".into(),
                approval_policy: "never".into(),
                network_access: true,
                web_search: "cached".into(),
                feature_multi_agent: true,
                feature_remote_plugin: true,
                feature_hooks: true,
                feature_goals: true,
                feature_shell_tool: true,
                max_run_seconds: 10,
                prompt: "check Relay inbox".into(),
                existing_thread_id: Some("missing-thread".into()),
                run_token: "art_test".into(),
                environment: HashMap::new(),
                approval_handler: None,
                progress_handler: None,
                cancellation_handler: None,
            }))
            .expect("fake Codex run");
        assert_eq!(result.status, CodexRunStatus::Succeeded);
        assert_eq!(result.thread_id.as_deref(), Some("new-thread"));
        assert!(result.replaced_unresumable_session);
        assert!(!result.resumed_existing_session);
        std::fs::remove_dir_all(workspace).expect("cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn app_server_approval_continues_the_same_turn() {
        let workspace = std::env::temp_dir().join(format!(
            "relay-fake-app-server-{}",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&workspace).expect("workspace");
        let script = r#"IFS= read -r initialize
printf '%s\n' '{"id":0,"result":{"userAgent":"fake","platformFamily":"unix","platformOs":"macos","codexHome":"/tmp"}}'
IFS= read -r initialized
IFS= read -r thread
printf '%s\n' '{"id":1,"result":{"thread":{"id":"thread-approval"},"model":"fake","modelProvider":"fake","cwd":"/tmp","approvalPolicy":"on-request","approvalsReviewer":"user","sandbox":{"type":"workspaceWrite","writableRoots":[],"readOnlyAccess":{"type":"fullAccess"},"networkAccess":true,"excludeTmpdirEnvVar":false,"excludeSlashTmp":false}}}'
IFS= read -r turn
case "$turn" in *'"effort":"high"'*) ;; *) exit 8 ;; esac
printf '%s\n' '{"id":2,"result":{"turn":{"id":"turn-1","items":[],"status":"inProgress"}}}'
printf '%s\n' '{"method":"turn/started","params":{"threadId":"thread-approval","turn":{"id":"turn-1","items":[],"status":"inProgress"}}}'
printf '%s\n' '{"id":99,"method":"item/commandExecution/requestApproval","params":{"threadId":"thread-approval","turnId":"turn-1","itemId":"item-1","startedAtMs":1,"command":"git push origin relay/test","cwd":"/tmp","reason":"push branch"}}'
IFS= read -r approval
case "$approval" in *'"decision":"accept"'*) ;; *) exit 9 ;; esac
printf '%s\n' '{"method":"item/completed","params":{"threadId":"thread-approval","turnId":"turn-1","completedAtMs":2,"item":{"id":"message-1","type":"agentMessage","text":"push completed"}}}'
printf '%s\n' '{"method":"turn/completed","params":{"threadId":"thread-approval","turn":{"id":"turn-1","items":[{"id":"message-1","type":"agentMessage","text":"push completed"}],"status":"completed"}}}'"#;
        let script_path = workspace.join("fake-app-server.sh");
        std::fs::write(&script_path, script).expect("fake app-server script");
        let runner = CodexTriggerRunner::new(
            PathBuf::from("/bin/sh"),
            vec![script_path.to_string_lossy().into_owned()],
            "http://127.0.0.1:8080/mcp".into(),
            "relay_company".into(),
            DEFAULT_RUN_TOKEN_ENV.into(),
        )
        .expect("runner");
        let handler = Arc::new(AcceptingApprovalHandler::default());
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        let result = runtime
            .block_on(runner.run(CodexRunRequest {
                cwd: workspace.clone(),
                codex_profile: "default".into(),
                model: None,
                reasoning_effort: Some("high".into()),
                reasoning_summary: Some("auto".into()),
                verbosity: None,
                personality: Some("pragmatic".into()),
                service_tier: None,
                sandbox_mode: "workspace_write".into(),
                approval_policy: "on-request".into(),
                network_access: true,
                web_search: "cached".into(),
                feature_multi_agent: true,
                feature_remote_plugin: true,
                feature_hooks: true,
                feature_goals: true,
                feature_shell_tool: true,
                max_run_seconds: 10,
                prompt: "push the branch".into(),
                existing_thread_id: None,
                run_token: "art_test".into(),
                environment: HashMap::new(),
                approval_handler: Some(handler.clone()),
                progress_handler: None,
                cancellation_handler: None,
            }))
            .expect("fake app-server run");
        assert_eq!(result.status, CodexRunStatus::Succeeded);
        assert_eq!(result.thread_id.as_deref(), Some("thread-approval"));
        assert_eq!(result.final_message.as_deref(), Some("push completed"));
        assert_eq!(handler.calls.load(Ordering::SeqCst), 1);
        std::fs::remove_dir_all(workspace).expect("cleanup");
    }
}
