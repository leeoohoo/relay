use super::*;

pub trait CodexControlPlatformRepository: Send + Sync {
    fn save_company_codex_runner_profile(
        &self,
        _profile: CompanyCodexRunnerProfile,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "company Codex runner profiles are not supported by this repository".into(),
        ))
    }
    fn get_company_codex_runner_profile(
        &self,
        _profile_id: Uuid,
    ) -> Option<CompanyCodexRunnerProfile> {
        None
    }
    fn list_company_codex_runner_profiles(
        &self,
        _company_id: Uuid,
    ) -> Vec<CompanyCodexRunnerProfile> {
        Vec::new()
    }
    fn delete_company_codex_runner_profile(&self, _profile_id: Uuid) -> AppResult<()> {
        Err(AppError::Validation(
            "company Codex runner profiles are not supported by this repository".into(),
        ))
    }
    fn clear_company_codex_runner_profile_defaults(
        &self,
        _company_id: Uuid,
        _except_profile_id: Uuid,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "company Codex runner profiles are not supported by this repository".into(),
        ))
    }
    fn assign_agent_codex_runner_profile(
        &self,
        _agent_id: Uuid,
        _profile_id: Uuid,
        _human_user_id: Uuid,
        _assigned_at: DateTime<Utc>,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "Agent Codex runner profile assignments are not supported by this repository".into(),
        ))
    }
    fn get_agent_codex_runner_profile_assignment(&self, _agent_id: Uuid) -> Option<Uuid> {
        None
    }
    fn list_codex_runner_profile_agent_ids(&self, _profile_id: Uuid) -> Vec<Uuid> {
        Vec::new()
    }
    fn save_codex_plugin_catalog_snapshot(
        &self,
        _snapshot: CodexPluginCatalogSnapshot,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "Codex plugin catalogs are not supported by this repository".into(),
        ))
    }
    fn list_codex_plugin_catalog_snapshots(&self) -> AppResult<Vec<CodexPluginCatalogSnapshot>> {
        Ok(Vec::new())
    }
    fn insert_codex_plugin_operation(&self, _operation: CodexPluginOperation) -> AppResult<()> {
        Err(AppError::Validation(
            "Codex plugin operations are not supported by this repository".into(),
        ))
    }
    fn list_company_codex_plugin_operations(
        &self,
        _company_id: Uuid,
        _limit: usize,
    ) -> AppResult<Vec<CodexPluginOperation>> {
        Ok(Vec::new())
    }
    fn claim_codex_plugin_operations(
        &self,
        _target_runner_id: &str,
        _lease_owner: &str,
        _now: DateTime<Utc>,
        _limit: usize,
    ) -> AppResult<Vec<CodexPluginOperation>> {
        Ok(Vec::new())
    }
    fn finish_codex_plugin_operation(
        &self,
        _operation_id: Uuid,
        _lease_owner: &str,
        _succeeded: bool,
        _result: Value,
        _error_message: Option<String>,
        _finished_at: DateTime<Utc>,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "Codex plugin operations are not supported by this repository".into(),
        ))
    }
}

pub trait CodexRuntimePlatformRepository: Send + Sync {
    fn save_agent_codex_trigger_config(&self, _config: AgentCodexTriggerConfig) -> AppResult<()> {
        Err(AppError::Validation(
            "Codex trigger configuration is not supported by this repository".into(),
        ))
    }
    fn get_agent_codex_trigger_config_by_agent(
        &self,
        _agent_id: Uuid,
    ) -> Option<AgentCodexTriggerConfig> {
        None
    }
    fn get_agent_codex_trigger_config_by_agent_result(
        &self,
        agent_id: Uuid,
    ) -> AppResult<Option<AgentCodexTriggerConfig>> {
        Ok(self.get_agent_codex_trigger_config_by_agent(agent_id))
    }
    fn claim_due_agent_codex_trigger_configs(
        &self,
        _lease_owner: &str,
        _now: DateTime<Utc>,
        _limit: usize,
    ) -> AppResult<Vec<AgentCodexTriggerConfig>> {
        Ok(Vec::new())
    }
    fn abandon_agent_codex_trigger_leases(
        &self,
        _lease_owner: &str,
        _now: DateTime<Utc>,
    ) -> AppResult<usize> {
        Err(AppError::Validation(
            "Codex trigger leases are not supported by this repository".into(),
        ))
    }
    fn request_agent_codex_trigger_wake(
        &self,
        _agent_id: Uuid,
        _requested_at: DateTime<Utc>,
        _reason: &str,
    ) -> AppResult<bool> {
        Ok(false)
    }
    fn complete_agent_codex_trigger_lease(
        &self,
        _input: CompleteAgentCodexTriggerLeaseInput,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "Codex trigger leases are not supported by this repository".into(),
        ))
    }
    fn insert_agent_codex_trigger_run(&self, _run: AgentCodexTriggerRun) -> AppResult<()> {
        Err(AppError::Validation(
            "Codex trigger runs are not supported by this repository".into(),
        ))
    }
    fn has_running_agent_codex_trigger_run(&self, _agent_id: Uuid) -> bool {
        false
    }
    fn update_agent_codex_trigger_run(&self, _run: AgentCodexTriggerRun) -> AppResult<()> {
        Err(AppError::Validation(
            "Codex trigger runs are not supported by this repository".into(),
        ))
    }
    fn append_agent_codex_trigger_run_activity(
        &self,
        _run_id: Uuid,
        _activity: AgentCodexRunActivity,
        _codex_thread_id: Option<String>,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "Codex trigger run activity is not supported by this repository".into(),
        ))
    }
    fn list_agent_codex_trigger_runs(
        &self,
        _agent_id: Uuid,
        _limit: usize,
    ) -> Vec<AgentCodexTriggerRun> {
        Vec::new()
    }
    fn list_agent_codex_trigger_runs_result(
        &self,
        agent_id: Uuid,
        limit: usize,
    ) -> AppResult<Vec<AgentCodexTriggerRun>> {
        Ok(self.list_agent_codex_trigger_runs(agent_id, limit))
    }
    fn save_agent_codex_session(&self, _session: AgentCodexSession) -> AppResult<()> {
        Err(AppError::Validation(
            "Codex sessions are not supported by this repository".into(),
        ))
    }
    fn get_agent_codex_session(
        &self,
        _agent_id: Uuid,
        _scope_key: &str,
    ) -> Option<AgentCodexSession> {
        None
    }
    fn list_agent_codex_sessions(&self, _agent_id: Uuid, _limit: usize) -> Vec<AgentCodexSession> {
        Vec::new()
    }
    fn insert_agent_execution_intent(&self, _intent: AgentExecutionIntent) -> AppResult<()> {
        Err(AppError::Validation(
            "Agent execution intents are not supported by this repository".into(),
        ))
    }
    fn update_agent_execution_intent(&self, _intent: AgentExecutionIntent) -> AppResult<()> {
        Err(AppError::Validation(
            "Agent execution intents are not supported by this repository".into(),
        ))
    }
    fn get_agent_execution_intent(&self, _intent_id: Uuid) -> Option<AgentExecutionIntent> {
        None
    }
    fn list_agent_execution_intents(
        &self,
        _agent_id: Uuid,
        _status: Option<&str>,
        _limit: usize,
    ) -> Vec<AgentExecutionIntent> {
        Vec::new()
    }
    fn insert_agent_codex_run_token(&self, _token: AgentCodexRunToken) -> AppResult<()> {
        Err(AppError::Validation(
            "Codex run tokens are not supported by this repository".into(),
        ))
    }
    fn find_agent_codex_run_token_by_hash(&self, _token_hash: &str) -> Option<AgentCodexRunToken> {
        None
    }
    fn revoke_agent_codex_run_tokens(
        &self,
        _run_id: Uuid,
        _revoked_at: DateTime<Utc>,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "Codex run tokens are not supported by this repository".into(),
        ))
    }
    fn delete_expired_agent_codex_run_tokens(&self, _now: DateTime<Utc>) -> AppResult<usize> {
        Ok(0)
    }
}

pub trait CodexPlatformRepository:
    CodexControlPlatformRepository + CodexRuntimePlatformRepository
{
}

impl<T> CodexPlatformRepository for T where
    T: CodexControlPlatformRepository + CodexRuntimePlatformRepository
{
}
