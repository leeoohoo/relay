pub use crate::contracts::*;
use ai_chat_domain::{agent_identity::*, company::*, social::*};
use ai_chat_shared::{AppError, AppResult};
use chrono::{DateTime, Utc};
use serde_json::Value;
use uuid::Uuid;
pub trait PlatformRepository: Clone + Send + Sync + 'static {
    fn health_check(&self) -> AppResult<()> {
        Ok(())
    }
    fn find_human_user_by_email(&self, email: &str) -> Option<HumanUser>;
    fn find_human_user_by_email_result(&self, email: &str) -> AppResult<Option<HumanUser>> {
        Ok(self.find_human_user_by_email(email))
    }
    fn get_human_user(&self, user_id: Uuid) -> Option<HumanUser>;
    fn get_human_user_result(&self, user_id: Uuid) -> AppResult<Option<HumanUser>> {
        Ok(self.get_human_user(user_id))
    }
    fn list_human_users(&self) -> Vec<HumanUser>;
    fn insert_human_user(&self, user: HumanUser) -> AppResult<HumanUser>;
    fn human_user_exists(&self, user_id: Uuid) -> bool;
    fn human_user_exists_result(&self, user_id: Uuid) -> AppResult<bool> {
        Ok(self.human_user_exists(user_id))
    }
    fn get_human_credential(&self, _user_id: Uuid) -> Option<HumanCredential> {
        None
    }
    fn get_human_credential_result(&self, user_id: Uuid) -> AppResult<Option<HumanCredential>> {
        Ok(self.get_human_credential(user_id))
    }
    fn insert_human_auth_bundle(
        &self,
        _user: HumanUser,
        _credential: HumanCredential,
    ) -> AppResult<HumanUser> {
        Err(AppError::Validation(
            "human authentication is not supported by this repository".into(),
        ))
    }
    fn get_human_harness_account(
        &self,
        _human_user_id: Uuid,
    ) -> Option<ai_chat_domain::agent_identity::HumanHarnessAccount> {
        None
    }
    fn get_human_harness_account_result(
        &self,
        human_user_id: Uuid,
    ) -> AppResult<Option<ai_chat_domain::agent_identity::HumanHarnessAccount>> {
        Ok(self.get_human_harness_account(human_user_id))
    }
    fn upsert_human_harness_account(
        &self,
        _account: ai_chat_domain::agent_identity::HumanHarnessAccount,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "human Harness accounts are not supported by this repository".into(),
        ))
    }
    fn insert_human_session(&self, _session: HumanSession) -> AppResult<()> {
        Err(AppError::Validation(
            "human sessions are not supported by this repository".into(),
        ))
    }
    fn find_human_session_by_token_hash(&self, _token_hash: &str) -> Option<HumanSession> {
        None
    }
    fn find_human_session_by_token_hash_result(
        &self,
        token_hash: &str,
    ) -> AppResult<Option<HumanSession>> {
        Ok(self.find_human_session_by_token_hash(token_hash))
    }
    fn touch_human_session(
        &self,
        _session_id: Uuid,
        _used_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "human sessions are not supported by this repository".into(),
        ))
    }
    fn revoke_human_session(
        &self,
        _session_id: Uuid,
        _revoked_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "human sessions are not supported by this repository".into(),
        ))
    }
    fn get_human_session(&self, _session_id: Uuid) -> Option<HumanSession> {
        None
    }
    fn get_human_session_result(&self, session_id: Uuid) -> AppResult<Option<HumanSession>> {
        Ok(self.get_human_session(session_id))
    }
    fn list_human_sessions(&self, _human_user_id: Uuid) -> Vec<HumanSession> {
        Vec::new()
    }
    fn list_human_sessions_result(&self, human_user_id: Uuid) -> AppResult<Vec<HumanSession>> {
        Ok(self.list_human_sessions(human_user_id))
    }
    fn revoke_human_sessions(
        &self,
        _human_user_id: Uuid,
        _except_session_id: Option<Uuid>,
        _revoked_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<usize> {
        Ok(0)
    }
    fn update_human_password_hash(
        &self,
        _human_user_id: Uuid,
        _password_hash: String,
        _updated_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "password updates are not supported by this repository".into(),
        ))
    }
    fn delete_expired_human_sessions(
        &self,
        _now: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<usize> {
        Ok(0)
    }
    fn insert_human_account_token(&self, _token: HumanAccountToken) -> AppResult<()> {
        Err(AppError::Validation(
            "human account tokens are not supported by this repository".into(),
        ))
    }
    fn find_human_account_token_by_hash(&self, _token_hash: &str) -> Option<HumanAccountToken> {
        None
    }
    fn find_human_account_token_by_hash_result(
        &self,
        token_hash: &str,
    ) -> AppResult<Option<HumanAccountToken>> {
        Ok(self.find_human_account_token_by_hash(token_hash))
    }
    fn mark_human_account_token_used(
        &self,
        _token_id: Uuid,
        _used_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "human account tokens are not supported by this repository".into(),
        ))
    }
    fn mark_human_email_verified(
        &self,
        _human_user_id: Uuid,
        _verified_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "human email verification is not supported by this repository".into(),
        ))
    }
    fn verify_human_email_atomic(
        &self,
        token_id: Uuid,
        human_user_id: Uuid,
        verified_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        self.mark_human_email_verified(human_user_id, verified_at)?;
        self.mark_human_account_token_used(token_id, verified_at)
    }
    fn reset_human_password_atomic(
        &self,
        token_id: Uuid,
        human_user_id: Uuid,
        password_hash: String,
        updated_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        self.update_human_password_hash(human_user_id, password_hash, updated_at)?;
        self.revoke_human_sessions(human_user_id, None, updated_at)?;
        self.mark_human_account_token_used(token_id, updated_at)
    }
    fn is_human_email_verified(&self, _human_user_id: Uuid) -> bool {
        false
    }
    fn is_human_email_verified_result(&self, human_user_id: Uuid) -> AppResult<bool> {
        Ok(self.is_human_email_verified(human_user_id))
    }
    fn delete_expired_human_account_tokens(
        &self,
        _now: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<usize> {
        Ok(0)
    }
    fn insert_company_bundle(&self, _bundle: CompanyCreationBundle) -> AppResult<()> {
        Err(AppError::Validation(
            "companies are not supported by this repository".into(),
        ))
    }
    fn get_company(&self, _company_id: Uuid) -> Option<Company> {
        None
    }
    fn get_company_result(&self, company_id: Uuid) -> AppResult<Option<Company>> {
        Ok(self.get_company(company_id))
    }
    fn get_company_by_slug(&self, _slug: &str) -> Option<Company> {
        None
    }
    fn list_human_companies(&self, _human_user_id: Uuid) -> Vec<Company> {
        Vec::new()
    }
    fn get_company_human_member(
        &self,
        _company_id: Uuid,
        _human_user_id: Uuid,
    ) -> Option<CompanyHumanMember> {
        None
    }
    fn get_company_human_member_result(
        &self,
        company_id: Uuid,
        human_user_id: Uuid,
    ) -> AppResult<Option<CompanyHumanMember>> {
        Ok(self.get_company_human_member(company_id, human_user_id))
    }
    fn get_org_unit(&self, _org_unit_id: Uuid) -> Option<OrgUnit> {
        None
    }
    fn insert_org_unit(&self, _org_unit: OrgUnit) -> AppResult<()> {
        Err(AppError::Validation(
            "organization units are not supported by this repository".into(),
        ))
    }
    fn list_company_org_units(&self, _company_id: Uuid) -> Vec<OrgUnit> {
        Vec::new()
    }
    fn get_company_agent_membership(&self, _agent_id: Uuid) -> Option<CompanyAgentMembership> {
        None
    }
    fn list_company_agent_memberships(&self, _company_id: Uuid) -> Vec<CompanyAgentMembership> {
        Vec::new()
    }
    fn update_company_agent_work_profile(
        &self,
        _agent_id: Uuid,
        _responsibilities: Vec<String>,
        _skills: Vec<String>,
        _current_focus: String,
        _collaboration_preference: String,
        _updated_at: DateTime<Utc>,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "company agent work profiles are not supported by this repository".into(),
        ))
    }
    fn complete_company_agent_creation_bundle(
        &self,
        _bundle: CompanyAgentCreationBundle,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "company agent creation is not supported by this repository".into(),
        ))
    }
    fn complete_company_agent_membership_update(
        &self,
        _bundle: CompanyAgentMembershipUpdateBundle,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "company agent membership updates are not supported by this repository".into(),
        ))
    }
    fn complete_agent_staffing_hire(&self, _bundle: AgentStaffingHireBundle) -> AppResult<()> {
        Err(AppError::Validation(
            "agent staffing hires are not supported by this repository".into(),
        ))
    }
    fn complete_company_agent_activation(
        &self,
        _bundle: CompanyAgentActivationBundle,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "company agent activation is not supported by this repository".into(),
        ))
    }
    fn complete_agent_staffing_status_change(
        &self,
        _bundle: AgentStaffingStatusChangeBundle,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "agent staffing status changes are not supported by this repository".into(),
        ))
    }
    fn get_agent_staffing_action(&self, _action_id: Uuid) -> Option<AgentStaffingAction> {
        None
    }
    fn list_agent_staffing_actions(&self, _company_id: Uuid) -> Vec<AgentStaffingAction> {
        Vec::new()
    }
    fn insert_registration_request(&self, request: AgentRegistrationRequest) -> AppResult<()>;
    fn insert_ownership_challenge(&self, challenge: OwnershipProofChallenge) -> AppResult<()>;
    fn get_challenge(&self, challenge_id: Uuid) -> Option<OwnershipProofChallenge>;
    fn list_challenges(&self) -> Vec<OwnershipProofChallenge>;
    fn update_challenge(&self, challenge: OwnershipProofChallenge) -> AppResult<()>;
    fn get_registration_request(&self, request_id: Uuid) -> Option<AgentRegistrationRequest>;
    fn list_registration_requests(&self) -> Vec<AgentRegistrationRequest>;
    fn update_registration_request(&self, request: AgentRegistrationRequest) -> AppResult<()>;
    fn insert_agent_profile(&self, profile: AgentProfile) -> AppResult<()>;
    fn complete_registration_bundle(&self, bundle: RegistrationCompletionBundle) -> AppResult<()>;
    fn list_agent_profiles(&self) -> Vec<AgentProfile>;
    fn update_agent_profile_status(&self, agent_id: Uuid, status: AgentStatus) -> AppResult<()>;
    fn update_agent_profile(
        &self,
        agent_id: Uuid,
        display_name: String,
        persona: String,
        collaboration_preference: String,
    ) -> AppResult<()>;
    fn insert_owner_binding(&self, binding: AgentOwnerBinding) -> AppResult<()>;
    fn insert_agent_key(&self, key_record: AgentKeyRecord) -> AppResult<()>;
    fn find_agent_key_by_plaintext(&self, plaintext_key: &str) -> Option<AgentKeyRecord>;
    fn list_agent_keys(&self, agent_id: Uuid) -> Vec<AgentKeyRecord>;
    fn touch_agent_key_usage(
        &self,
        key_id: Uuid,
        used_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()>;
    fn revoke_agent_keys(
        &self,
        agent_id: Uuid,
        revoked_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()>;
    fn insert_agent_key_issue_log(&self, log: AgentKeyIssueLog) -> AppResult<()>;
    fn list_agent_key_issue_logs(&self, agent_id: Uuid, limit: usize) -> Vec<AgentKeyIssueLog>;
    fn insert_social_proof_submission(&self, submission: SocialProofSubmission) -> AppResult<()>;
    fn list_social_proof_submissions(&self) -> Vec<SocialProofSubmission>;
    fn insert_agent_inbox_event(&self, event: AgentInboxEvent) -> AppResult<()>;
    fn list_agent_inbox_events(
        &self,
        agent_id: Uuid,
        status: Option<AgentInboxEventStatus>,
        limit: usize,
    ) -> Vec<AgentInboxEvent>;
    fn get_agent_inbox_event(&self, event_id: Uuid) -> Option<AgentInboxEvent>;
    fn update_agent_inbox_event(&self, event: AgentInboxEvent) -> AppResult<()>;
    fn insert_agent_action_log(&self, log: AgentActionLog) -> AppResult<()>;
    fn list_all_agent_action_logs(&self) -> Vec<AgentActionLog>;
    fn count_agent_actions_since(
        &self,
        _agent_id: Uuid,
        _since: chrono::DateTime<chrono::Utc>,
    ) -> usize {
        0
    }
    fn get_agent_idempotency_record(
        &self,
        _agent_id: Uuid,
        _operation: &str,
        _idempotency_key: &str,
    ) -> Option<AgentIdempotencyRecord> {
        None
    }
    fn insert_agent_idempotency_record(&self, _record: AgentIdempotencyRecord) -> AppResult<()> {
        Ok(())
    }
    fn delete_expired_agent_idempotency_records(
        &self,
        _now: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<usize> {
        Ok(0)
    }
    fn ensure_agent_conversation_bucket(&self, agent_id: Uuid) -> AppResult<()>;
    fn insert_conversation_preview(
        &self,
        agent_id: Uuid,
        preview: ConversationPreview,
    ) -> AppResult<()>;
    fn complete_company_conversation_creation(
        &self,
        _bundle: CompanyConversationCreationBundle,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "company conversations are not supported by this repository".into(),
        ))
    }
    fn complete_human_company_direct_conversation_creation(
        &self,
        _bundle: HumanCompanyDirectConversationCreationBundle,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "human company conversations are not supported by this repository".into(),
        ))
    }
    fn find_company_direct_conversation(
        &self,
        _company_id: Uuid,
        _left_agent_id: Uuid,
        _right_agent_id: Uuid,
    ) -> Option<ConversationPreview> {
        None
    }
    fn find_human_company_direct_conversation(
        &self,
        _company_id: Uuid,
        _human_user_id: Uuid,
        _target_agent_id: Uuid,
    ) -> Option<ConversationPreview> {
        None
    }
    fn get_conversation_context(&self, _conversation_id: Uuid) -> Option<ConversationContext> {
        None
    }
    fn get_conversation_context_result(
        &self,
        conversation_id: Uuid,
    ) -> AppResult<Option<ConversationContext>> {
        Ok(self.get_conversation_context(conversation_id))
    }
    fn list_conversation_member_ids(&self, _conversation_id: Uuid) -> Vec<Uuid> {
        Vec::new()
    }
    fn complete_company_project_creation(
        &self,
        _bundle: CompanyProjectCreationBundle,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "company projects are not supported by this repository".into(),
        ))
    }
    fn get_company_project(&self, _project_id: Uuid) -> Option<CompanyProject> {
        None
    }
    fn get_company_project_result(&self, project_id: Uuid) -> AppResult<Option<CompanyProject>> {
        Ok(self.get_company_project(project_id))
    }
    fn list_company_projects(&self, _company_id: Uuid) -> Vec<CompanyProject> {
        Vec::new()
    }
    fn list_company_projects_result(&self, company_id: Uuid) -> AppResult<Vec<CompanyProject>> {
        Ok(self.list_company_projects(company_id))
    }
    fn save_company_project_git_config(&self, _config: CompanyProjectGitConfig) -> AppResult<()> {
        Err(AppError::Validation(
            "company project Git configuration is not supported by this repository".into(),
        ))
    }
    fn get_company_project_git_config(&self, _project_id: Uuid) -> Option<CompanyProjectGitConfig> {
        None
    }
    fn delete_company_project_git_config(&self, _project_id: Uuid) -> AppResult<()> {
        Err(AppError::Validation(
            "company project Git configuration is not supported by this repository".into(),
        ))
    }
    fn save_company_project_rule(&self, _rule: CompanyProjectRule) -> AppResult<()> {
        Err(AppError::Validation(
            "company project rules are not supported by this repository".into(),
        ))
    }
    fn get_company_project_rule(&self, _project_id: Uuid) -> Option<CompanyProjectRule> {
        None
    }
    fn replace_company_project_assets(
        &self,
        _project_id: Uuid,
        _assets: Vec<CompanyProjectAsset>,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "company project assets are not supported by this repository".into(),
        ))
    }
    fn list_company_project_assets(&self, _project_id: Uuid) -> Vec<CompanyProjectAsset> {
        Vec::new()
    }
    fn insert_agent_memory(&self, _memory: AgentMemory) -> AppResult<()> {
        Err(AppError::Validation(
            "Agent memories are not supported by this repository".into(),
        ))
    }
    fn update_agent_memory(&self, _memory: AgentMemory) -> AppResult<()> {
        Err(AppError::Validation(
            "Agent memories are not supported by this repository".into(),
        ))
    }
    fn get_agent_memory(&self, _memory_id: Uuid) -> Option<AgentMemory> {
        None
    }
    fn get_agent_memory_result(&self, memory_id: Uuid) -> AppResult<Option<AgentMemory>> {
        Ok(self.get_agent_memory(memory_id))
    }
    fn list_company_agent_memories(&self, _company_id: Uuid) -> Vec<AgentMemory> {
        Vec::new()
    }
    fn list_company_agent_memories_result(&self, company_id: Uuid) -> AppResult<Vec<AgentMemory>> {
        Ok(self.list_company_agent_memories(company_id))
    }
    fn delete_agent_memory(&self, _memory_id: Uuid) -> AppResult<()> {
        Err(AppError::Validation(
            "Agent memories are not supported by this repository".into(),
        ))
    }
    fn save_company_project_asset_refresh_config(
        &self,
        _config: CompanyProjectAssetRefreshConfig,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "company project asset refresh is not supported by this repository".into(),
        ))
    }
    fn get_company_project_asset_refresh_config(
        &self,
        _project_id: Uuid,
    ) -> Option<CompanyProjectAssetRefreshConfig> {
        None
    }
    fn claim_due_company_project_asset_refresh(
        &self,
        _agent_id: Uuid,
        _now: DateTime<Utc>,
    ) -> AppResult<Option<CompanyProjectAssetRefreshConfig>> {
        Ok(None)
    }
    fn mark_company_project_asset_refresh_completed(
        &self,
        _project_id: Uuid,
        _completed_at: DateTime<Utc>,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "company project asset refresh is not supported by this repository".into(),
        ))
    }
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
    fn get_agent_codex_session(&self, _agent_id: Uuid) -> Option<AgentCodexSession> {
        None
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
    fn update_company_project(&self, _project: CompanyProject) -> AppResult<()> {
        Err(AppError::Validation(
            "company project updates are not supported by this repository".into(),
        ))
    }
    fn update_company_project_metadata(
        &self,
        _project: CompanyProject,
        _project_group_title: String,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "company project metadata updates are not supported by this repository".into(),
        ))
    }
    fn get_company_project_member(
        &self,
        _project_id: Uuid,
        _agent_id: Uuid,
    ) -> Option<CompanyProjectMember> {
        None
    }
    fn list_company_project_members(&self, _project_id: Uuid) -> Vec<CompanyProjectMember> {
        Vec::new()
    }
    fn complete_company_project_member_add(
        &self,
        _bundle: CompanyProjectMemberAddBundle,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "company project membership is not supported by this repository".into(),
        ))
    }
    fn complete_company_project_member_remove(
        &self,
        _project_id: Uuid,
        _agent_id: Uuid,
        _conversation_id: Uuid,
        _left_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "company project membership is not supported by this repository".into(),
        ))
    }
    fn insert_company_project_task(&self, _task: CompanyProjectTask) -> AppResult<()> {
        Err(AppError::Validation(
            "company project tasks are not supported by this repository".into(),
        ))
    }
    fn get_company_project_task(&self, _task_id: Uuid) -> Option<CompanyProjectTask> {
        None
    }
    fn list_company_project_tasks(&self, _project_id: Uuid) -> Vec<CompanyProjectTask> {
        Vec::new()
    }
    fn list_company_project_tasks_result(
        &self,
        project_id: Uuid,
    ) -> AppResult<Vec<CompanyProjectTask>> {
        Ok(self.list_company_project_tasks(project_id))
    }
    fn update_company_project_task(&self, _task: CompanyProjectTask) -> AppResult<()> {
        Err(AppError::Validation(
            "company project tasks are not supported by this repository".into(),
        ))
    }
    fn update_company_project_tasks(&self, _tasks: Vec<CompanyProjectTask>) -> AppResult<()> {
        Err(AppError::Validation(
            "company project task batch updates are not supported by this repository".into(),
        ))
    }
    fn insert_company_project_task_dependency(
        &self,
        _dependency: CompanyProjectTaskDependency,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "company project task dependencies are not supported by this repository".into(),
        ))
    }
    fn remove_company_project_task_dependency(
        &self,
        _project_id: Uuid,
        _task_id: Uuid,
        _depends_on_task_id: Uuid,
        _removed_by_agent_id: Option<Uuid>,
        _removed_by_human_user_id: Option<Uuid>,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "company project task dependencies are not supported by this repository".into(),
        ))
    }
    fn list_company_project_task_dependencies(
        &self,
        _project_id: Uuid,
    ) -> Vec<CompanyProjectTaskDependency> {
        Vec::new()
    }
    fn list_company_project_task_status_history(
        &self,
        _project_id: Uuid,
    ) -> Vec<CompanyProjectTaskStatusHistory> {
        Vec::new()
    }
    fn insert_company_project_status_update(
        &self,
        _update: CompanyProjectStatusUpdate,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "company project status updates are not supported by this repository".into(),
        ))
    }
    fn list_company_project_status_updates(
        &self,
        _project_id: Uuid,
    ) -> Vec<CompanyProjectStatusUpdate> {
        Vec::new()
    }
    fn list_company_realtime_events(
        &self,
        _company_id: Uuid,
        _after_sequence_id: i64,
        _limit: usize,
    ) -> Vec<CompanyRealtimeEvent> {
        Vec::new()
    }
    fn list_company_realtime_events_result(
        &self,
        company_id: Uuid,
        after_sequence_id: i64,
        limit: usize,
    ) -> AppResult<Vec<CompanyRealtimeEvent>> {
        Ok(self.list_company_realtime_events(company_id, after_sequence_id, limit))
    }
    fn latest_company_realtime_sequence(&self, _company_id: Uuid) -> i64 {
        0
    }
    fn latest_company_realtime_sequence_result(&self, company_id: Uuid) -> AppResult<i64> {
        Ok(self.latest_company_realtime_sequence(company_id))
    }
    fn publish_company_governance_policy_version(
        &self,
        _version: CompanyGovernancePolicyVersion,
    ) -> AppResult<CompanyGovernancePolicyVersion> {
        Err(AppError::Validation(
            "company governance policies are not supported by this repository".into(),
        ))
    }
    fn get_active_company_governance_policy_version(
        &self,
        _company_id: Uuid,
    ) -> Option<CompanyGovernancePolicyVersion> {
        None
    }
    fn list_company_governance_policy_versions(
        &self,
        _company_id: Uuid,
    ) -> Vec<CompanyGovernancePolicyVersion> {
        Vec::new()
    }
    fn insert_agent_tool_approval_request(
        &self,
        _request: AgentToolApprovalRequest,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "agent tool approvals are not supported by this repository".into(),
        ))
    }
    fn get_agent_tool_approval_request(
        &self,
        _approval_request_id: Uuid,
    ) -> Option<AgentToolApprovalRequest> {
        None
    }
    fn list_company_agent_tool_approval_requests(
        &self,
        _company_id: Uuid,
        _status: Option<&str>,
        _limit: usize,
    ) -> Vec<AgentToolApprovalRequest> {
        Vec::new()
    }
    fn claim_agent_tool_approval_request(
        &self,
        _approval_request_id: Uuid,
        _human_user_id: Uuid,
        _status: &str,
        _review_note: &str,
        _now: DateTime<Utc>,
    ) -> AppResult<Option<AgentToolApprovalRequest>> {
        Ok(None)
    }
    fn update_agent_tool_approval_request(
        &self,
        _request: AgentToolApprovalRequest,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "agent tool approvals are not supported by this repository".into(),
        ))
    }
    fn count_agent_tool_approval_requests_since(
        &self,
        _agent_id: Uuid,
        _since: DateTime<Utc>,
    ) -> usize {
        0
    }
    fn list_owner_agents(&self, human_user_id: Uuid) -> Vec<AgentProfile>;
    fn agent_exists(&self, agent_id: Uuid) -> bool;
    fn get_agent_profile(&self, agent_id: Uuid) -> Option<AgentProfile>;
    fn list_agent_conversations(&self, agent_id: Uuid) -> Vec<ConversationPreview>;
    fn list_company_conversations(&self, _company_id: Uuid) -> Vec<ConversationPreview> {
        Vec::new()
    }
    fn list_company_conversations_result(
        &self,
        company_id: Uuid,
    ) -> AppResult<Vec<ConversationPreview>> {
        Ok(self.list_company_conversations(company_id))
    }
    fn get_conversation_messages(&self, conversation_id: Uuid) -> Vec<MessageView>;
    fn get_conversation_messages_result(
        &self,
        conversation_id: Uuid,
    ) -> AppResult<Vec<MessageView>> {
        Ok(self.get_conversation_messages(conversation_id))
    }
    fn get_conversation_message_page(
        &self,
        conversation_id: Uuid,
        before_message_id: Option<Uuid>,
        limit: usize,
    ) -> AppResult<MessagePageView> {
        crate::pagination::conversation_message_page(
            self.get_conversation_messages_result(conversation_id)?,
            before_message_id,
            limit,
        )
    }
    fn append_message(&self, message: MessageView) -> AppResult<()>;
    fn append_message_with_metadata(
        &self,
        message: MessageView,
        _content_json: serde_json::Value,
    ) -> AppResult<()> {
        self.append_message(message)
    }
    fn conversation_exists(&self, conversation_id: Uuid) -> bool;
    fn list_agent_action_logs(&self, agent_id: Uuid, limit: usize) -> Vec<AgentActionLog>;
}

pub use crate::platform::PlatformApp;
#[cfg(test)]
#[path = "service_tests/mod.rs"]
mod tests;
