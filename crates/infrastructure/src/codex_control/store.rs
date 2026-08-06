use std::{
    fs,
    path::{Path, PathBuf},
};

use ai_chat_shared::{now_utc, AppError, AppResult};
use uuid::Uuid;

use super::validation::{
    normalize_codex_base_url, validate_agent_trigger_batch_size, validate_api_key,
    validate_company_cli_settings, validate_profile_name,
};
use super::*;

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
}
