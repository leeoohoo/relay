use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

use chrono::{DateTime, Utc};
use fs2::FileExt;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use uuid::Uuid;

use ai_chat_shared::{now_utc, AppError, AppResult};

pub const CODEX_AUTH_PROFILE_STATUS_PENDING: &str = "pending";
pub const CODEX_AUTH_PROFILE_STATUS_ACTIVE: &str = "active";
pub const CODEX_AUTH_PROFILE_STATUS_FAILED: &str = "failed";
pub const CODEX_AUTH_PROFILE_STATUS_DELETING: &str = "deleting";

pub const CODEX_CLI_OPERATION_IDLE: &str = "idle";
pub const CODEX_CLI_OPERATION_INSTALL_PENDING: &str = "install_pending";
pub const CODEX_CLI_OPERATION_INSTALLING: &str = "installing";
pub const CODEX_CLI_OPERATION_UPDATE_PENDING: &str = "update_pending";
pub const CODEX_CLI_OPERATION_UPDATING: &str = "updating";
pub const CODEX_CLI_OPERATION_FAILED: &str = "failed";

pub const CODEX_DEFAULT_AUTH_STATUS_UNKNOWN: &str = "unknown";
pub const CODEX_DEFAULT_AUTH_STATUS_ACTIVE: &str = "active";
pub const CODEX_DEFAULT_AUTH_STATUS_LOGGED_OUT: &str = "logged_out";

pub const CODEX_INSTALLER_POSIX_SHELL: &str = "posix_shell";
pub const CODEX_INSTALLER_POWERSHELL: &str = "powershell";
pub const CODEX_INSTALLER_UNSUPPORTED: &str = "unsupported";

pub const CODEX_MCP_TRANSPORT_STDIO: &str = "stdio";
pub const CODEX_MCP_TRANSPORT_HTTP: &str = "streamable_http";
pub const CODEX_MCP_OPERATION_IDLE: &str = "idle";
pub const CODEX_MCP_OPERATION_REFRESH_PENDING: &str = "refresh_pending";
pub const CODEX_MCP_OPERATION_REFRESHING: &str = "refreshing";
pub const CODEX_MCP_OPERATION_ADD_PENDING: &str = "add_pending";
pub const CODEX_MCP_OPERATION_ADDING: &str = "adding";
pub const CODEX_MCP_OPERATION_REMOVE_PENDING: &str = "remove_pending";
pub const CODEX_MCP_OPERATION_REMOVING: &str = "removing";
pub const CODEX_MCP_OPERATION_FAILED: &str = "failed";
pub const CODEX_DEFAULT_OPENAI_BASE_URL: &str = "https://api.openai.com/v1";

pub const CODEX_REASONING_SUMMARY_AUTO: &str = "auto";
pub const CODEX_WEB_SEARCH_CACHED: &str = "cached";
pub const CODEX_PERSONALITY_PRAGMATIC: &str = "pragmatic";
pub const CODEX_APPROVAL_POLICY_NEVER: &str = "never";
pub const CODEX_SANDBOX_WORKSPACE_WRITE: &str = "workspace_write";
pub const AGENT_TRIGGER_BATCH_SIZE_MIN: usize = 1;
pub const AGENT_TRIGGER_BATCH_SIZE_MAX: usize = 100;
pub const AGENT_TRIGGER_BATCH_SIZE_DEFAULT: usize = 10;

pub fn agent_trigger_batch_size_from_env() -> usize {
    std::env::var("AGENT_TRIGGER_BATCH_SIZE")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .map(|value| value.clamp(AGENT_TRIGGER_BATCH_SIZE_MIN, AGENT_TRIGGER_BATCH_SIZE_MAX))
        .unwrap_or(AGENT_TRIGGER_BATCH_SIZE_DEFAULT)
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AgentTriggerPreferences {
    pub batch_size: usize,
    pub environment_default: usize,
    pub managed: bool,
    pub updated_by_human_user_id: Option<Uuid>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct AgentTriggerPreferencesFile {
    batch_size: Option<usize>,
    updated_by_human_user_id: Option<Uuid>,
    updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct CodexDefaultConfigSummary {
    pub codex_home: Option<String>,
    pub config_path: Option<String>,
    pub config_exists: bool,
    pub auth_path: Option<String>,
    pub auth_exists: bool,
    pub credential_hint: Option<String>,
    pub openai_base_url: Option<String>,
    pub model_provider: Option<String>,
    pub model: Option<String>,
    pub reasoning_effort: Option<String>,
    pub sandbox_mode: Option<String>,
    pub approval_policy: Option<String>,
    pub mcp_servers: Vec<String>,
    pub named_profiles: Vec<String>,
    pub trusted_project_count: usize,
    pub plugin_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CodexDefaultAuthEnvironment {
    pub selector: String,
    pub name: String,
    pub status: String,
    pub method: Option<String>,
    pub last_checked_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    #[serde(default)]
    pub config: CodexDefaultConfigSummary,
}

impl Default for CodexDefaultAuthEnvironment {
    fn default() -> Self {
        Self {
            selector: "default".into(),
            name: "宿主机默认登录".into(),
            status: CODEX_DEFAULT_AUTH_STATUS_UNKNOWN.into(),
            method: None,
            last_checked_at: None,
            last_error: None,
            config: CodexDefaultConfigSummary::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexCliRuntime {
    pub installed: bool,
    pub source: String,
    pub executable_path: Option<String>,
    pub installed_version: Option<String>,
    pub latest_version: Option<String>,
    pub update_available: bool,
    pub operation_status: String,
    pub last_checked_at: Option<DateTime<Utc>>,
    pub update_check_error: Option<String>,
    pub last_error: Option<String>,
    #[serde(default = "current_host_os")]
    pub host_os: String,
    #[serde(default = "current_host_arch")]
    pub host_arch: String,
    #[serde(default = "current_installer_kind")]
    pub installer_kind: String,
    #[serde(default = "current_installation_supported")]
    pub installation_supported: bool,
    #[serde(default)]
    pub default_auth: CodexDefaultAuthEnvironment,
    pub updated_at: DateTime<Utc>,
}

impl Default for CodexCliRuntime {
    fn default() -> Self {
        Self {
            installed: false,
            source: "unavailable".into(),
            executable_path: None,
            installed_version: None,
            latest_version: None,
            update_available: false,
            operation_status: CODEX_CLI_OPERATION_IDLE.into(),
            last_checked_at: None,
            update_check_error: None,
            last_error: None,
            host_os: current_host_os(),
            host_arch: current_host_arch(),
            installer_kind: current_installer_kind(),
            installation_supported: current_installation_supported(),
            default_auth: CodexDefaultAuthEnvironment::default(),
            updated_at: now_utc(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CodexAuthProfile {
    pub id: Uuid,
    pub company_id: Uuid,
    pub name: String,
    pub selector: String,
    #[serde(default)]
    pub base_url: Option<String>,
    pub status: String,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CompanyCodexCliSettings {
    pub company_id: Uuid,
    pub model: Option<String>,
    pub reasoning_effort: Option<String>,
    pub reasoning_summary: String,
    pub verbosity: Option<String>,
    pub personality: Option<String>,
    pub service_tier: Option<String>,
    pub approval_policy: String,
    pub sandbox_mode: String,
    pub network_access: bool,
    pub web_search: String,
    pub feature_multi_agent: bool,
    pub feature_remote_plugin: bool,
    pub feature_hooks: bool,
    pub feature_goals: bool,
    pub feature_shell_tool: bool,
    pub updated_at: DateTime<Utc>,
}

impl CompanyCodexCliSettings {
    pub fn new(company_id: Uuid) -> Self {
        Self {
            company_id,
            model: None,
            reasoning_effort: None,
            reasoning_summary: CODEX_REASONING_SUMMARY_AUTO.into(),
            verbosity: None,
            personality: Some(CODEX_PERSONALITY_PRAGMATIC.into()),
            service_tier: None,
            approval_policy: CODEX_APPROVAL_POLICY_NEVER.into(),
            sandbox_mode: CODEX_SANDBOX_WORKSPACE_WRITE.into(),
            network_access: true,
            web_search: CODEX_WEB_SEARCH_CACHED.into(),
            feature_multi_agent: true,
            feature_remote_plugin: true,
            feature_hooks: true,
            feature_goals: true,
            feature_shell_tool: true,
            updated_at: now_utc(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct CompanyCodexCliSettingsFile {
    settings: Vec<CompanyCodexCliSettings>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct CodexMcpServerView {
    pub name: String,
    pub transport: String,
    pub enabled: bool,
    pub auth_status: Option<String>,
    pub address: Option<String>,
    pub command: Option<String>,
    pub argument_count: usize,
    pub bearer_token_env_var: Option<String>,
    pub startup_timeout_sec: Option<u64>,
    pub tool_timeout_sec: Option<u64>,
    pub disabled_reason: Option<String>,
    pub configured_by_user: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CodexMcpEnvironmentSnapshot {
    pub selector: String,
    pub status: String,
    pub operation_status: String,
    pub pending_server_name: Option<String>,
    pub servers: Vec<CodexMcpServerView>,
    pub last_checked_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
}

impl CodexMcpEnvironmentSnapshot {
    fn new(selector: String) -> Self {
        Self {
            selector,
            status: "unknown".into(),
            operation_status: CODEX_MCP_OPERATION_IDLE.into(),
            pending_server_name: None,
            servers: Vec::new(),
            last_checked_at: None,
            last_error: None,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct CodexMcpCatalogFile {
    environments: Vec<CodexMcpEnvironmentSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CodexMcpServerInput {
    pub target_selector: String,
    pub name: String,
    pub transport: String,
    pub url: Option<String>,
    pub command: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    pub bearer_token_env_var: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct CodexAuthProfileFile {
    profiles: Vec<CodexAuthProfile>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CodexEnvironmentView {
    pub runtime: CodexCliRuntime,
    pub profiles: Vec<CodexAuthProfile>,
    pub mcp_environments: Vec<CodexMcpEnvironmentSnapshot>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CodexControlRequestKind {
    InstallCli,
    UpdateCli,
    ProvisionAuth,
    DeleteAuth,
    RefreshMcp,
    AddMcp,
    RemoveMcp,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct CodexControlRequest {
    pub id: Uuid,
    pub kind: CodexControlRequestKind,
    pub company_id: Option<Uuid>,
    pub profile_id: Option<Uuid>,
    pub api_key: Option<String>,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub target_selector: Option<String>,
    #[serde(default)]
    pub mcp_server: Option<CodexMcpServerInput>,
    #[serde(default)]
    pub mcp_server_name: Option<String>,
    pub created_at: DateTime<Utc>,
}

pub struct ClaimedCodexControlRequest {
    pub request: CodexControlRequest,
    path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct CodexControlStore {
    control_root: PathBuf,
    state_root: PathBuf,
}

impl CodexControlStore {
    pub fn from_env() -> AppResult<Self> {
        let state_root = std::env::var("AGENT_TRIGGER_STATE_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(".relay-agent-trigger"));
        let control_root = std::env::var("AGENT_TRIGGER_CODEX_CONTROL_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|_| state_root.join("codex-control"));
        Self::new_with_state_root(control_root, state_root)
    }

    pub fn new(control_root: PathBuf) -> AppResult<Self> {
        let state_root = control_root
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        Self::new_with_state_root(control_root, state_root)
    }

    pub fn new_with_state_root(control_root: PathBuf, state_root: PathBuf) -> AppResult<Self> {
        let store = Self {
            control_root,
            state_root,
        };
        store.ensure_layout()?;
        Ok(store)
    }

    pub fn control_root(&self) -> &Path {
        &self.control_root
    }

    pub fn managed_cli_bin_dir(&self) -> PathBuf {
        self.state_root.join("codex-cli").join("bin")
    }

    pub fn managed_cli_executable(&self) -> PathBuf {
        self.managed_cli_bin_dir()
            .join(if cfg!(windows) { "codex.exe" } else { "codex" })
    }

    pub fn managed_cli_home(&self) -> PathBuf {
        self.state_root.join("codex-cli").join("home")
    }

    pub fn managed_profile_home(&self, profile_id: Uuid) -> PathBuf {
        self.state_root
            .join("codex-profiles")
            .join("homes")
            .join(profile_id.to_string())
    }

    pub fn managed_profile_home_for_selector(&self, selector: &str) -> Option<PathBuf> {
        managed_profile_id(selector).map(|id| self.managed_profile_home(id))
    }

    pub fn environment_for_company(&self, company_id: Uuid) -> AppResult<CodexEnvironmentView> {
        let _lock = self.lock()?;
        let runtime = self.read_runtime_unlocked()?;
        let mut profiles = self.read_profiles_unlocked()?.profiles;
        profiles.retain(|profile| profile.company_id == company_id);
        profiles.sort_by_key(|profile| (profile.created_at, profile.id));
        let mut allowed_selectors = profiles
            .iter()
            .filter(|profile| profile.status == CODEX_AUTH_PROFILE_STATUS_ACTIVE)
            .map(|profile| profile.selector.as_str())
            .collect::<std::collections::HashSet<_>>();
        allowed_selectors.insert("default");
        let mut mcp_environments = self.read_mcp_catalog_unlocked()?.environments;
        mcp_environments.retain(|snapshot| allowed_selectors.contains(snapshot.selector.as_str()));
        if !mcp_environments
            .iter()
            .any(|snapshot| snapshot.selector == "default")
        {
            mcp_environments.push(CodexMcpEnvironmentSnapshot::new("default".into()));
        }
        mcp_environments.sort_by(|left, right| left.selector.cmp(&right.selector));
        Ok(CodexEnvironmentView {
            runtime,
            profiles,
            mcp_environments,
        })
    }

    pub fn runtime(&self) -> AppResult<CodexCliRuntime> {
        let _lock = self.lock()?;
        self.read_runtime_unlocked()
    }

    pub fn agent_trigger_preferences(
        &self,
        environment_default: usize,
    ) -> AppResult<AgentTriggerPreferences> {
        let environment_default = validate_agent_trigger_batch_size(environment_default)?;
        let _lock = self.lock()?;
        let file = self.read_trigger_preferences_unlocked()?;
        let batch_size = match file.batch_size {
            Some(value) => validate_agent_trigger_batch_size(value)?,
            None => environment_default,
        };
        Ok(AgentTriggerPreferences {
            batch_size,
            environment_default,
            managed: file.batch_size.is_some(),
            updated_by_human_user_id: file.updated_by_human_user_id,
            updated_at: file.updated_at,
        })
    }

    pub fn save_agent_trigger_preferences(
        &self,
        batch_size: usize,
        updated_by_human_user_id: Uuid,
        environment_default: usize,
    ) -> AppResult<AgentTriggerPreferences> {
        let batch_size = validate_agent_trigger_batch_size(batch_size)?;
        let environment_default = validate_agent_trigger_batch_size(environment_default)?;
        let _lock = self.lock()?;
        let updated_at = now_utc();
        self.write_trigger_preferences_unlocked(&AgentTriggerPreferencesFile {
            batch_size: Some(batch_size),
            updated_by_human_user_id: Some(updated_by_human_user_id),
            updated_at: Some(updated_at),
        })?;
        Ok(AgentTriggerPreferences {
            batch_size,
            environment_default,
            managed: true,
            updated_by_human_user_id: Some(updated_by_human_user_id),
            updated_at: Some(updated_at),
        })
    }

    pub fn company_cli_settings(&self, company_id: Uuid) -> AppResult<CompanyCodexCliSettings> {
        let _lock = self.lock()?;
        Ok(self
            .read_cli_settings_unlocked()?
            .settings
            .into_iter()
            .find(|settings| settings.company_id == company_id)
            .unwrap_or_else(|| CompanyCodexCliSettings::new(company_id)))
    }

    pub fn save_company_cli_settings(
        &self,
        mut settings: CompanyCodexCliSettings,
    ) -> AppResult<CompanyCodexCliSettings> {
        validate_company_cli_settings(&settings)?;
        let _lock = self.lock()?;
        let mut file = self.read_cli_settings_unlocked()?;
        settings.updated_at = now_utc();
        if let Some(existing) = file
            .settings
            .iter_mut()
            .find(|existing| existing.company_id == settings.company_id)
        {
            *existing = settings.clone();
        } else {
            file.settings.push(settings.clone());
        }
        self.write_cli_settings_unlocked(&file)?;
        Ok(settings)
    }

    pub fn list_active_profile_selectors(&self) -> AppResult<Vec<String>> {
        let _lock = self.lock()?;
        let mut selectors = self
            .read_profiles_unlocked()?
            .profiles
            .into_iter()
            .filter(|profile| profile.status == CODEX_AUTH_PROFILE_STATUS_ACTIVE)
            .map(|profile| profile.selector)
            .collect::<Vec<_>>();
        selectors.sort();
        selectors.dedup();
        Ok(selectors)
    }

    pub fn find_active_company_profile(
        &self,
        company_id: Uuid,
        selector: &str,
    ) -> AppResult<Option<CodexAuthProfile>> {
        let _lock = self.lock()?;
        Ok(self
            .read_profiles_unlocked()?
            .profiles
            .into_iter()
            .find(|profile| {
                profile.company_id == company_id
                    && profile.selector == selector
                    && profile.status == CODEX_AUTH_PROFILE_STATUS_ACTIVE
            }))
    }

    pub fn create_auth_profile(
        &self,
        company_id: Uuid,
        name: String,
        api_key: String,
        base_url: Option<String>,
    ) -> AppResult<CodexAuthProfile> {
        validate_profile_name(&name)?;
        validate_api_key(&api_key)?;
        let base_url = Some(normalize_codex_base_url(base_url.as_deref())?);
        let _lock = self.lock()?;
        let now = now_utc();
        let id = Uuid::new_v4();
        let profile = CodexAuthProfile {
            id,
            company_id,
            name,
            selector: managed_profile_selector(id),
            base_url: base_url.clone(),
            status: CODEX_AUTH_PROFILE_STATUS_PENDING.into(),
            last_error: None,
            created_at: now,
            updated_at: now,
        };
        let mut file = self.read_profiles_unlocked()?;
        file.profiles.push(profile.clone());
        let request = CodexControlRequest {
            id: Uuid::new_v4(),
            kind: CodexControlRequestKind::ProvisionAuth,
            company_id: Some(company_id),
            profile_id: Some(id),
            api_key: Some(api_key),
            base_url,
            target_selector: None,
            mcp_server: None,
            mcp_server_name: None,
            created_at: now,
        };
        self.write_request_unlocked(&request)?;
        if let Err(error) = self.write_profiles_unlocked(&file) {
            let _ = fs::remove_file(self.queued_request_path(request.id));
            return Err(error);
        }
        Ok(profile)
    }

    pub fn update_auth_profile(
        &self,
        company_id: Uuid,
        profile_id: Uuid,
        name: String,
        api_key: Option<String>,
        base_url: Option<String>,
    ) -> AppResult<CodexAuthProfile> {
        validate_profile_name(&name)?;
        if let Some(api_key) = api_key.as_deref() {
            validate_api_key(api_key)?;
        }
        let base_url = Some(normalize_codex_base_url(base_url.as_deref())?);
        let _lock = self.lock()?;
        let mut file = self.read_profiles_unlocked()?;
        let profile = file
            .profiles
            .iter_mut()
            .find(|profile| profile.id == profile_id && profile.company_id == company_id)
            .ok_or_else(|| AppError::NotFound("Codex authentication profile not found".into()))?;
        if profile.status == CODEX_AUTH_PROFILE_STATUS_DELETING {
            return Err(AppError::Conflict(
                "Codex authentication profile is being deleted".into(),
            ));
        }
        profile.name = name;
        profile.updated_at = now_utc();
        let base_url_changed = profile.base_url != base_url;
        profile.base_url = base_url.clone();
        let request_id = if api_key.is_some() || base_url_changed {
            profile.status = CODEX_AUTH_PROFILE_STATUS_PENDING.into();
            profile.last_error = None;
            let request = CodexControlRequest {
                id: Uuid::new_v4(),
                kind: CodexControlRequestKind::ProvisionAuth,
                company_id: Some(company_id),
                profile_id: Some(profile_id),
                api_key,
                base_url,
                target_selector: None,
                mcp_server: None,
                mcp_server_name: None,
                created_at: now_utc(),
            };
            self.write_request_unlocked(&request)?;
            Some(request.id)
        } else {
            None
        };
        let result = profile.clone();
        if let Err(error) = self.write_profiles_unlocked(&file) {
            if let Some(request_id) = request_id {
                let _ = fs::remove_file(self.queued_request_path(request_id));
            }
            return Err(error);
        }
        Ok(result)
    }

    pub fn request_delete_auth_profile(
        &self,
        company_id: Uuid,
        profile_id: Uuid,
    ) -> AppResult<CodexAuthProfile> {
        let _lock = self.lock()?;
        let mut file = self.read_profiles_unlocked()?;
        let profile = file
            .profiles
            .iter_mut()
            .find(|profile| profile.id == profile_id && profile.company_id == company_id)
            .ok_or_else(|| AppError::NotFound("Codex authentication profile not found".into()))?;
        if profile.status != CODEX_AUTH_PROFILE_STATUS_DELETING {
            profile.status = CODEX_AUTH_PROFILE_STATUS_DELETING.into();
            profile.last_error = None;
            profile.updated_at = now_utc();
            self.write_request_unlocked(&CodexControlRequest {
                id: Uuid::new_v4(),
                kind: CodexControlRequestKind::DeleteAuth,
                company_id: Some(company_id),
                profile_id: Some(profile_id),
                api_key: None,
                base_url: None,
                target_selector: None,
                mcp_server: None,
                mcp_server_name: None,
                created_at: now_utc(),
            })?;
        }
        let result = profile.clone();
        self.write_profiles_unlocked(&file)?;
        Ok(result)
    }

    pub fn enqueue_cli_install(&self) -> AppResult<CodexCliRuntime> {
        self.enqueue_cli_request(
            CodexControlRequestKind::InstallCli,
            CODEX_CLI_OPERATION_INSTALL_PENDING,
        )
    }

    pub fn enqueue_cli_update(&self) -> AppResult<CodexCliRuntime> {
        let runtime = self.runtime()?;
        if !runtime.installed {
            return Err(AppError::Conflict("Codex CLI is not installed".into()));
        }
        if !runtime.update_available {
            return Err(AppError::Conflict(
                "no confirmed Codex CLI update is available".into(),
            ));
        }
        self.enqueue_cli_request(
            CodexControlRequestKind::UpdateCli,
            CODEX_CLI_OPERATION_UPDATE_PENDING,
        )
    }

    fn enqueue_cli_request(
        &self,
        kind: CodexControlRequestKind,
        pending_status: &str,
    ) -> AppResult<CodexCliRuntime> {
        let _lock = self.lock()?;
        let mut runtime = self.read_runtime_unlocked()?;
        if runtime.operation_status != CODEX_CLI_OPERATION_IDLE
            && runtime.operation_status != CODEX_CLI_OPERATION_FAILED
        {
            return Err(AppError::Conflict(
                "another Codex CLI operation is already pending".into(),
            ));
        }
        self.write_request_unlocked(&CodexControlRequest {
            id: Uuid::new_v4(),
            kind,
            company_id: None,
            profile_id: None,
            api_key: None,
            base_url: None,
            target_selector: None,
            mcp_server: None,
            mcp_server_name: None,
            created_at: now_utc(),
        })?;
        runtime.operation_status = pending_status.into();
        runtime.last_error = None;
        runtime.updated_at = now_utc();
        self.write_runtime_unlocked(&runtime)?;
        Ok(runtime)
    }

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

    pub fn list_mcp_target_selectors(&self) -> AppResult<Vec<String>> {
        let _lock = self.lock()?;
        let mut selectors = vec!["default".to_string()];
        selectors.extend(
            self.read_profiles_unlocked()?
                .profiles
                .into_iter()
                .filter(|profile| profile.status == CODEX_AUTH_PROFILE_STATUS_ACTIVE)
                .map(|profile| profile.selector),
        );
        selectors.sort();
        selectors.dedup();
        Ok(selectors)
    }

    pub fn enqueue_mcp_refresh(&self, company_id: Uuid, selector: String) -> AppResult<()> {
        let _lock = self.lock()?;
        self.ensure_company_selector_unlocked(company_id, &selector)?;
        self.ensure_no_pending_mcp_operation_unlocked(&selector)?;
        self.write_request_unlocked(&CodexControlRequest {
            id: Uuid::new_v4(),
            kind: CodexControlRequestKind::RefreshMcp,
            company_id: Some(company_id),
            profile_id: None,
            api_key: None,
            base_url: None,
            target_selector: Some(selector.clone()),
            mcp_server: None,
            mcp_server_name: None,
            created_at: now_utc(),
        })?;
        self.update_mcp_operation_unlocked(
            &selector,
            CODEX_MCP_OPERATION_REFRESH_PENDING,
            None,
            None,
        )?;
        Ok(())
    }

    pub fn enqueue_mcp_add(&self, company_id: Uuid, input: CodexMcpServerInput) -> AppResult<()> {
        validate_mcp_server_input(&input)?;
        let _lock = self.lock()?;
        self.ensure_company_selector_unlocked(company_id, &input.target_selector)?;
        self.ensure_no_pending_mcp_operation_unlocked(&input.target_selector)?;
        let selector = input.target_selector.clone();
        let server_name = input.name.clone();
        self.write_request_unlocked(&CodexControlRequest {
            id: Uuid::new_v4(),
            kind: CodexControlRequestKind::AddMcp,
            company_id: Some(company_id),
            profile_id: None,
            api_key: None,
            base_url: None,
            target_selector: Some(selector.clone()),
            mcp_server: Some(input),
            mcp_server_name: Some(server_name.clone()),
            created_at: now_utc(),
        })?;
        self.update_mcp_operation_unlocked(
            &selector,
            CODEX_MCP_OPERATION_ADD_PENDING,
            Some(server_name),
            None,
        )?;
        Ok(())
    }

    pub fn enqueue_mcp_remove(
        &self,
        company_id: Uuid,
        selector: String,
        server_name: String,
    ) -> AppResult<()> {
        validate_mcp_selector(&selector)?;
        validate_mcp_server_name(&server_name)?;
        let _lock = self.lock()?;
        self.ensure_company_selector_unlocked(company_id, &selector)?;
        self.ensure_no_pending_mcp_operation_unlocked(&selector)?;
        let snapshot = self
            .read_mcp_catalog_unlocked()?
            .environments
            .into_iter()
            .find(|snapshot| snapshot.selector == selector)
            .ok_or_else(|| {
                AppError::Conflict("refresh the MCP list before removing a server".into())
            })?;
        let server = snapshot
            .servers
            .iter()
            .find(|server| server.name == server_name)
            .ok_or_else(|| AppError::NotFound("Codex MCP server not found".into()))?;
        if !server.configured_by_user {
            return Err(AppError::Conflict(
                "plugin-provided MCP servers cannot be removed from user Codex configuration"
                    .into(),
            ));
        }
        self.write_request_unlocked(&CodexControlRequest {
            id: Uuid::new_v4(),
            kind: CodexControlRequestKind::RemoveMcp,
            company_id: Some(company_id),
            profile_id: None,
            api_key: None,
            base_url: None,
            target_selector: Some(selector.clone()),
            mcp_server: None,
            mcp_server_name: Some(server_name.clone()),
            created_at: now_utc(),
        })?;
        self.update_mcp_operation_unlocked(
            &selector,
            CODEX_MCP_OPERATION_REMOVE_PENDING,
            Some(server_name),
            None,
        )?;
        Ok(())
    }

    pub fn publish_mcp_snapshot(
        &self,
        selector: &str,
        servers: Vec<CodexMcpServerView>,
        error: Option<String>,
    ) -> AppResult<()> {
        validate_mcp_selector(selector)?;
        let _lock = self.lock()?;
        let mut catalog = self.read_mcp_catalog_unlocked()?;
        let snapshot = mcp_snapshot_mut(&mut catalog, selector);
        snapshot.status = if error.is_some() { "failed" } else { "ready" }.into();
        snapshot.operation_status = if error.is_some() {
            CODEX_MCP_OPERATION_FAILED
        } else {
            CODEX_MCP_OPERATION_IDLE
        }
        .into();
        snapshot.pending_server_name = None;
        if error.is_none() {
            snapshot.servers = servers;
        }
        snapshot.last_checked_at = Some(now_utc());
        snapshot.last_error = error.map(|message| truncate(&message, 1_000));
        self.write_mcp_catalog_unlocked(&catalog)
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

    fn ensure_layout(&self) -> AppResult<()> {
        create_private_dir(&self.control_root)?;
        create_private_dir(&self.requests_dir())?;
        create_private_dir(&self.processing_dir())?;
        create_private_dir(&self.managed_cli_bin_dir())?;
        create_private_dir(&self.managed_cli_home())?;
        create_private_dir(&self.state_root.join("codex-profiles").join("homes"))?;
        Ok(())
    }

    fn lock(&self) -> AppResult<ControlLock> {
        self.ensure_layout()?;
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(self.control_root.join(".lock"))
            .map_err(file_error)?;
        set_private_file_permissions(&file)?;
        file.lock_exclusive().map_err(file_error)?;
        Ok(ControlLock(file))
    }

    fn runtime_path(&self) -> PathBuf {
        self.control_root.join("runtime.json")
    }

    fn profiles_path(&self) -> PathBuf {
        self.control_root.join("profiles.json")
    }

    fn mcp_catalog_path(&self) -> PathBuf {
        self.control_root.join("mcp-catalog.json")
    }

    fn cli_settings_path(&self) -> PathBuf {
        self.control_root.join("cli-settings.json")
    }

    fn trigger_preferences_path(&self) -> PathBuf {
        self.control_root.join("trigger-preferences.json")
    }

    fn requests_dir(&self) -> PathBuf {
        self.control_root.join("requests")
    }

    fn processing_dir(&self) -> PathBuf {
        self.control_root.join("processing")
    }

    fn queued_request_path(&self, request_id: Uuid) -> PathBuf {
        self.requests_dir().join(format!("{request_id}.json"))
    }

    fn read_runtime_unlocked(&self) -> AppResult<CodexCliRuntime> {
        read_json_or_default(&self.runtime_path())
    }

    fn write_runtime_unlocked(&self, runtime: &CodexCliRuntime) -> AppResult<()> {
        write_private_json_atomic(&self.runtime_path(), runtime)
    }

    fn read_profiles_unlocked(&self) -> AppResult<CodexAuthProfileFile> {
        read_json_or_default(&self.profiles_path())
    }

    fn write_profiles_unlocked(&self, profiles: &CodexAuthProfileFile) -> AppResult<()> {
        write_private_json_atomic(&self.profiles_path(), profiles)
    }

    fn read_mcp_catalog_unlocked(&self) -> AppResult<CodexMcpCatalogFile> {
        read_json_or_default(&self.mcp_catalog_path())
    }

    fn write_mcp_catalog_unlocked(&self, catalog: &CodexMcpCatalogFile) -> AppResult<()> {
        write_private_json_atomic(&self.mcp_catalog_path(), catalog)
    }

    fn read_cli_settings_unlocked(&self) -> AppResult<CompanyCodexCliSettingsFile> {
        read_json_or_default(&self.cli_settings_path())
    }

    fn write_cli_settings_unlocked(&self, settings: &CompanyCodexCliSettingsFile) -> AppResult<()> {
        write_private_json_atomic(&self.cli_settings_path(), settings)
    }

    fn read_trigger_preferences_unlocked(&self) -> AppResult<AgentTriggerPreferencesFile> {
        read_json_or_default(&self.trigger_preferences_path())
    }

    fn write_trigger_preferences_unlocked(
        &self,
        preferences: &AgentTriggerPreferencesFile,
    ) -> AppResult<()> {
        write_private_json_atomic(&self.trigger_preferences_path(), preferences)
    }

    fn ensure_company_selector_unlocked(&self, company_id: Uuid, selector: &str) -> AppResult<()> {
        validate_mcp_selector(selector)?;
        if selector == "default" {
            return Ok(());
        }
        let allowed = self
            .read_profiles_unlocked()?
            .profiles
            .into_iter()
            .any(|profile| {
                profile.company_id == company_id
                    && profile.selector == selector
                    && profile.status == CODEX_AUTH_PROFILE_STATUS_ACTIVE
            });
        if allowed {
            Ok(())
        } else {
            Err(AppError::NotFound(
                "Codex authentication environment not found".into(),
            ))
        }
    }

    fn ensure_no_pending_mcp_operation_unlocked(&self, selector: &str) -> AppResult<()> {
        let catalog = self.read_mcp_catalog_unlocked()?;
        let pending = catalog
            .environments
            .iter()
            .find(|snapshot| snapshot.selector == selector)
            .is_some_and(|snapshot| {
                !matches!(
                    snapshot.operation_status.as_str(),
                    CODEX_MCP_OPERATION_IDLE | CODEX_MCP_OPERATION_FAILED
                )
            });
        if pending {
            Err(AppError::Conflict(
                "another Codex MCP operation is already pending for this environment".into(),
            ))
        } else {
            Ok(())
        }
    }

    fn update_mcp_operation_unlocked(
        &self,
        selector: &str,
        operation_status: &str,
        pending_server_name: Option<String>,
        error: Option<String>,
    ) -> AppResult<()> {
        let mut catalog = self.read_mcp_catalog_unlocked()?;
        let snapshot = mcp_snapshot_mut(&mut catalog, selector);
        snapshot.operation_status = operation_status.into();
        snapshot.pending_server_name = pending_server_name;
        snapshot.last_error = error;
        self.write_mcp_catalog_unlocked(&catalog)
    }

    fn mark_mcp_request_processing(
        &self,
        request: &CodexControlRequest,
        operation_status: &str,
    ) -> AppResult<()> {
        let selector = request
            .target_selector
            .as_deref()
            .ok_or_else(|| AppError::Internal("Codex MCP request has no target selector".into()))?;
        let _lock = self.lock()?;
        self.update_mcp_operation_unlocked(
            selector,
            operation_status,
            request.mcp_server_name.clone(),
            None,
        )
    }

    fn write_request_unlocked(&self, request: &CodexControlRequest) -> AppResult<()> {
        write_private_json_atomic(&self.queued_request_path(request.id), request)
    }
}

fn mcp_snapshot_mut<'a>(
    catalog: &'a mut CodexMcpCatalogFile,
    selector: &str,
) -> &'a mut CodexMcpEnvironmentSnapshot {
    if let Some(index) = catalog
        .environments
        .iter()
        .position(|snapshot| snapshot.selector == selector)
    {
        return &mut catalog.environments[index];
    }
    catalog
        .environments
        .push(CodexMcpEnvironmentSnapshot::new(selector.to_string()));
    catalog.environments.last_mut().expect("MCP snapshot")
}

fn validate_mcp_selector(value: &str) -> AppResult<()> {
    if value == "default" || managed_profile_id(value).is_some() {
        Ok(())
    } else {
        Err(AppError::Validation(
            "Codex MCP target environment is invalid".into(),
        ))
    }
}

fn validate_mcp_server_name(value: &str) -> AppResult<()> {
    if value.trim() != value
        || value.is_empty()
        || value.len() > 80
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "_-".contains(character))
    {
        return Err(AppError::Validation(
            "Codex MCP server name must contain only letters, numbers, underscores, or hyphens"
                .into(),
        ));
    }
    Ok(())
}

fn validate_mcp_server_input(input: &CodexMcpServerInput) -> AppResult<()> {
    validate_mcp_selector(&input.target_selector)?;
    validate_mcp_server_name(&input.name)?;
    if input.args.len() > 64
        || input.args.iter().any(|argument| {
            argument.is_empty() || argument.len() > 2_048 || argument.chars().any(char::is_control)
        })
    {
        return Err(AppError::Validation(
            "Codex MCP stdio arguments are invalid".into(),
        ));
    }
    match input.transport.as_str() {
        CODEX_MCP_TRANSPORT_HTTP => {
            let url = input
                .url
                .as_deref()
                .ok_or_else(|| AppError::Validation("Streamable HTTP MCP requires a URL".into()))?;
            let parsed_url = reqwest::Url::parse(url)
                .map_err(|_| AppError::Validation("Codex MCP URL is invalid".into()))?;
            if url.len() > 2_048
                || url.chars().any(char::is_control)
                || !matches!(parsed_url.scheme(), "https" | "http")
                || !parsed_url.username().is_empty()
                || parsed_url.password().is_some()
                || parsed_url.query().is_some()
                || parsed_url.fragment().is_some()
            {
                return Err(AppError::Validation("Codex MCP URL is invalid".into()));
            }
            if input.command.is_some() || !input.args.is_empty() {
                return Err(AppError::Validation(
                    "HTTP MCP cannot include a stdio command".into(),
                ));
            }
            if let Some(name) = input.bearer_token_env_var.as_deref() {
                validate_environment_name(name)?;
            }
        }
        CODEX_MCP_TRANSPORT_STDIO => {
            let command = input
                .command
                .as_deref()
                .ok_or_else(|| AppError::Validation("stdio MCP requires a command".into()))?;
            if command.trim() != command
                || command.is_empty()
                || command.len() > 1_024
                || command.chars().any(char::is_control)
            {
                return Err(AppError::Validation(
                    "Codex MCP stdio command is invalid".into(),
                ));
            }
            if input.url.is_some() || input.bearer_token_env_var.is_some() {
                return Err(AppError::Validation(
                    "stdio MCP cannot include HTTP configuration".into(),
                ));
            }
        }
        _ => {
            return Err(AppError::Validation(
                "unsupported Codex MCP transport".into(),
            ))
        }
    }
    Ok(())
}

fn validate_environment_name(value: &str) -> AppResult<()> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
        || value.as_bytes()[0].is_ascii_digit()
    {
        return Err(AppError::Validation(
            "Codex MCP bearer token environment variable name is invalid".into(),
        ));
    }
    Ok(())
}

fn current_host_os() -> String {
    std::env::consts::OS.to_string()
}

fn current_host_arch() -> String {
    std::env::consts::ARCH.to_string()
}

fn current_installer_kind() -> String {
    match std::env::consts::OS {
        "macos" | "linux" => CODEX_INSTALLER_POSIX_SHELL,
        "windows" => CODEX_INSTALLER_POWERSHELL,
        _ => CODEX_INSTALLER_UNSUPPORTED,
    }
    .to_string()
}

fn current_installation_supported() -> bool {
    matches!(std::env::consts::OS, "macos" | "linux" | "windows")
}

struct ControlLock(File);

impl Drop for ControlLock {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}

pub fn managed_profile_selector(profile_id: Uuid) -> String {
    format!("relay_{profile_id}")
}

pub fn managed_profile_id(selector: &str) -> Option<Uuid> {
    selector
        .strip_prefix("relay_")
        .and_then(|value| Uuid::parse_str(value).ok())
}

fn validate_profile_name(value: &str) -> AppResult<()> {
    if value.trim() != value
        || value.is_empty()
        || value.chars().count() > 80
        || value.chars().any(char::is_control)
    {
        return Err(AppError::Validation(
            "Codex authentication profile name must contain 1 to 80 safe characters".into(),
        ));
    }
    Ok(())
}

fn validate_api_key(value: &str) -> AppResult<()> {
    if value.trim() != value
        || value.chars().count() < 20
        || value.chars().count() > 1_024
        || value.chars().any(char::is_whitespace)
        || value.chars().any(char::is_control)
    {
        return Err(AppError::Validation("OpenAI API key is invalid".into()));
    }
    Ok(())
}

fn validate_company_cli_settings(settings: &CompanyCodexCliSettings) -> AppResult<()> {
    if settings.model.as_ref().is_some_and(|value| {
        value.trim() != value
            || value.is_empty()
            || value.chars().count() > 128
            || value.chars().any(char::is_control)
    }) {
        return Err(AppError::Validation(
            "Codex CLI model must contain at most 128 safe characters".into(),
        ));
    }
    if settings.reasoning_effort.as_deref().is_some_and(|value| {
        !matches!(
            value,
            "none" | "minimal" | "low" | "medium" | "high" | "xhigh" | "max" | "ultra"
        )
    }) {
        return Err(AppError::Validation(
            "unsupported Codex reasoning effort".into(),
        ));
    }
    if !matches!(
        settings.reasoning_summary.as_str(),
        "auto" | "concise" | "detailed" | "none"
    ) {
        return Err(AppError::Validation(
            "unsupported Codex reasoning summary".into(),
        ));
    }
    if settings
        .verbosity
        .as_deref()
        .is_some_and(|value| !matches!(value, "low" | "medium" | "high"))
    {
        return Err(AppError::Validation(
            "unsupported Codex model verbosity".into(),
        ));
    }
    if settings
        .personality
        .as_deref()
        .is_some_and(|value| !matches!(value, "none" | "friendly" | "pragmatic"))
    {
        return Err(AppError::Validation("unsupported Codex personality".into()));
    }
    if settings
        .service_tier
        .as_deref()
        .is_some_and(|value| value != "fast")
    {
        return Err(AppError::Validation(
            "unsupported Codex service tier".into(),
        ));
    }
    if !matches!(settings.approval_policy.as_str(), "never" | "on-request") {
        return Err(AppError::Validation(
            "unsupported Codex approval policy".into(),
        ));
    }
    if !matches!(
        settings.sandbox_mode.as_str(),
        "read_only" | "workspace_write"
    ) {
        return Err(AppError::Validation(
            "unsupported Codex sandbox mode".into(),
        ));
    }
    if !matches!(
        settings.web_search.as_str(),
        "disabled" | "cached" | "indexed" | "live"
    ) {
        return Err(AppError::Validation(
            "unsupported Codex web search mode".into(),
        ));
    }
    Ok(())
}

fn validate_agent_trigger_batch_size(value: usize) -> AppResult<usize> {
    if !(AGENT_TRIGGER_BATCH_SIZE_MIN..=AGENT_TRIGGER_BATCH_SIZE_MAX).contains(&value) {
        return Err(AppError::Validation(format!(
            "Agent Trigger batch size must be between {AGENT_TRIGGER_BATCH_SIZE_MIN} and {AGENT_TRIGGER_BATCH_SIZE_MAX}"
        )));
    }
    Ok(value)
}

fn normalize_codex_base_url(value: Option<&str>) -> AppResult<String> {
    let value = value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(CODEX_DEFAULT_OPENAI_BASE_URL);
    if value.len() > 2_048 || value.chars().any(char::is_control) {
        return Err(AppError::Validation("Codex Base URL is invalid".into()));
    }
    let parsed = reqwest::Url::parse(value)
        .map_err(|_| AppError::Validation("Codex Base URL is invalid".into()))?;
    if !matches!(parsed.scheme(), "http" | "https")
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err(AppError::Validation("Codex Base URL is invalid".into()));
    }
    Ok(value.trim_end_matches('/').to_string())
}

fn versions_show_update(installed: Option<&str>, latest: Option<&str>) -> bool {
    match (
        installed.and_then(version_tuple),
        latest.and_then(version_tuple),
    ) {
        (Some(installed), Some(latest)) => latest > installed,
        _ => false,
    }
}

fn version_tuple(value: &str) -> Option<(u64, u64, u64)> {
    let value = value
        .split_whitespace()
        .last()
        .unwrap_or(value)
        .trim_start_matches('v');
    let core = value.split(['-', '+']).next()?;
    let mut parts = core.split('.');
    Some((
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
    ))
}

fn read_json_or_default<T: DeserializeOwned + Default>(path: &Path) -> AppResult<T> {
    if !path.exists() {
        return Ok(T::default());
    }
    read_json(path)
}

fn read_json<T: DeserializeOwned>(path: &Path) -> AppResult<T> {
    let mut file = File::open(path).map_err(file_error)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(file_error)?;
    serde_json::from_slice(&bytes).map_err(|error| {
        AppError::Internal(format!(
            "cannot parse Codex control file {}: {error}",
            path.display()
        ))
    })
}

fn write_private_json_atomic<T: Serialize>(path: &Path, value: &T) -> AppResult<()> {
    let parent = path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    create_private_dir(parent)?;
    let temporary = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("codex"),
        Uuid::new_v4()
    ));
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)
        .map_err(file_error)?;
    set_private_file_permissions(&file)?;
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| {
        AppError::Internal(format!("cannot serialize Codex control state: {error}"))
    })?;
    file.write_all(&bytes).map_err(file_error)?;
    file.sync_all().map_err(file_error)?;
    drop(file);
    fs::rename(&temporary, path).map_err(file_error)?;
    Ok(())
}

fn create_private_dir(path: &Path) -> AppResult<()> {
    fs::create_dir_all(path).map_err(file_error)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(file_error)?;
    }
    Ok(())
}

fn set_private_file_permissions(file: &File) -> AppResult<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(file_error)?;
    }
    Ok(())
}

fn file_error(error: std::io::Error) -> AppError {
    AppError::Internal(format!("Codex control storage error: {error}"))
}

fn truncate(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary_store() -> (PathBuf, CodexControlStore) {
        let root = std::env::temp_dir().join(format!("relay-codex-control-{}", Uuid::new_v4()));
        let store = CodexControlStore::new(root.join("codex-control")).expect("store");
        (root, store)
    }

    #[test]
    fn managed_profiles_have_independent_codex_homes() {
        let (_root, store) = temporary_store();
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        assert_ne!(
            store.managed_profile_home(first),
            store.managed_profile_home(second)
        );
        assert_eq!(
            store
                .managed_profile_home_for_selector(&managed_profile_selector(first))
                .expect("managed home"),
            store.managed_profile_home(first)
        );
    }

    #[test]
    fn agent_trigger_preferences_use_fallback_until_managed() {
        let (root, store) = temporary_store();
        let preferences = store
            .agent_trigger_preferences(24)
            .expect("fallback preferences");
        assert_eq!(preferences.batch_size, 24);
        assert_eq!(preferences.environment_default, 24);
        assert!(!preferences.managed);
        assert_eq!(preferences.updated_by_human_user_id, None);
        assert_eq!(preferences.updated_at, None);
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn agent_trigger_preferences_are_saved_and_reloaded() {
        let (root, store) = temporary_store();
        let human_id = Uuid::new_v4();
        let saved = store
            .save_agent_trigger_preferences(32, human_id, 10)
            .expect("save preferences");
        assert_eq!(saved.batch_size, 32);
        assert!(saved.managed);
        assert_eq!(saved.updated_by_human_user_id, Some(human_id));

        let reloaded = store
            .agent_trigger_preferences(10)
            .expect("reload preferences");
        assert_eq!(reloaded, saved);
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn agent_trigger_preferences_reject_out_of_range_values() {
        let (root, store) = temporary_store();
        let human_id = Uuid::new_v4();
        assert!(store
            .save_agent_trigger_preferences(0, human_id, 10)
            .is_err());
        assert!(store
            .save_agent_trigger_preferences(101, human_id, 10)
            .is_err());
        assert!(store.agent_trigger_preferences(0).is_err());
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn api_key_request_is_private_and_never_exposed_by_environment_view() {
        let (root, store) = temporary_store();
        let company_id = Uuid::new_v4();
        let profile = store
            .create_auth_profile(
                company_id,
                "Primary".into(),
                "sk-test-abcdefghijklmnopqrstuvwxyz".into(),
                None,
            )
            .expect("create profile");
        let environment = store
            .environment_for_company(company_id)
            .expect("environment");
        assert_eq!(environment.profiles, vec![profile]);
        assert_eq!(
            environment.profiles[0].base_url.as_deref(),
            Some(CODEX_DEFAULT_OPENAI_BASE_URL)
        );
        let request_path = fs::read_dir(store.requests_dir())
            .expect("requests")
            .next()
            .expect("request")
            .expect("entry")
            .path();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&request_path)
                    .expect("metadata")
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn codex_base_url_is_normalized_and_rejects_embedded_secrets() {
        assert_eq!(
            normalize_codex_base_url(Some("https://proxy.example.com/v1/")).expect("safe Base URL"),
            "https://proxy.example.com/v1"
        );
        assert_eq!(
            normalize_codex_base_url(None).expect("default Base URL"),
            CODEX_DEFAULT_OPENAI_BASE_URL
        );
        for value in [
            "https://user:secret@example.com/v1",
            "https://example.com/v1?token=secret",
            "https://example.com/v1#secret",
        ] {
            assert!(normalize_codex_base_url(Some(value)).is_err());
        }
    }

    #[test]
    fn update_is_only_available_when_latest_is_newer() {
        let (_root, store) = temporary_store();
        store
            .publish_runtime_probe(
                Some("codex-cli 0.145.0".into()),
                "system",
                Path::new("codex"),
            )
            .expect("runtime");
        let runtime = store
            .publish_latest_version(Some("0.146.0".into()), None)
            .expect("latest");
        assert!(runtime.update_available);
        let runtime = store
            .publish_latest_version(Some("0.145.0".into()), None)
            .expect("latest");
        assert!(!runtime.update_available);
    }

    #[test]
    fn failed_api_key_can_be_replaced_and_the_temporary_request_is_removed() {
        let (root, store) = temporary_store();
        let company_id = Uuid::new_v4();
        let profile = store
            .create_auth_profile(
                company_id,
                "Primary".into(),
                "sk-test-first-abcdefghijklmnopqrstuvwxyz".into(),
                None,
            )
            .expect("create profile");
        let claimed = store.claim_next_request().expect("claim").expect("request");
        store
            .fail_request(claimed, "authentication failed")
            .expect("failure");
        let failed = store
            .environment_for_company(company_id)
            .expect("environment")
            .profiles
            .pop()
            .expect("profile");
        assert_eq!(failed.status, CODEX_AUTH_PROFILE_STATUS_FAILED);
        assert_eq!(
            fs::read_dir(store.processing_dir())
                .expect("processing")
                .count(),
            0
        );

        store
            .update_auth_profile(
                company_id,
                profile.id,
                "Primary".into(),
                Some("sk-test-second-abcdefghijklmnopqrstuvwxyz".into()),
                Some("https://proxy.example.com/v1".into()),
            )
            .expect("retry profile");
        let claimed = store
            .claim_next_request()
            .expect("claim retry")
            .expect("retry request");
        store
            .mark_auth_profile_active(profile.id)
            .expect("activate");
        store.finish_request(claimed).expect("finish");
        let active = store
            .environment_for_company(company_id)
            .expect("environment")
            .profiles
            .pop()
            .expect("profile");
        assert_eq!(active.status, CODEX_AUTH_PROFILE_STATUS_ACTIVE);
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn company_cannot_resolve_another_company_profile() {
        let (root, store) = temporary_store();
        let owner_company = Uuid::new_v4();
        let other_company = Uuid::new_v4();
        let profile = store
            .create_auth_profile(
                owner_company,
                "Private".into(),
                "sk-test-company-abcdefghijklmnopqrstuvwxyz".into(),
                None,
            )
            .expect("create profile");
        store
            .mark_auth_profile_active(profile.id)
            .expect("activate");
        assert!(store
            .find_active_company_profile(other_company, &profile.selector)
            .expect("lookup")
            .is_none());
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn mcp_input_accepts_safe_http_and_stdio_configurations() {
        validate_mcp_server_input(&CodexMcpServerInput {
            target_selector: "default".into(),
            name: "docs".into(),
            transport: CODEX_MCP_TRANSPORT_HTTP.into(),
            url: Some("https://example.com/mcp".into()),
            command: None,
            args: Vec::new(),
            bearer_token_env_var: Some("EXAMPLE_MCP_TOKEN".into()),
        })
        .expect("safe HTTP MCP");
        validate_mcp_server_input(&CodexMcpServerInput {
            target_selector: "default".into(),
            name: "local_tools".into(),
            transport: CODEX_MCP_TRANSPORT_STDIO.into(),
            url: None,
            command: Some("npx".into()),
            args: vec!["-y".into(), "@example/mcp".into()],
            bearer_token_env_var: None,
        })
        .expect("safe stdio MCP");
    }

    #[test]
    fn mcp_http_input_rejects_secrets_embedded_in_urls() {
        for url in [
            "https://user:secret@example.com/mcp",
            "https://example.com/mcp?token=secret",
            "https://example.com/mcp#secret",
        ] {
            let result = validate_mcp_server_input(&CodexMcpServerInput {
                target_selector: "default".into(),
                name: "unsafe".into(),
                transport: CODEX_MCP_TRANSPORT_HTTP.into(),
                url: Some(url.into()),
                command: None,
                args: Vec::new(),
                bearer_token_env_var: None,
            });
            assert!(result.is_err(), "URL must be rejected: {url}");
        }
    }

    #[test]
    fn plugin_provided_mcp_server_cannot_be_removed() {
        let (root, store) = temporary_store();
        let company_id = Uuid::new_v4();
        store
            .publish_mcp_snapshot(
                "default",
                vec![CodexMcpServerView {
                    name: "plugin-server".into(),
                    transport: CODEX_MCP_TRANSPORT_STDIO.into(),
                    enabled: true,
                    configured_by_user: false,
                    ..CodexMcpServerView::default()
                }],
                None,
            )
            .expect("publish snapshot");

        let result = store.enqueue_mcp_remove(company_id, "default".into(), "plugin-server".into());
        assert!(matches!(result, Err(AppError::Conflict(_))));
        fs::remove_dir_all(root).expect("cleanup");
    }
}
