use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use uuid::Uuid;

use ai_chat_domain::agent_identity::{
    AgentActionLog, AgentIdempotencyRecord, AgentInboxEvent, AgentInboxEventStatus,
    AgentKeyIssueLog, AgentKeyRecord, AgentOwnerBinding, AgentProfile, AgentRegistrationRequest,
    AgentStatus, HumanAccountToken, HumanCredential, HumanSession, HumanUser,
    OwnershipProofChallenge, SocialProofSubmission,
};
use ai_chat_domain::company::{
    AgentCodexRunActivity, AgentCodexRunToken, AgentCodexSession, AgentCodexTriggerConfig,
    AgentCodexTriggerRun, AgentModelPriceCatalogEntry, AgentRuntimeConfig, AgentRuntimeDailyUsage,
    AgentRuntimeRun, AgentRuntimeTemplate, AgentStaffingAction, AgentToolApprovalRequest, Company,
    CompanyAgentMembership, CompanyCodexRunnerProfile, CompanyGovernancePolicyVersion,
    CompanyHumanMember, CompanyModelBudgetPolicy, CompanyModelDailyUsage, CompanyProject,
    CompanyProjectAsset, CompanyProjectAssetRefreshConfig, CompanyProjectGitConfig,
    CompanyProjectMember, CompanyProjectRule, CompanyProjectStatusUpdate, CompanyProjectTask,
    CompanyProjectTaskDependency, CompanyProjectTaskStatusHistory, CompanyRuntimePolicy, OrgUnit,
    AGENT_CODEX_RUN_STATUS_RUNNING, AGENT_CODEX_RUN_STATUS_TIMED_OUT,
    AGENT_CODEX_TRIGGER_STATUS_ACTIVE, AGENT_CODEX_TRIGGER_STATUS_ERROR,
    AGENT_MODEL_PRICE_CATALOG_STATUS_ACTIVE, AGENT_MODEL_PRICE_CATALOG_STATUS_ARCHIVED,
    AGENT_RUNTIME_RUN_STATUS_FAILED, AGENT_RUNTIME_RUN_STATUS_SKIPPED_BUDGET,
    AGENT_RUNTIME_STATUS_ACTIVE, AGENT_TOOL_APPROVAL_STATUS_PENDING,
    COMPANY_GOVERNANCE_POLICY_STATUS_ACTIVE, COMPANY_GOVERNANCE_POLICY_STATUS_ARCHIVED,
};
use ai_chat_domain::social::{
    ConversationContext, ConversationPreview, DiaryEntryView, FriendProfileSnapshot,
    FriendRequestView, FriendSummary, MessageView, PostCommentView, PostView,
    CONVERSATION_CONTEXT_PROJECT_GROUP, CONVERSATION_CONTEXT_SELF_NOTES,
};
use ai_chat_shared::{hash_secret, now_utc, AppResult};

use crate::service::{
    AgentStaffingHireBundle, AgentStaffingStatusChangeBundle, CompanyAgentActivationBundle,
    CompanyAgentCreationBundle, CompanyAgentMembershipUpdateBundle,
    CompanyConversationCreationBundle, CompanyCreationBundle, CompanyProjectCreationBundle,
    CompanyProjectMemberAddBundle, CompleteAgentCodexTriggerLeaseInput,
    HumanCompanyDirectConversationCreationBundle, PlatformRepository, ProblemWorkspace,
    ProblemWorkspaceInvitation, ProblemWorkspaceProgressRecord, RegistrationCompletionBundle,
};

#[derive(Default)]
struct MemoryState {
    human_users: HashMap<Uuid, HumanUser>,
    human_credentials: HashMap<Uuid, HumanCredential>,
    human_sessions: HashMap<Uuid, HumanSession>,
    human_account_tokens: HashMap<Uuid, HumanAccountToken>,
    human_email_verifications: HashMap<Uuid, chrono::DateTime<chrono::Utc>>,
    companies: HashMap<Uuid, Company>,
    company_human_members: HashMap<(Uuid, Uuid), CompanyHumanMember>,
    org_units: HashMap<Uuid, OrgUnit>,
    company_agent_memberships: HashMap<Uuid, CompanyAgentMembership>,
    agent_staffing_actions: HashMap<Uuid, AgentStaffingAction>,
    agent_profiles: HashMap<Uuid, AgentProfile>,
    owner_bindings: Vec<AgentOwnerBinding>,
    registration_requests: HashMap<Uuid, AgentRegistrationRequest>,
    challenges: HashMap<Uuid, OwnershipProofChallenge>,
    submissions: Vec<SocialProofSubmission>,
    inbox_events: HashMap<Uuid, AgentInboxEvent>,
    action_logs: Vec<AgentActionLog>,
    idempotency_records: HashMap<(Uuid, String, String), AgentIdempotencyRecord>,
    agent_key_issue_logs: Vec<AgentKeyIssueLog>,
    agent_keys: HashMap<Uuid, AgentKeyRecord>,
    agent_keys_by_hash: HashMap<String, Uuid>,
    conversations: HashMap<Uuid, Vec<ConversationPreview>>,
    messages: HashMap<Uuid, Vec<MessageView>>,
    posts: HashMap<Uuid, PostView>,
    post_comments: HashMap<Uuid, Vec<PostCommentView>>,
    diary_entries: HashMap<Uuid, DiaryEntryView>,
    friend_requests: HashMap<Uuid, FriendRequestView>,
    friendships: Vec<(Uuid, Uuid)>,
    friend_profiles: HashMap<(Uuid, Uuid), FriendProfileSnapshot>,
    direct_conversations: HashMap<(Uuid, Uuid), Uuid>,
    company_direct_conversations: HashMap<(Uuid, Uuid, Uuid), Uuid>,
    company_human_direct_conversations: HashMap<(Uuid, Uuid, Uuid), Uuid>,
    company_default_groups: HashMap<Uuid, ConversationPreview>,
    conversation_contexts: HashMap<Uuid, ConversationContext>,
    company_projects: HashMap<Uuid, CompanyProject>,
    company_project_git_configs: HashMap<Uuid, CompanyProjectGitConfig>,
    company_project_rules: HashMap<Uuid, CompanyProjectRule>,
    company_project_assets: HashMap<Uuid, Vec<CompanyProjectAsset>>,
    company_project_asset_refresh_configs: HashMap<Uuid, CompanyProjectAssetRefreshConfig>,
    company_codex_runner_profiles: HashMap<Uuid, CompanyCodexRunnerProfile>,
    agent_codex_runner_profile_assignments: HashMap<Uuid, Uuid>,
    agent_codex_trigger_configs: HashMap<Uuid, AgentCodexTriggerConfig>,
    agent_codex_trigger_runs: HashMap<Uuid, AgentCodexTriggerRun>,
    agent_codex_sessions: HashMap<Uuid, AgentCodexSession>,
    agent_codex_run_tokens: HashMap<String, AgentCodexRunToken>,
    company_project_members: HashMap<(Uuid, Uuid), CompanyProjectMember>,
    company_project_tasks: HashMap<Uuid, CompanyProjectTask>,
    company_project_task_dependencies: HashMap<Uuid, CompanyProjectTaskDependency>,
    company_project_task_status_history: Vec<CompanyProjectTaskStatusHistory>,
    company_project_status_updates: HashMap<Uuid, Vec<CompanyProjectStatusUpdate>>,
    agent_runtime_configs: HashMap<Uuid, AgentRuntimeConfig>,
    agent_runtime_templates: HashMap<Uuid, AgentRuntimeTemplate>,
    company_runtime_policies: HashMap<Uuid, CompanyRuntimePolicy>,
    agent_model_price_catalog_entries: HashMap<Uuid, AgentModelPriceCatalogEntry>,
    company_governance_policy_versions: HashMap<Uuid, CompanyGovernancePolicyVersion>,
    agent_runtime_runs: HashMap<Uuid, AgentRuntimeRun>,
    company_model_budget_policies: HashMap<Uuid, CompanyModelBudgetPolicy>,
    agent_tool_approval_requests: HashMap<Uuid, AgentToolApprovalRequest>,
    problem_workspaces: HashMap<Uuid, ProblemWorkspace>,
    problem_workspace_progress_records: HashMap<Uuid, Vec<ProblemWorkspaceProgressRecord>>,
    problem_workspace_invitations: HashMap<Uuid, ProblemWorkspaceInvitation>,
}

#[derive(Clone, Default)]
pub struct MemoryPlatformRepository {
    inner: Arc<RwLock<MemoryState>>,
}

impl PlatformRepository for MemoryPlatformRepository {
    fn find_human_user_by_email(&self, email: &str) -> Option<HumanUser> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard
            .human_users
            .values()
            .find(|user| user.email.eq_ignore_ascii_case(email))
            .cloned()
    }

    fn get_human_user(&self, user_id: Uuid) -> Option<HumanUser> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard.human_users.get(&user_id).cloned()
    }

    fn list_human_users(&self) -> Vec<HumanUser> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut users = guard.human_users.values().cloned().collect::<Vec<_>>();
        users.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        users
    }

    fn insert_human_user(&self, user: HumanUser) -> AppResult<HumanUser> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        guard.human_users.insert(user.id, user.clone());
        Ok(user)
    }

    fn human_user_exists(&self, user_id: Uuid) -> bool {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard.human_users.contains_key(&user_id)
    }

    fn get_human_credential(&self, user_id: Uuid) -> Option<HumanCredential> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard.human_credentials.get(&user_id).cloned()
    }

    fn insert_human_auth_bundle(
        &self,
        user: HumanUser,
        credential: HumanCredential,
    ) -> AppResult<HumanUser> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if guard
            .human_users
            .values()
            .any(|existing| existing.email.eq_ignore_ascii_case(&user.email))
        {
            return Err(ai_chat_shared::AppError::Conflict(
                "email is already registered".into(),
            ));
        }
        guard.human_credentials.insert(user.id, credential);
        guard.human_users.insert(user.id, user.clone());
        Ok(user)
    }

    fn insert_human_session(&self, session: HumanSession) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        guard.human_sessions.insert(session.id, session);
        Ok(())
    }

    fn find_human_session_by_token_hash(&self, token_hash: &str) -> Option<HumanSession> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard
            .human_sessions
            .values()
            .find(|session| session.token_hash == token_hash)
            .cloned()
    }

    fn touch_human_session(
        &self,
        session_id: Uuid,
        used_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let session = guard
            .human_sessions
            .get_mut(&session_id)
            .ok_or_else(|| ai_chat_shared::AppError::NotFound("human session not found".into()))?;
        session.last_used_at = Some(used_at);
        Ok(())
    }

    fn revoke_human_session(
        &self,
        session_id: Uuid,
        revoked_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let session = guard
            .human_sessions
            .get_mut(&session_id)
            .ok_or_else(|| ai_chat_shared::AppError::NotFound("human session not found".into()))?;
        session.revoked_at = Some(revoked_at);
        Ok(())
    }

    fn get_human_session(&self, session_id: Uuid) -> Option<HumanSession> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard.human_sessions.get(&session_id).cloned()
    }

    fn list_human_sessions(&self, human_user_id: Uuid) -> Vec<HumanSession> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut sessions = guard
            .human_sessions
            .values()
            .filter(|session| session.human_user_id == human_user_id)
            .cloned()
            .collect::<Vec<_>>();
        sessions.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        sessions
    }

    fn revoke_human_sessions(
        &self,
        human_user_id: Uuid,
        except_session_id: Option<Uuid>,
        revoked_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<usize> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let mut count = 0;
        for session in guard.human_sessions.values_mut() {
            if session.human_user_id == human_user_id
                && session.revoked_at.is_none()
                && Some(session.id) != except_session_id
            {
                session.revoked_at = Some(revoked_at);
                count += 1;
            }
        }
        Ok(count)
    }

    fn update_human_password_hash(
        &self,
        human_user_id: Uuid,
        password_hash: String,
        updated_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let credential = guard
            .human_credentials
            .get_mut(&human_user_id)
            .ok_or_else(|| {
                ai_chat_shared::AppError::NotFound("human credential not found".into())
            })?;
        credential.password_hash = password_hash;
        credential.updated_at = updated_at;
        Ok(())
    }

    fn delete_expired_human_sessions(
        &self,
        now: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<usize> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let before = guard.human_sessions.len();
        guard.human_sessions.retain(|_, session| {
            session.expires_at > now
                || session
                    .revoked_at
                    .is_some_and(|revoked_at| revoked_at > now - chrono::Duration::days(30))
        });
        Ok(before - guard.human_sessions.len())
    }

    fn insert_human_account_token(&self, token: HumanAccountToken) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        guard.human_account_tokens.insert(token.id, token);
        Ok(())
    }

    fn find_human_account_token_by_hash(&self, token_hash: &str) -> Option<HumanAccountToken> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard
            .human_account_tokens
            .values()
            .find(|token| token.token_hash == token_hash)
            .cloned()
    }

    fn mark_human_account_token_used(
        &self,
        token_id: Uuid,
        used_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let token = guard
            .human_account_tokens
            .get_mut(&token_id)
            .ok_or_else(|| {
                ai_chat_shared::AppError::NotFound("human account token not found".into())
            })?;
        if token.used_at.is_some() {
            return Err(ai_chat_shared::AppError::Conflict(
                "human account token was already used".into(),
            ));
        }
        token.used_at = Some(used_at);
        Ok(())
    }

    fn mark_human_email_verified(
        &self,
        human_user_id: Uuid,
        verified_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        guard
            .human_email_verifications
            .insert(human_user_id, verified_at);
        Ok(())
    }

    fn is_human_email_verified(&self, human_user_id: Uuid) -> bool {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard.human_email_verifications.contains_key(&human_user_id)
    }

    fn delete_expired_human_account_tokens(
        &self,
        now: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<usize> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let before = guard.human_account_tokens.len();
        guard.human_account_tokens.retain(|_, token| {
            token.expires_at > now
                || token
                    .used_at
                    .is_some_and(|used_at| used_at > now - chrono::Duration::days(30))
        });
        Ok(before - guard.human_account_tokens.len())
    }

    fn insert_company_bundle(&self, bundle: CompanyCreationBundle) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if guard
            .companies
            .values()
            .any(|company| company.slug == bundle.company.slug)
        {
            return Err(ai_chat_shared::AppError::Conflict(
                "company slug is already in use".into(),
            ));
        }
        let company_id = bundle.company.id;
        let default_group_id = bundle.default_group.preview.id;
        guard.company_human_members.insert(
            (
                bundle.owner_membership.company_id,
                bundle.owner_membership.human_user_id,
            ),
            bundle.owner_membership,
        );
        guard
            .org_units
            .insert(bundle.root_org_unit.id, bundle.root_org_unit);
        guard.companies.insert(bundle.company.id, bundle.company);
        guard
            .company_default_groups
            .insert(company_id, bundle.default_group.preview);
        guard
            .conversation_contexts
            .insert(default_group_id, bundle.default_group.context);
        Ok(())
    }

    fn get_company(&self, company_id: Uuid) -> Option<Company> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard.companies.get(&company_id).cloned()
    }

    fn get_company_by_slug(&self, slug: &str) -> Option<Company> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard
            .companies
            .values()
            .find(|company| company.slug.eq_ignore_ascii_case(slug))
            .cloned()
    }

    fn list_human_companies(&self, human_user_id: Uuid) -> Vec<Company> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut companies = guard
            .company_human_members
            .values()
            .filter(|membership| {
                membership.human_user_id == human_user_id && membership.status == "active"
            })
            .filter_map(|membership| guard.companies.get(&membership.company_id).cloned())
            .collect::<Vec<_>>();
        companies.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        companies
    }

    fn get_company_human_member(
        &self,
        company_id: Uuid,
        human_user_id: Uuid,
    ) -> Option<CompanyHumanMember> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard
            .company_human_members
            .get(&(company_id, human_user_id))
            .cloned()
    }

    fn get_org_unit(&self, org_unit_id: Uuid) -> Option<OrgUnit> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard.org_units.get(&org_unit_id).cloned()
    }

    fn insert_org_unit(&self, org_unit: OrgUnit) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if !guard.companies.contains_key(&org_unit.company_id) {
            return Err(ai_chat_shared::AppError::NotFound(
                "company not found".into(),
            ));
        }
        guard.org_units.insert(org_unit.id, org_unit);
        Ok(())
    }

    fn list_company_org_units(&self, company_id: Uuid) -> Vec<OrgUnit> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut org_units = guard
            .org_units
            .values()
            .filter(|org_unit| org_unit.company_id == company_id)
            .cloned()
            .collect::<Vec<_>>();
        org_units.sort_by(|left, right| {
            left.sort_order
                .cmp(&right.sort_order)
                .then_with(|| left.created_at.cmp(&right.created_at))
        });
        org_units
    }

    fn get_company_agent_membership(&self, agent_id: Uuid) -> Option<CompanyAgentMembership> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard.company_agent_memberships.get(&agent_id).cloned()
    }

    fn list_company_agent_memberships(&self, company_id: Uuid) -> Vec<CompanyAgentMembership> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut memberships = guard
            .company_agent_memberships
            .values()
            .filter(|membership| membership.company_id == company_id)
            .cloned()
            .collect::<Vec<_>>();
        memberships.sort_by(|left, right| right.joined_at.cmp(&left.joined_at));
        memberships
    }

    fn update_company_agent_work_profile(
        &self,
        agent_id: Uuid,
        responsibilities: Vec<String>,
        skills: Vec<String>,
        current_focus: String,
        collaboration_preference: String,
        updated_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let membership = guard
            .company_agent_memberships
            .get_mut(&agent_id)
            .ok_or_else(|| {
                ai_chat_shared::AppError::NotFound("company membership not found".into())
            })?;
        membership.responsibilities = responsibilities;
        membership.skills = skills;
        membership.current_focus = current_focus;
        membership.updated_at = updated_at;

        let profile = guard
            .agent_profiles
            .get_mut(&agent_id)
            .ok_or_else(|| ai_chat_shared::AppError::NotFound("agent profile not found".into()))?;
        profile.collaboration_preference = collaboration_preference;
        Ok(())
    }

    fn complete_company_agent_creation_bundle(
        &self,
        bundle: CompanyAgentCreationBundle,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if guard
            .agent_profiles
            .values()
            .any(|profile| profile.handle == bundle.agent_profile.handle)
        {
            return Err(ai_chat_shared::AppError::Conflict(
                "agent handle is already in use".into(),
            ));
        }
        if guard
            .company_agent_memberships
            .contains_key(&bundle.agent_profile.id)
        {
            return Err(ai_chat_shared::AppError::Conflict(
                "agent already belongs to a company".into(),
            ));
        }
        if guard
            .agent_keys_by_hash
            .contains_key(&bundle.key_record.key_hash)
        {
            return Err(ai_chat_shared::AppError::Conflict(
                "agent key already exists".into(),
            ));
        }

        let company_id = bundle.membership.company_id;
        let self_notes_conversation_id = bundle.self_notes_conversation.id;
        guard.owner_bindings.push(bundle.owner_binding);
        guard
            .company_agent_memberships
            .insert(bundle.membership.agent_profile_id, bundle.membership);
        guard
            .agent_keys_by_hash
            .insert(bundle.key_record.key_hash.clone(), bundle.key_record.id);
        guard
            .agent_keys
            .insert(bundle.key_record.id, bundle.key_record);
        guard.agent_key_issue_logs.push(bundle.key_issue_log);
        guard
            .conversations
            .entry(bundle.agent_profile.id)
            .or_default()
            .push(bundle.self_notes_conversation);
        if let Some(default_group) = guard.company_default_groups.get(&company_id).cloned() {
            guard
                .conversations
                .entry(bundle.agent_profile.id)
                .or_default()
                .push(default_group);
        }
        guard.conversation_contexts.insert(
            self_notes_conversation_id,
            ConversationContext {
                conversation_id: self_notes_conversation_id,
                company_id: Some(company_id),
                project_id: None,
                context_type: CONVERSATION_CONTEXT_SELF_NOTES.into(),
                visibility: "private".into(),
            },
        );
        guard
            .agent_profiles
            .insert(bundle.agent_profile.id, bundle.agent_profile);
        if let Some(runtime_config) = bundle.runtime_config {
            guard
                .agent_runtime_configs
                .insert(runtime_config.id, runtime_config);
        }
        Ok(())
    }

    fn complete_company_agent_membership_update(
        &self,
        bundle: CompanyAgentMembershipUpdateBundle,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if !guard
            .company_agent_memberships
            .contains_key(&bundle.membership.agent_profile_id)
        {
            return Err(ai_chat_shared::AppError::NotFound(
                "company agent membership not found".into(),
            ));
        }
        guard
            .company_agent_memberships
            .insert(bundle.membership.agent_profile_id, bundle.membership);
        guard
            .agent_staffing_actions
            .insert(bundle.action.id, bundle.action);
        Ok(())
    }

    fn complete_agent_staffing_hire(&self, bundle: AgentStaffingHireBundle) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if guard
            .agent_profiles
            .values()
            .any(|profile| profile.handle == bundle.agent_profile.handle)
        {
            return Err(ai_chat_shared::AppError::Conflict(
                "agent handle is already in use".into(),
            ));
        }
        if guard
            .company_agent_memberships
            .contains_key(&bundle.agent_profile.id)
        {
            return Err(ai_chat_shared::AppError::Conflict(
                "agent already belongs to a company".into(),
            ));
        }

        let company_id = bundle.membership.company_id;
        let self_notes_conversation_id = bundle.self_notes_conversation.id;
        guard.owner_bindings.push(bundle.owner_binding);
        guard
            .company_agent_memberships
            .insert(bundle.membership.agent_profile_id, bundle.membership);
        guard
            .conversations
            .entry(bundle.agent_profile.id)
            .or_default()
            .push(bundle.self_notes_conversation);
        guard.conversation_contexts.insert(
            self_notes_conversation_id,
            ConversationContext {
                conversation_id: self_notes_conversation_id,
                company_id: Some(company_id),
                project_id: None,
                context_type: CONVERSATION_CONTEXT_SELF_NOTES.into(),
                visibility: "private".into(),
            },
        );
        guard
            .agent_staffing_actions
            .insert(bundle.action.id, bundle.action);
        guard
            .agent_profiles
            .insert(bundle.agent_profile.id, bundle.agent_profile);
        Ok(())
    }

    fn complete_company_agent_activation(
        &self,
        bundle: CompanyAgentActivationBundle,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let company_id = bundle.membership.company_id;
        let agent_id = bundle.membership.agent_profile_id;
        if !guard.agent_profiles.contains_key(&bundle.agent_profile.id)
            || !guard
                .company_agent_memberships
                .contains_key(&bundle.membership.agent_profile_id)
        {
            return Err(ai_chat_shared::AppError::NotFound(
                "company agent not found".into(),
            ));
        }
        if guard
            .agent_keys_by_hash
            .contains_key(&bundle.key_record.key_hash)
        {
            return Err(ai_chat_shared::AppError::Conflict(
                "agent key already exists".into(),
            ));
        }

        guard
            .agent_profiles
            .insert(bundle.agent_profile.id, bundle.agent_profile);
        guard
            .company_agent_memberships
            .insert(bundle.membership.agent_profile_id, bundle.membership);
        guard
            .agent_keys_by_hash
            .insert(bundle.key_record.key_hash.clone(), bundle.key_record.id);
        guard
            .agent_keys
            .insert(bundle.key_record.id, bundle.key_record);
        guard.agent_key_issue_logs.push(bundle.key_issue_log);
        guard
            .agent_staffing_actions
            .insert(bundle.action.id, bundle.action);
        if let Some(default_group) = guard.company_default_groups.get(&company_id).cloned() {
            let conversations = guard.conversations.entry(agent_id).or_default();
            if !conversations
                .iter()
                .any(|conversation| conversation.id == default_group.id)
            {
                conversations.push(default_group);
            }
        }
        if let Some(runtime_config) = bundle.runtime_config {
            guard
                .agent_runtime_configs
                .insert(runtime_config.id, runtime_config);
        }
        Ok(())
    }

    fn complete_agent_staffing_status_change(
        &self,
        bundle: AgentStaffingStatusChangeBundle,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if !guard.agent_profiles.contains_key(&bundle.agent_profile.id)
            || !guard
                .company_agent_memberships
                .contains_key(&bundle.membership.agent_profile_id)
        {
            return Err(ai_chat_shared::AppError::NotFound(
                "company agent not found".into(),
            ));
        }
        if bundle
            .reassigned_tasks
            .iter()
            .any(|task| !guard.company_project_tasks.contains_key(&task.id))
        {
            return Err(ai_chat_shared::AppError::NotFound(
                "company project task not found".into(),
            ));
        }
        let changed_at = bundle.action.created_at;
        for key in guard.agent_keys.values_mut() {
            if key.agent_profile_id == bundle.agent_profile.id && key.revoked_at.is_none() {
                key.revoked_at = Some(changed_at);
            }
        }
        guard
            .agent_profiles
            .insert(bundle.agent_profile.id, bundle.agent_profile);
        guard
            .company_agent_memberships
            .insert(bundle.membership.agent_profile_id, bundle.membership);
        for task in bundle.reassigned_tasks {
            if let Some(project) = guard.company_projects.get_mut(&task.project_id) {
                project.updated_at = task.updated_at;
                project.updated_by_agent_id = task.updated_by_agent_id;
            }
            guard.company_project_tasks.insert(task.id, task);
        }
        guard.agent_key_issue_logs.extend(bundle.key_issue_logs);
        guard
            .agent_staffing_actions
            .insert(bundle.action.id, bundle.action);
        Ok(())
    }

    fn get_agent_staffing_action(&self, action_id: Uuid) -> Option<AgentStaffingAction> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard.agent_staffing_actions.get(&action_id).cloned()
    }

    fn list_agent_staffing_actions(&self, company_id: Uuid) -> Vec<AgentStaffingAction> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut actions = guard
            .agent_staffing_actions
            .values()
            .filter(|action| action.company_id == company_id)
            .cloned()
            .collect::<Vec<_>>();
        actions.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        actions
    }

    fn insert_registration_request(&self, request: AgentRegistrationRequest) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        guard.registration_requests.insert(request.id, request);
        Ok(())
    }

    fn insert_ownership_challenge(&self, challenge: OwnershipProofChallenge) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        guard.challenges.insert(challenge.id, challenge);
        Ok(())
    }

    fn get_challenge(&self, challenge_id: Uuid) -> Option<OwnershipProofChallenge> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard.challenges.get(&challenge_id).cloned()
    }

    fn list_challenges(&self) -> Vec<OwnershipProofChallenge> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut challenges = guard.challenges.values().cloned().collect::<Vec<_>>();
        challenges.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        challenges
    }

    fn update_challenge(&self, challenge: OwnershipProofChallenge) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        guard.challenges.insert(challenge.id, challenge);
        Ok(())
    }

    fn get_registration_request(&self, request_id: Uuid) -> Option<AgentRegistrationRequest> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard.registration_requests.get(&request_id).cloned()
    }

    fn list_registration_requests(&self) -> Vec<AgentRegistrationRequest> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut requests = guard
            .registration_requests
            .values()
            .cloned()
            .collect::<Vec<_>>();
        requests.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        requests
    }

    fn update_registration_request(&self, request: AgentRegistrationRequest) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        guard.registration_requests.insert(request.id, request);
        Ok(())
    }

    fn insert_agent_profile(&self, profile: AgentProfile) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        guard.agent_profiles.insert(profile.id, profile);
        Ok(())
    }

    fn complete_registration_bundle(&self, bundle: RegistrationCompletionBundle) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let agent_id = bundle.agent.id;

        guard
            .challenges
            .insert(bundle.challenge.id, bundle.challenge);
        guard
            .registration_requests
            .insert(bundle.registration.id, bundle.registration);
        guard.agent_profiles.insert(bundle.agent.id, bundle.agent);
        guard.owner_bindings.push(bundle.binding);

        let key_id = bundle.key_record.id;
        let key_hash = bundle.key_record.key_hash.clone();
        guard
            .agent_keys
            .insert(bundle.key_record.id, bundle.key_record);
        guard.agent_keys_by_hash.insert(key_hash, key_id);

        guard.agent_key_issue_logs.push(bundle.key_issue_log);
        guard.submissions.push(bundle.submission);
        guard.conversations.entry(agent_id).or_default();
        guard
            .conversations
            .entry(agent_id)
            .or_default()
            .push(bundle.self_notes_conversation);
        Ok(())
    }

    fn list_agent_profiles(&self) -> Vec<AgentProfile> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut profiles = guard.agent_profiles.values().cloned().collect::<Vec<_>>();
        profiles.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        profiles
    }

    fn update_agent_profile_status(&self, agent_id: Uuid, status: AgentStatus) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let Some(profile) = guard.agent_profiles.get_mut(&agent_id) else {
            return Err(ai_chat_shared::AppError::NotFound("agent not found".into()));
        };
        profile.status = status;
        Ok(())
    }

    fn update_agent_profile(
        &self,
        agent_id: Uuid,
        display_name: String,
        persona: String,
        collaboration_preference: String,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let Some(profile) = guard.agent_profiles.get_mut(&agent_id) else {
            return Err(ai_chat_shared::AppError::NotFound("agent not found".into()));
        };
        profile.display_name = display_name;
        profile.persona = persona;
        profile.collaboration_preference = collaboration_preference;
        Ok(())
    }

    fn insert_owner_binding(&self, binding: AgentOwnerBinding) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        guard.owner_bindings.push(binding);
        Ok(())
    }

    fn insert_agent_key(&self, key_record: AgentKeyRecord) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let key_id = key_record.id;
        let key_hash = key_record.key_hash.clone();
        guard.agent_keys.insert(key_record.id, key_record);
        guard.agent_keys_by_hash.insert(key_hash, key_id);
        Ok(())
    }

    fn find_agent_key_by_plaintext(&self, plaintext_key: &str) -> Option<AgentKeyRecord> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let key_id = guard.agent_keys_by_hash.get(&hash_secret(plaintext_key))?;
        let key = guard.agent_keys.get(key_id)?;
        if key.revoked_at.is_some() {
            return None;
        }
        if key
            .expires_at
            .is_some_and(|expires_at| now_utc() >= expires_at)
        {
            return None;
        }
        Some(key.clone())
    }

    fn list_agent_keys(&self, agent_id: Uuid) -> Vec<AgentKeyRecord> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut keys = guard
            .agent_keys
            .values()
            .filter(|key| key.agent_profile_id == agent_id)
            .cloned()
            .collect::<Vec<_>>();
        keys.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        keys
    }

    fn touch_agent_key_usage(
        &self,
        key_id: Uuid,
        used_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let Some(key) = guard.agent_keys.get_mut(&key_id) else {
            return Err(ai_chat_shared::AppError::NotFound(
                "agent key not found".into(),
            ));
        };
        key.last_used_at = Some(used_at);
        Ok(())
    }

    fn revoke_agent_keys(
        &self,
        agent_id: Uuid,
        revoked_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        for key in guard.agent_keys.values_mut() {
            if key.agent_profile_id == agent_id && key.revoked_at.is_none() {
                key.revoked_at = Some(revoked_at);
            }
        }
        Ok(())
    }

    fn insert_agent_key_issue_log(&self, log: AgentKeyIssueLog) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        guard.agent_key_issue_logs.push(log);
        Ok(())
    }

    fn list_agent_key_issue_logs(&self, agent_id: Uuid, limit: usize) -> Vec<AgentKeyIssueLog> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut logs = guard
            .agent_key_issue_logs
            .iter()
            .filter(|item| item.agent_profile_id == agent_id)
            .cloned()
            .collect::<Vec<_>>();
        logs.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        logs.truncate(limit);
        logs
    }

    fn insert_social_proof_submission(&self, submission: SocialProofSubmission) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        guard.submissions.push(submission);
        Ok(())
    }

    fn list_social_proof_submissions(&self) -> Vec<SocialProofSubmission> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut submissions = guard.submissions.clone();
        submissions.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        submissions
    }

    fn insert_agent_inbox_event(&self, event: AgentInboxEvent) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        guard.inbox_events.insert(event.id, event);
        Ok(())
    }

    fn list_agent_inbox_events(
        &self,
        agent_id: Uuid,
        status: Option<AgentInboxEventStatus>,
        limit: usize,
    ) -> Vec<AgentInboxEvent> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut items = guard
            .inbox_events
            .values()
            .filter(|item| item.agent_profile_id == agent_id)
            .filter(|item| match status.as_ref() {
                Some(value) => item.status == *value,
                None => true,
            })
            .cloned()
            .collect::<Vec<_>>();
        items.sort_by(|left, right| {
            left.priority
                .cmp(&right.priority)
                .then_with(|| right.created_at.cmp(&left.created_at))
        });
        items.truncate(limit);
        items
    }

    fn get_agent_inbox_event(&self, event_id: Uuid) -> Option<AgentInboxEvent> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard.inbox_events.get(&event_id).cloned()
    }

    fn update_agent_inbox_event(&self, event: AgentInboxEvent) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        guard.inbox_events.insert(event.id, event);
        Ok(())
    }

    fn insert_agent_action_log(&self, log: AgentActionLog) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        guard.action_logs.push(log);
        Ok(())
    }

    fn list_all_agent_action_logs(&self) -> Vec<AgentActionLog> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut logs = guard.action_logs.clone();
        logs.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        logs
    }

    fn count_agent_actions_since(
        &self,
        agent_id: Uuid,
        since: chrono::DateTime<chrono::Utc>,
    ) -> usize {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard
            .action_logs
            .iter()
            .filter(|log| log.agent_profile_id == agent_id && log.created_at >= since)
            .count()
    }

    fn get_agent_idempotency_record(
        &self,
        agent_id: Uuid,
        operation: &str,
        idempotency_key: &str,
    ) -> Option<AgentIdempotencyRecord> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard
            .idempotency_records
            .get(&(agent_id, operation.to_string(), idempotency_key.to_string()))
            .cloned()
    }

    fn insert_agent_idempotency_record(&self, record: AgentIdempotencyRecord) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        guard
            .idempotency_records
            .entry((
                record.agent_profile_id,
                record.operation.clone(),
                record.idempotency_key.clone(),
            ))
            .or_insert(record);
        Ok(())
    }

    fn delete_expired_agent_idempotency_records(
        &self,
        now: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<usize> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let before = guard.idempotency_records.len();
        guard
            .idempotency_records
            .retain(|_, record| record.expires_at > now);
        Ok(before - guard.idempotency_records.len())
    }

    fn ensure_agent_conversation_bucket(&self, agent_id: Uuid) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        guard.conversations.entry(agent_id).or_default();
        Ok(())
    }

    fn insert_conversation_preview(
        &self,
        agent_id: Uuid,
        preview: ConversationPreview,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let preview_id = preview.id;
        guard
            .conversations
            .entry(agent_id)
            .or_default()
            .push(preview);
        let all_lists = guard.conversations.values().collect::<Vec<_>>();
        let participants = all_lists
            .iter()
            .filter(|items| items.iter().any(|item| item.id == preview_id))
            .count();
        if participants >= 2 {
            let owners = guard
                .conversations
                .iter()
                .filter_map(|(owner_id, items)| {
                    items
                        .iter()
                        .any(|item| item.id == preview_id)
                        .then_some(*owner_id)
                })
                .collect::<Vec<_>>();
            if owners.len() == 2 {
                let pair = ordered_pair(owners[0], owners[1]);
                guard.direct_conversations.insert(pair, preview_id);
            }
        }
        Ok(())
    }

    fn complete_company_conversation_creation(
        &self,
        bundle: CompanyConversationCreationBundle,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let first = bundle.members.first().ok_or_else(|| {
            ai_chat_shared::AppError::Validation("conversation members required".into())
        })?;
        let conversation_id = first.preview.id;
        if bundle
            .members
            .iter()
            .any(|member| member.preview.id != conversation_id)
        {
            return Err(ai_chat_shared::AppError::Validation(
                "company conversation previews must share one id".into(),
            ));
        }
        if bundle
            .members
            .iter()
            .any(|member| !guard.agent_profiles.contains_key(&member.agent_id))
        {
            return Err(ai_chat_shared::AppError::NotFound(
                "company conversation member not found".into(),
            ));
        }
        if let Some((left_agent_id, right_agent_id)) = bundle.direct_pair {
            let key = (bundle.company_id, left_agent_id, right_agent_id);
            if guard.company_direct_conversations.contains_key(&key) {
                return Err(ai_chat_shared::AppError::Conflict(
                    "company direct conversation already exists".into(),
                ));
            }
            guard
                .company_direct_conversations
                .insert(key, conversation_id);
            guard
                .direct_conversations
                .insert(ordered_pair(left_agent_id, right_agent_id), conversation_id);
        }
        for member in bundle.members {
            guard
                .conversations
                .entry(member.agent_id)
                .or_default()
                .push(member.preview);
        }
        guard.conversation_contexts.insert(
            conversation_id,
            ConversationContext {
                conversation_id,
                company_id: Some(bundle.company_id),
                project_id: None,
                context_type: bundle.context_type,
                visibility: bundle.visibility,
            },
        );
        Ok(())
    }

    fn complete_human_company_direct_conversation_creation(
        &self,
        bundle: HumanCompanyDirectConversationCreationBundle,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if !guard.human_users.contains_key(&bundle.human_user_id) {
            return Err(ai_chat_shared::AppError::NotFound(
                "human conversation sender not found".into(),
            ));
        }
        if !guard.agent_profiles.contains_key(&bundle.target_agent_id) {
            return Err(ai_chat_shared::AppError::NotFound(
                "human conversation target not found".into(),
            ));
        }
        let key = (
            bundle.company_id,
            bundle.human_user_id,
            bundle.target_agent_id,
        );
        if guard.company_human_direct_conversations.contains_key(&key) {
            return Err(ai_chat_shared::AppError::Conflict(
                "human company direct conversation already exists".into(),
            ));
        }
        let conversation_id = bundle.preview.id;
        guard
            .company_human_direct_conversations
            .insert(key, conversation_id);
        guard
            .conversations
            .entry(bundle.target_agent_id)
            .or_default()
            .push(bundle.preview);
        guard.conversation_contexts.insert(
            conversation_id,
            ConversationContext {
                conversation_id,
                company_id: Some(bundle.company_id),
                project_id: None,
                context_type: ai_chat_domain::social::CONVERSATION_CONTEXT_COMPANY_DIRECT.into(),
                visibility: "members".into(),
            },
        );
        Ok(())
    }

    fn find_company_direct_conversation(
        &self,
        company_id: Uuid,
        left_agent_id: Uuid,
        right_agent_id: Uuid,
    ) -> Option<ConversationPreview> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let (left_agent_id, right_agent_id) = ordered_pair(left_agent_id, right_agent_id);
        let conversation_id =
            guard
                .company_direct_conversations
                .get(&(company_id, left_agent_id, right_agent_id))?;
        guard
            .conversations
            .get(&left_agent_id)
            .and_then(|items| items.iter().find(|preview| preview.id == *conversation_id))
            .cloned()
    }

    fn find_human_company_direct_conversation(
        &self,
        company_id: Uuid,
        human_user_id: Uuid,
        target_agent_id: Uuid,
    ) -> Option<ConversationPreview> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let conversation_id = guard.company_human_direct_conversations.get(&(
            company_id,
            human_user_id,
            target_agent_id,
        ))?;
        guard
            .conversations
            .get(&target_agent_id)
            .and_then(|items| items.iter().find(|preview| preview.id == *conversation_id))
            .cloned()
    }

    fn get_conversation_context(&self, conversation_id: Uuid) -> Option<ConversationContext> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard.conversation_contexts.get(&conversation_id).cloned()
    }

    fn list_conversation_member_ids(&self, conversation_id: Uuid) -> Vec<Uuid> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut member_ids = guard
            .conversations
            .iter()
            .filter_map(|(agent_id, previews)| {
                previews
                    .iter()
                    .any(|preview| preview.id == conversation_id)
                    .then_some(*agent_id)
            })
            .collect::<Vec<_>>();
        member_ids.sort();
        member_ids
    }

    fn complete_company_project_creation(
        &self,
        bundle: CompanyProjectCreationBundle,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if guard.company_projects.contains_key(&bundle.project.id) {
            return Err(ai_chat_shared::AppError::Conflict(
                "company project already exists".into(),
            ));
        }
        if bundle.conversation_members.is_empty() {
            return Err(ai_chat_shared::AppError::Validation(
                "project conversation members required".into(),
            ));
        }
        let conversation_id = bundle.project.project_group_conversation_id;
        if bundle
            .conversation_members
            .iter()
            .any(|member| member.preview.id != conversation_id)
        {
            return Err(ai_chat_shared::AppError::Validation(
                "project conversation previews must share one id".into(),
            ));
        }
        for conversation_member in bundle.conversation_members {
            guard
                .conversations
                .entry(conversation_member.agent_id)
                .or_default()
                .push(conversation_member.preview);
        }
        guard.conversation_contexts.insert(
            conversation_id,
            ConversationContext {
                conversation_id,
                company_id: Some(bundle.project.company_id),
                project_id: Some(bundle.project.id),
                context_type: CONVERSATION_CONTEXT_PROJECT_GROUP.into(),
                visibility: "members".into(),
            },
        );
        for member in bundle.members {
            guard
                .company_project_members
                .insert((member.project_id, member.agent_profile_id), member);
        }
        guard
            .company_projects
            .insert(bundle.project.id, bundle.project);
        Ok(())
    }

    fn get_company_project(&self, project_id: Uuid) -> Option<CompanyProject> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard.company_projects.get(&project_id).cloned()
    }

    fn list_company_projects(&self, company_id: Uuid) -> Vec<CompanyProject> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut projects = guard
            .company_projects
            .values()
            .filter(|project| project.company_id == company_id)
            .cloned()
            .collect::<Vec<_>>();
        projects.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        projects
    }

    fn save_company_project_git_config(&self, config: CompanyProjectGitConfig) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if !guard.company_projects.contains_key(&config.project_id) {
            return Err(ai_chat_shared::AppError::NotFound(
                "company project not found".into(),
            ));
        }
        guard
            .company_project_git_configs
            .insert(config.project_id, config);
        Ok(())
    }

    fn get_company_project_git_config(&self, project_id: Uuid) -> Option<CompanyProjectGitConfig> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard.company_project_git_configs.get(&project_id).cloned()
    }

    fn delete_company_project_git_config(&self, project_id: Uuid) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        guard.company_project_git_configs.remove(&project_id);
        Ok(())
    }

    fn save_company_project_rule(&self, rule: CompanyProjectRule) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if !guard.company_projects.contains_key(&rule.project_id) {
            return Err(ai_chat_shared::AppError::NotFound(
                "company project not found".into(),
            ));
        }
        guard.company_project_rules.insert(rule.project_id, rule);
        Ok(())
    }

    fn get_company_project_rule(&self, project_id: Uuid) -> Option<CompanyProjectRule> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard.company_project_rules.get(&project_id).cloned()
    }

    fn replace_company_project_assets(
        &self,
        project_id: Uuid,
        assets: Vec<CompanyProjectAsset>,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if !guard.company_projects.contains_key(&project_id) {
            return Err(ai_chat_shared::AppError::NotFound(
                "company project not found".into(),
            ));
        }
        guard.company_project_assets.insert(project_id, assets);
        Ok(())
    }

    fn list_company_project_assets(&self, project_id: Uuid) -> Vec<CompanyProjectAsset> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard
            .company_project_assets
            .get(&project_id)
            .cloned()
            .unwrap_or_default()
    }

    fn save_company_project_asset_refresh_config(
        &self,
        config: CompanyProjectAssetRefreshConfig,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if !guard.company_projects.contains_key(&config.project_id) {
            return Err(ai_chat_shared::AppError::NotFound(
                "company project not found".into(),
            ));
        }
        guard
            .company_project_asset_refresh_configs
            .insert(config.project_id, config);
        Ok(())
    }

    fn get_company_project_asset_refresh_config(
        &self,
        project_id: Uuid,
    ) -> Option<CompanyProjectAssetRefreshConfig> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard
            .company_project_asset_refresh_configs
            .get(&project_id)
            .cloned()
    }

    fn claim_due_company_project_asset_refresh(
        &self,
        agent_id: Uuid,
        now: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<Option<CompanyProjectAssetRefreshConfig>> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let project_id = guard
            .company_project_asset_refresh_configs
            .values()
            .filter(|config| {
                config.enabled
                    && config.maintainer_agent_id == agent_id
                    && config.next_refresh_at <= now
            })
            .min_by(|left, right| {
                left.next_refresh_at
                    .cmp(&right.next_refresh_at)
                    .then_with(|| left.project_id.cmp(&right.project_id))
            })
            .map(|config| config.project_id);
        let Some(project_id) = project_id else {
            return Ok(None);
        };
        let config = guard
            .company_project_asset_refresh_configs
            .get_mut(&project_id)
            .expect("selected asset refresh config must exist");
        config.last_requested_at = Some(now);
        config.next_refresh_at =
            now + chrono::Duration::minutes(i64::from(config.interval_minutes));
        config.updated_at = now;
        Ok(Some(config.clone()))
    }

    fn mark_company_project_asset_refresh_completed(
        &self,
        project_id: Uuid,
        completed_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if let Some(config) = guard
            .company_project_asset_refresh_configs
            .get_mut(&project_id)
        {
            config.last_completed_at = Some(completed_at);
            config.next_refresh_at =
                completed_at + chrono::Duration::minutes(i64::from(config.interval_minutes));
            config.updated_at = completed_at;
        }
        Ok(())
    }

    fn save_company_codex_runner_profile(
        &self,
        profile: CompanyCodexRunnerProfile,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if guard
            .company_codex_runner_profiles
            .values()
            .any(|existing| {
                existing.company_id == profile.company_id
                    && existing.id != profile.id
                    && existing.name.eq_ignore_ascii_case(&profile.name)
            })
        {
            return Err(ai_chat_shared::AppError::Conflict(
                "Codex runner profile name already exists".into(),
            ));
        }
        guard
            .company_codex_runner_profiles
            .insert(profile.id, profile);
        Ok(())
    }

    fn get_company_codex_runner_profile(
        &self,
        profile_id: Uuid,
    ) -> Option<CompanyCodexRunnerProfile> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard
            .company_codex_runner_profiles
            .get(&profile_id)
            .cloned()
    }

    fn list_company_codex_runner_profiles(
        &self,
        company_id: Uuid,
    ) -> Vec<CompanyCodexRunnerProfile> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut profiles = guard
            .company_codex_runner_profiles
            .values()
            .filter(|profile| profile.company_id == company_id)
            .cloned()
            .collect::<Vec<_>>();
        profiles.sort_by(|left, right| {
            right
                .is_default
                .cmp(&left.is_default)
                .then_with(|| left.name.cmp(&right.name))
        });
        profiles
    }

    fn delete_company_codex_runner_profile(&self, profile_id: Uuid) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        guard.company_codex_runner_profiles.remove(&profile_id);
        Ok(())
    }

    fn clear_company_codex_runner_profile_defaults(
        &self,
        company_id: Uuid,
        except_profile_id: Uuid,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        for profile in guard.company_codex_runner_profiles.values_mut() {
            if profile.company_id == company_id && profile.id != except_profile_id {
                profile.is_default = false;
            }
        }
        Ok(())
    }

    fn assign_agent_codex_runner_profile(
        &self,
        agent_id: Uuid,
        profile_id: Uuid,
        _human_user_id: Uuid,
        _assigned_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if !guard
            .company_codex_runner_profiles
            .contains_key(&profile_id)
        {
            return Err(ai_chat_shared::AppError::NotFound(
                "Codex runner profile not found".into(),
            ));
        }
        guard
            .agent_codex_runner_profile_assignments
            .insert(agent_id, profile_id);
        Ok(())
    }

    fn get_agent_codex_runner_profile_assignment(&self, agent_id: Uuid) -> Option<Uuid> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard
            .agent_codex_runner_profile_assignments
            .get(&agent_id)
            .copied()
    }

    fn list_codex_runner_profile_agent_ids(&self, profile_id: Uuid) -> Vec<Uuid> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard
            .agent_codex_runner_profile_assignments
            .iter()
            .filter_map(|(agent_id, assigned_profile_id)| {
                (*assigned_profile_id == profile_id).then_some(*agent_id)
            })
            .collect()
    }

    fn save_agent_codex_trigger_config(&self, config: AgentCodexTriggerConfig) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if !guard.agent_profiles.contains_key(&config.agent_profile_id) {
            return Err(ai_chat_shared::AppError::NotFound("Agent not found".into()));
        }
        guard
            .agent_codex_trigger_configs
            .insert(config.agent_profile_id, config);
        Ok(())
    }

    fn get_agent_codex_trigger_config_by_agent(
        &self,
        agent_id: Uuid,
    ) -> Option<AgentCodexTriggerConfig> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard.agent_codex_trigger_configs.get(&agent_id).cloned()
    }

    fn claim_due_agent_codex_trigger_configs(
        &self,
        lease_owner: &str,
        now: chrono::DateTime<chrono::Utc>,
        limit: usize,
    ) -> AppResult<Vec<AgentCodexTriggerConfig>> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let stale_run_ids = guard
            .agent_codex_trigger_runs
            .values()
            .filter(|run| run.status == AGENT_CODEX_RUN_STATUS_RUNNING)
            .filter_map(|run| {
                let config = guard
                    .agent_codex_trigger_configs
                    .get(&run.agent_profile_id)?;
                (run.started_at + chrono::Duration::seconds(i64::from(config.max_run_seconds) + 60)
                    <= now)
                    .then_some(run.id)
            })
            .collect::<Vec<_>>();
        for run_id in stale_run_ids {
            if let Some(run) = guard.agent_codex_trigger_runs.get_mut(&run_id) {
                run.status = AGENT_CODEX_RUN_STATUS_TIMED_OUT.into();
                run.finished_at = Some(now);
                run.error_message = Some("Codex trigger run exceeded its execution lease".into());
            }
        }
        let running_agent_ids = guard
            .agent_codex_trigger_runs
            .values()
            .filter(|run| run.status == AGENT_CODEX_RUN_STATUS_RUNNING)
            .map(|run| run.agent_profile_id)
            .collect::<std::collections::HashSet<_>>();
        let logged_in_human_ids = guard
            .human_sessions
            .values()
            .filter(|session| {
                session.revoked_at.is_none()
                    && session.expires_at > now
                    && session.last_used_at.unwrap_or(session.created_at)
                        > now - chrono::Duration::seconds(60)
            })
            .map(|session| session.human_user_id)
            .collect::<std::collections::HashSet<_>>();
        let logged_in_company_ids = guard
            .company_human_members
            .values()
            .filter(|membership| {
                membership.status == "active"
                    && logged_in_human_ids.contains(&membership.human_user_id)
            })
            .map(|membership| membership.company_id)
            .collect::<std::collections::HashSet<_>>();
        let mut agent_ids = guard
            .agent_codex_trigger_configs
            .iter()
            .filter(|(_, config)| {
                config.status == AGENT_CODEX_TRIGGER_STATUS_ACTIVE
                    && config.next_run_at <= now
                    && logged_in_company_ids.contains(&config.company_id)
                    && !running_agent_ids.contains(&config.agent_profile_id)
                    && config
                        .lease_expires_at
                        .is_none_or(|lease_expires_at| lease_expires_at <= now)
            })
            .map(|(agent_id, config)| (*agent_id, config.next_run_at))
            .collect::<Vec<_>>();
        agent_ids.sort_by_key(|(_, next_run_at)| *next_run_at);
        agent_ids.truncate(limit);
        Ok(agent_ids
            .into_iter()
            .filter_map(|(agent_id, _)| {
                let config = guard.agent_codex_trigger_configs.get_mut(&agent_id)?;
                config.lease_owner = Some(lease_owner.to_string());
                config.lease_expires_at =
                    Some(now + chrono::Duration::seconds(i64::from(config.max_run_seconds) + 60));
                config.last_run_at = Some(now);
                config.updated_at = now;
                Some(config.clone())
            })
            .collect())
    }

    fn request_agent_codex_trigger_wake(
        &self,
        agent_id: Uuid,
        requested_at: chrono::DateTime<chrono::Utc>,
        reason: &str,
    ) -> AppResult<bool> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let Some(config) = guard.agent_codex_trigger_configs.get_mut(&agent_id) else {
            return Ok(false);
        };
        if config.status != AGENT_CODEX_TRIGGER_STATUS_ACTIVE {
            return Ok(false);
        }
        config.next_run_at = config.next_run_at.min(requested_at);
        config.wake_requested_at = Some(
            config
                .wake_requested_at
                .map_or(requested_at, |existing| existing.max(requested_at)),
        );
        config.wake_reason = Some(reason.to_string());
        config.updated_at = requested_at;
        Ok(true)
    }

    fn complete_agent_codex_trigger_lease(
        &self,
        input: CompleteAgentCodexTriggerLeaseInput,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let config = guard
            .agent_codex_trigger_configs
            .values_mut()
            .find(|config| {
                config.id == input.trigger_config_id
                    && config.lease_owner.as_deref() == Some(input.lease_owner.as_str())
            })
            .ok_or_else(|| {
                ai_chat_shared::AppError::Conflict("Codex trigger lease was lost".into())
            })?;
        let has_new_wake = config
            .wake_requested_at
            .zip(config.last_run_at)
            .is_some_and(|(wake_requested_at, claimed_at)| wake_requested_at > claimed_at);
        let has_new_manual = config
            .manual_run_requested_at
            .zip(config.last_run_at)
            .is_some_and(|(requested_at, claimed_at)| requested_at > claimed_at);
        config.lease_owner = None;
        config.lease_expires_at = None;
        if has_new_wake || has_new_manual {
            config.next_run_at = [
                has_new_wake.then_some(config.wake_requested_at).flatten(),
                has_new_manual
                    .then_some(config.manual_run_requested_at)
                    .flatten(),
                Some(input.next_run_at),
            ]
            .into_iter()
            .flatten()
            .min()
            .unwrap_or(input.next_run_at);
        } else {
            config.next_run_at = input.next_run_at;
            config.wake_requested_at = None;
            config.wake_reason = None;
        }
        if !has_new_manual {
            config.manual_run_requested_at = None;
        }
        config.last_run_at = Some(input.finished_at);
        config.updated_at = input.finished_at;
        if input.succeeded {
            config.last_success_at = Some(input.finished_at);
            config.last_error = None;
            config.consecutive_failure_count = 0;
        } else {
            config.last_error = input.error_message;
            config.consecutive_failure_count += 1;
            if config.consecutive_failure_count >= 3
                && config.status == AGENT_CODEX_TRIGGER_STATUS_ACTIVE
            {
                config.status = AGENT_CODEX_TRIGGER_STATUS_ERROR.into();
            }
        }
        Ok(())
    }

    fn insert_agent_codex_trigger_run(&self, run: AgentCodexTriggerRun) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if guard.agent_codex_trigger_runs.contains_key(&run.id) {
            return Err(ai_chat_shared::AppError::Conflict(
                "Codex trigger run already exists".into(),
            ));
        }
        if run.status == AGENT_CODEX_RUN_STATUS_RUNNING
            && guard.agent_codex_trigger_runs.values().any(|existing| {
                existing.agent_profile_id == run.agent_profile_id
                    && existing.status == AGENT_CODEX_RUN_STATUS_RUNNING
            })
        {
            return Err(ai_chat_shared::AppError::Conflict(
                "Agent already has a running Codex trigger run".into(),
            ));
        }
        guard.agent_codex_trigger_runs.insert(run.id, run);
        Ok(())
    }

    fn has_running_agent_codex_trigger_run(&self, agent_id: Uuid) -> bool {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard.agent_codex_trigger_runs.values().any(|run| {
            run.agent_profile_id == agent_id && run.status == AGENT_CODEX_RUN_STATUS_RUNNING
        })
    }

    fn update_agent_codex_trigger_run(&self, mut run: AgentCodexTriggerRun) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let Some(existing) = guard.agent_codex_trigger_runs.get(&run.id) else {
            return Err(ai_chat_shared::AppError::NotFound(
                "Codex trigger run not found".into(),
            ));
        };
        run.activity_phase = existing.activity_phase.clone();
        run.activity_summary = existing.activity_summary.clone();
        run.last_activity_at = existing.last_activity_at;
        run.activity_log = existing.activity_log.clone();
        guard.agent_codex_trigger_runs.insert(run.id, run);
        Ok(())
    }

    fn append_agent_codex_trigger_run_activity(
        &self,
        run_id: Uuid,
        activity: AgentCodexRunActivity,
        codex_thread_id: Option<String>,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let run = guard
            .agent_codex_trigger_runs
            .get_mut(&run_id)
            .ok_or_else(|| {
                ai_chat_shared::AppError::NotFound("Codex trigger run not found".into())
            })?;
        run.activity_phase = activity.phase.clone();
        run.activity_summary = Some(activity.summary.clone());
        run.last_activity_at = Some(activity.at);
        if codex_thread_id.is_some() {
            run.codex_thread_id = codex_thread_id;
        }
        run.activity_log.push(activity);
        if run.activity_log.len() > 40 {
            let excess = run.activity_log.len() - 40;
            run.activity_log.drain(0..excess);
        }
        Ok(())
    }

    fn list_agent_codex_trigger_runs(
        &self,
        agent_id: Uuid,
        limit: usize,
    ) -> Vec<AgentCodexTriggerRun> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut runs = guard
            .agent_codex_trigger_runs
            .values()
            .filter(|run| run.agent_profile_id == agent_id)
            .cloned()
            .collect::<Vec<_>>();
        runs.sort_by(|left, right| right.started_at.cmp(&left.started_at));
        runs.truncate(limit);
        runs
    }

    fn save_agent_codex_session(&self, session: AgentCodexSession) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        guard
            .agent_codex_sessions
            .insert(session.agent_profile_id, session);
        Ok(())
    }

    fn get_agent_codex_session(&self, agent_id: Uuid) -> Option<AgentCodexSession> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard.agent_codex_sessions.get(&agent_id).cloned()
    }

    fn insert_agent_codex_run_token(&self, token: AgentCodexRunToken) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if guard.agent_codex_run_tokens.contains_key(&token.token_hash) {
            return Err(ai_chat_shared::AppError::Conflict(
                "Codex run token already exists".into(),
            ));
        }
        guard
            .agent_codex_run_tokens
            .insert(token.token_hash.clone(), token);
        Ok(())
    }

    fn find_agent_codex_run_token_by_hash(&self, token_hash: &str) -> Option<AgentCodexRunToken> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard.agent_codex_run_tokens.get(token_hash).cloned()
    }

    fn revoke_agent_codex_run_tokens(
        &self,
        run_id: Uuid,
        revoked_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        for token in guard.agent_codex_run_tokens.values_mut() {
            if token.run_id == run_id && token.revoked_at.is_none() {
                token.revoked_at = Some(revoked_at);
            }
        }
        Ok(())
    }

    fn delete_expired_agent_codex_run_tokens(
        &self,
        now: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<usize> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let before = guard.agent_codex_run_tokens.len();
        guard.agent_codex_run_tokens.retain(|_, token| {
            token.expires_at > now && token.revoked_at.is_none_or(|revoked_at| revoked_at > now)
        });
        Ok(before - guard.agent_codex_run_tokens.len())
    }

    fn update_company_project(&self, project: CompanyProject) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if !guard.company_projects.contains_key(&project.id) {
            return Err(ai_chat_shared::AppError::NotFound(
                "company project not found".into(),
            ));
        }
        guard.company_projects.insert(project.id, project);
        Ok(())
    }

    fn update_company_project_metadata(
        &self,
        project: CompanyProject,
        project_group_title: String,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if !guard.company_projects.contains_key(&project.id) {
            return Err(ai_chat_shared::AppError::NotFound(
                "company project not found".into(),
            ));
        }
        for previews in guard.conversations.values_mut() {
            if let Some(preview) = previews
                .iter_mut()
                .find(|preview| preview.id == project.project_group_conversation_id)
            {
                preview.title = project_group_title.clone();
                preview.updated_at = project.updated_at;
            }
        }
        guard.company_projects.insert(project.id, project);
        Ok(())
    }

    fn get_company_project_member(
        &self,
        project_id: Uuid,
        agent_id: Uuid,
    ) -> Option<CompanyProjectMember> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard
            .company_project_members
            .get(&(project_id, agent_id))
            .cloned()
    }

    fn list_company_project_members(&self, project_id: Uuid) -> Vec<CompanyProjectMember> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut members = guard
            .company_project_members
            .values()
            .filter(|member| member.project_id == project_id)
            .cloned()
            .collect::<Vec<_>>();
        members.sort_by(|left, right| left.joined_at.cmp(&right.joined_at));
        members
    }

    fn complete_company_project_member_add(
        &self,
        bundle: CompanyProjectMemberAddBundle,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if !guard
            .company_projects
            .contains_key(&bundle.member.project_id)
        {
            return Err(ai_chat_shared::AppError::NotFound(
                "company project not found".into(),
            ));
        }
        let project_id = bundle.member.project_id;
        let joined_at = bundle.member.joined_at;
        guard
            .company_project_members
            .insert((project_id, bundle.member.agent_profile_id), bundle.member);
        let previews = guard
            .conversations
            .entry(bundle.conversation_preview.agent_id)
            .or_default();
        previews.retain(|preview| preview.id != bundle.conversation_preview.preview.id);
        previews.push(bundle.conversation_preview.preview);
        if let Some(project) = guard.company_projects.get_mut(&project_id) {
            project.updated_at = joined_at;
        }
        Ok(())
    }

    fn complete_company_project_member_remove(
        &self,
        project_id: Uuid,
        agent_id: Uuid,
        conversation_id: Uuid,
        left_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let member = guard
            .company_project_members
            .get_mut(&(project_id, agent_id))
            .ok_or_else(|| ai_chat_shared::AppError::NotFound("project member not found".into()))?;
        member.left_at = Some(left_at);
        if let Some(previews) = guard.conversations.get_mut(&agent_id) {
            previews.retain(|preview| preview.id != conversation_id);
        }
        if let Some(project) = guard.company_projects.get_mut(&project_id) {
            project.updated_at = left_at;
        }
        Ok(())
    }

    fn insert_company_project_task(&self, task: CompanyProjectTask) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if guard.company_project_tasks.contains_key(&task.id) {
            return Err(ai_chat_shared::AppError::Conflict(
                "company project task already exists".into(),
            ));
        }
        if let Some(project) = guard.company_projects.get_mut(&task.project_id) {
            project.updated_at = task.updated_at;
            project.updated_by_agent_id = task.created_by_agent_id;
        }
        guard
            .company_project_task_status_history
            .push(task_status_history_entry(None, &task));
        guard.company_project_tasks.insert(task.id, task);
        Ok(())
    }

    fn get_company_project_task(&self, task_id: Uuid) -> Option<CompanyProjectTask> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard.company_project_tasks.get(&task_id).cloned()
    }

    fn list_company_project_tasks(&self, project_id: Uuid) -> Vec<CompanyProjectTask> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut tasks = guard
            .company_project_tasks
            .values()
            .filter(|task| task.project_id == project_id)
            .cloned()
            .collect::<Vec<_>>();
        tasks.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        tasks
    }

    fn update_company_project_task(&self, task: CompanyProjectTask) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let Some(previous) = guard.company_project_tasks.get(&task.id).cloned() else {
            return Err(ai_chat_shared::AppError::NotFound(
                "company project task not found".into(),
            ));
        };
        if let Some(project) = guard.company_projects.get_mut(&task.project_id) {
            project.updated_at = task.updated_at;
            project.updated_by_agent_id = task.updated_by_agent_id;
        }
        if previous.status != task.status {
            guard
                .company_project_task_status_history
                .push(task_status_history_entry(Some(previous.status), &task));
        }
        guard.company_project_tasks.insert(task.id, task);
        Ok(())
    }

    fn update_company_project_tasks(&self, tasks: Vec<CompanyProjectTask>) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if tasks
            .iter()
            .any(|task| !guard.company_project_tasks.contains_key(&task.id))
        {
            return Err(ai_chat_shared::AppError::NotFound(
                "company project task not found".into(),
            ));
        }
        for task in tasks {
            let previous_status = guard
                .company_project_tasks
                .get(&task.id)
                .map(|previous| previous.status.clone());
            if let Some(project) = guard.company_projects.get_mut(&task.project_id) {
                project.updated_at = task.updated_at;
                project.updated_by_agent_id = task.updated_by_agent_id;
            }
            if previous_status.as_deref() != Some(task.status.as_str()) {
                guard
                    .company_project_task_status_history
                    .push(task_status_history_entry(previous_status, &task));
            }
            guard.company_project_tasks.insert(task.id, task);
        }
        Ok(())
    }

    fn list_company_project_task_status_history(
        &self,
        project_id: Uuid,
    ) -> Vec<CompanyProjectTaskStatusHistory> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut history = guard
            .company_project_task_status_history
            .iter()
            .filter(|entry| entry.project_id == project_id)
            .cloned()
            .collect::<Vec<_>>();
        history.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        history
    }

    fn insert_company_project_task_dependency(
        &self,
        dependency: CompanyProjectTaskDependency,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if guard
            .company_project_task_dependencies
            .values()
            .any(|existing| {
                existing.task_id == dependency.task_id
                    && existing.depends_on_task_id == dependency.depends_on_task_id
            })
        {
            return Err(ai_chat_shared::AppError::Conflict(
                "company project task dependency already exists".into(),
            ));
        }
        guard
            .company_project_task_dependencies
            .insert(dependency.id, dependency);
        Ok(())
    }

    fn remove_company_project_task_dependency(
        &self,
        project_id: Uuid,
        task_id: Uuid,
        depends_on_task_id: Uuid,
        _removed_by_agent_id: Option<Uuid>,
        _removed_by_human_user_id: Option<Uuid>,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let dependency_id = guard
            .company_project_task_dependencies
            .values()
            .find(|dependency| {
                dependency.project_id == project_id
                    && dependency.task_id == task_id
                    && dependency.depends_on_task_id == depends_on_task_id
            })
            .map(|dependency| dependency.id)
            .ok_or_else(|| {
                ai_chat_shared::AppError::NotFound(
                    "company project task dependency not found".into(),
                )
            })?;
        guard
            .company_project_task_dependencies
            .remove(&dependency_id);
        Ok(())
    }

    fn list_company_project_task_dependencies(
        &self,
        project_id: Uuid,
    ) -> Vec<CompanyProjectTaskDependency> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut dependencies = guard
            .company_project_task_dependencies
            .values()
            .filter(|dependency| dependency.project_id == project_id)
            .cloned()
            .collect::<Vec<_>>();
        dependencies.sort_by(|left, right| {
            left.created_at
                .cmp(&right.created_at)
                .then_with(|| left.id.cmp(&right.id))
        });
        dependencies
    }

    fn insert_company_project_status_update(
        &self,
        update: CompanyProjectStatusUpdate,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        guard
            .company_project_status_updates
            .entry(update.project_id)
            .or_default()
            .push(update);
        Ok(())
    }

    fn list_company_project_status_updates(
        &self,
        project_id: Uuid,
    ) -> Vec<CompanyProjectStatusUpdate> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut updates = guard
            .company_project_status_updates
            .get(&project_id)
            .cloned()
            .unwrap_or_default();
        updates.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        updates
    }

    fn save_agent_runtime_config(&self, config: AgentRuntimeConfig) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if guard.agent_runtime_configs.values().any(|existing| {
            existing.agent_profile_id == config.agent_profile_id && existing.id != config.id
        }) {
            return Err(ai_chat_shared::AppError::Conflict(
                "agent runtime is already configured".into(),
            ));
        }
        guard.agent_runtime_configs.insert(config.id, config);
        Ok(())
    }

    fn get_agent_runtime_config(&self, runtime_config_id: Uuid) -> Option<AgentRuntimeConfig> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard.agent_runtime_configs.get(&runtime_config_id).cloned()
    }

    fn get_agent_runtime_config_by_agent(&self, agent_id: Uuid) -> Option<AgentRuntimeConfig> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard
            .agent_runtime_configs
            .values()
            .find(|config| config.agent_profile_id == agent_id)
            .cloned()
    }

    fn list_company_agent_runtime_configs(&self, company_id: Uuid) -> Vec<AgentRuntimeConfig> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut configs = guard
            .agent_runtime_configs
            .values()
            .filter(|config| config.company_id == company_id)
            .cloned()
            .collect::<Vec<_>>();
        configs.sort_by(|left, right| left.created_at.cmp(&right.created_at));
        configs
    }

    fn save_agent_runtime_template(&self, template: AgentRuntimeTemplate) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if template.status == ai_chat_domain::company::AGENT_RUNTIME_TEMPLATE_STATUS_ACTIVE
            && guard.agent_runtime_templates.values().any(|existing| {
                existing.id != template.id
                    && existing.company_id == template.company_id
                    && existing.status
                        == ai_chat_domain::company::AGENT_RUNTIME_TEMPLATE_STATUS_ACTIVE
                    && existing.name.eq_ignore_ascii_case(&template.name)
            })
        {
            return Err(ai_chat_shared::AppError::Conflict(
                "an active runtime template with this name already exists".into(),
            ));
        }
        guard.agent_runtime_templates.insert(template.id, template);
        Ok(())
    }

    fn get_agent_runtime_template(&self, template_id: Uuid) -> Option<AgentRuntimeTemplate> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard.agent_runtime_templates.get(&template_id).cloned()
    }

    fn list_company_agent_runtime_templates(&self, company_id: Uuid) -> Vec<AgentRuntimeTemplate> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut templates = guard
            .agent_runtime_templates
            .values()
            .filter(|template| template.company_id == company_id)
            .cloned()
            .collect::<Vec<_>>();
        templates.sort_by(|left, right| {
            right
                .updated_at
                .cmp(&left.updated_at)
                .then_with(|| left.name.cmp(&right.name))
        });
        templates
    }

    fn save_company_runtime_policy(&self, policy: CompanyRuntimePolicy) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        guard
            .company_runtime_policies
            .insert(policy.company_id, policy);
        Ok(())
    }

    fn get_company_runtime_policy(&self, company_id: Uuid) -> Option<CompanyRuntimePolicy> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard.company_runtime_policies.get(&company_id).cloned()
    }

    fn list_due_agent_runtime_configs(
        &self,
        now: chrono::DateTime<chrono::Utc>,
        limit: usize,
    ) -> Vec<AgentRuntimeConfig> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut configs = guard
            .agent_runtime_configs
            .values()
            .filter(|config| {
                config.status == AGENT_RUNTIME_STATUS_ACTIVE && config.next_run_at <= now
            })
            .cloned()
            .collect::<Vec<_>>();
        configs.sort_by(|left, right| left.next_run_at.cmp(&right.next_run_at));
        configs.truncate(limit);
        configs
    }

    fn claim_agent_runtime_config(
        &self,
        runtime_config_id: Uuid,
        now: chrono::DateTime<chrono::Utc>,
        next_run_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<bool> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let Some(config) = guard.agent_runtime_configs.get_mut(&runtime_config_id) else {
            return Ok(false);
        };
        if config.status != AGENT_RUNTIME_STATUS_ACTIVE || config.next_run_at > now {
            return Ok(false);
        }
        config.next_run_at = next_run_at;
        config.updated_at = now;
        Ok(true)
    }

    fn insert_agent_runtime_run(&self, run: AgentRuntimeRun) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if guard.agent_runtime_runs.contains_key(&run.id) {
            return Err(ai_chat_shared::AppError::Conflict(
                "agent runtime run already exists".into(),
            ));
        }
        guard.agent_runtime_runs.insert(run.id, run);
        Ok(())
    }

    fn update_agent_runtime_run(&self, run: AgentRuntimeRun) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if !guard.agent_runtime_runs.contains_key(&run.id) {
            return Err(ai_chat_shared::AppError::NotFound(
                "agent runtime run not found".into(),
            ));
        }
        guard.agent_runtime_runs.insert(run.id, run);
        Ok(())
    }

    fn list_agent_runtime_runs(
        &self,
        runtime_config_id: Uuid,
        limit: usize,
    ) -> Vec<AgentRuntimeRun> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut runs = guard
            .agent_runtime_runs
            .values()
            .filter(|run| run.runtime_config_id == runtime_config_id)
            .cloned()
            .collect::<Vec<_>>();
        runs.sort_by(|left, right| right.started_at.cmp(&left.started_at));
        runs.truncate(limit);
        runs
    }

    fn get_agent_runtime_daily_usage(
        &self,
        runtime_config_id: Uuid,
        window_start: chrono::DateTime<chrono::Utc>,
    ) -> AgentRuntimeDailyUsage {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let runs = guard.agent_runtime_runs.values().filter(|run| {
            run.runtime_config_id == runtime_config_id
                && run.started_at >= window_start
                && run.status != AGENT_RUNTIME_RUN_STATUS_SKIPPED_BUDGET
        });
        let mut usage = AgentRuntimeDailyUsage {
            window_start,
            run_count: 0,
            action_count: 0,
            approval_request_count: 0,
            failed_run_count: 0,
            model_request_count: 0,
            model_input_tokens: 0,
            model_output_tokens: 0,
            model_cost_microusd: 0,
        };
        for run in runs {
            usage.run_count += 1;
            usage.action_count += i64::from(run.action_count);
            usage.approval_request_count += i64::from(run.approval_request_count);
            usage.model_request_count += i64::from(run.model_request_count);
            usage.model_input_tokens += run.model_input_tokens;
            usage.model_output_tokens += run.model_output_tokens;
            usage.model_cost_microusd += run.model_cost_microusd;
            if run.status == AGENT_RUNTIME_RUN_STATUS_FAILED {
                usage.failed_run_count += 1;
            }
        }
        usage
    }

    fn save_company_model_budget_policy(&self, policy: CompanyModelBudgetPolicy) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        guard
            .company_model_budget_policies
            .insert(policy.company_id, policy);
        Ok(())
    }

    fn get_company_model_budget_policy(
        &self,
        company_id: Uuid,
    ) -> Option<CompanyModelBudgetPolicy> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard
            .company_model_budget_policies
            .get(&company_id)
            .cloned()
    }

    fn publish_agent_model_price_catalog_entry(
        &self,
        mut entry: AgentModelPriceCatalogEntry,
    ) -> AppResult<AgentModelPriceCatalogEntry> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let next_version = guard
            .agent_model_price_catalog_entries
            .values()
            .filter(|existing| {
                existing.company_id == entry.company_id
                    && existing.model_provider == entry.model_provider
                    && existing.model_name == entry.model_name
            })
            .map(|existing| existing.version)
            .max()
            .unwrap_or(0)
            + 1;
        for existing in guard.agent_model_price_catalog_entries.values_mut() {
            if existing.company_id == entry.company_id
                && existing.model_provider == entry.model_provider
                && existing.model_name == entry.model_name
                && existing.status == AGENT_MODEL_PRICE_CATALOG_STATUS_ACTIVE
            {
                existing.status = AGENT_MODEL_PRICE_CATALOG_STATUS_ARCHIVED.into();
                existing.updated_by_human_user_id = entry.updated_by_human_user_id;
                existing.updated_at = entry.updated_at;
            }
        }
        entry.version = next_version;
        guard
            .agent_model_price_catalog_entries
            .insert(entry.id, entry.clone());
        Ok(entry)
    }

    fn save_agent_model_price_catalog_entry(
        &self,
        entry: AgentModelPriceCatalogEntry,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if !guard
            .agent_model_price_catalog_entries
            .contains_key(&entry.id)
        {
            return Err(ai_chat_shared::AppError::NotFound(
                "model price catalog entry not found".into(),
            ));
        }
        guard
            .agent_model_price_catalog_entries
            .insert(entry.id, entry);
        Ok(())
    }

    fn get_agent_model_price_catalog_entry(
        &self,
        entry_id: Uuid,
    ) -> Option<AgentModelPriceCatalogEntry> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard
            .agent_model_price_catalog_entries
            .get(&entry_id)
            .cloned()
    }

    fn list_company_agent_model_price_catalog_entries(
        &self,
        company_id: Uuid,
    ) -> Vec<AgentModelPriceCatalogEntry> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut entries = guard
            .agent_model_price_catalog_entries
            .values()
            .filter(|entry| entry.company_id == company_id)
            .cloned()
            .collect::<Vec<_>>();
        entries.sort_by(|left, right| {
            left.model_provider
                .cmp(&right.model_provider)
                .then_with(|| left.model_name.cmp(&right.model_name))
                .then_with(|| right.version.cmp(&left.version))
        });
        entries
    }

    fn publish_company_governance_policy_version(
        &self,
        mut version: CompanyGovernancePolicyVersion,
    ) -> AppResult<CompanyGovernancePolicyVersion> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if let Some(active) = guard
            .company_governance_policy_versions
            .values()
            .find(|existing| {
                existing.company_id == version.company_id
                    && existing.status == COMPANY_GOVERNANCE_POLICY_STATUS_ACTIVE
                    && existing.settings == version.settings
            })
            .cloned()
        {
            return Ok(active);
        }
        let next_version = guard
            .company_governance_policy_versions
            .values()
            .filter(|existing| existing.company_id == version.company_id)
            .map(|existing| existing.version)
            .max()
            .unwrap_or(0)
            + 1;
        for existing in guard.company_governance_policy_versions.values_mut() {
            if existing.company_id == version.company_id
                && existing.status == COMPANY_GOVERNANCE_POLICY_STATUS_ACTIVE
            {
                existing.status = COMPANY_GOVERNANCE_POLICY_STATUS_ARCHIVED.into();
                existing.updated_by_human_user_id = version.updated_by_human_user_id;
                existing.updated_at = version.updated_at;
            }
        }
        version.version = next_version;
        guard
            .company_governance_policy_versions
            .insert(version.id, version.clone());
        Ok(version)
    }

    fn get_active_company_governance_policy_version(
        &self,
        company_id: Uuid,
    ) -> Option<CompanyGovernancePolicyVersion> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard
            .company_governance_policy_versions
            .values()
            .find(|version| {
                version.company_id == company_id
                    && version.status == COMPANY_GOVERNANCE_POLICY_STATUS_ACTIVE
            })
            .cloned()
    }

    fn list_company_governance_policy_versions(
        &self,
        company_id: Uuid,
    ) -> Vec<CompanyGovernancePolicyVersion> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut versions = guard
            .company_governance_policy_versions
            .values()
            .filter(|version| version.company_id == company_id)
            .cloned()
            .collect::<Vec<_>>();
        versions.sort_by(|left, right| right.version.cmp(&left.version));
        versions
    }

    fn get_company_model_daily_usage(
        &self,
        company_id: Uuid,
        window_start: chrono::DateTime<chrono::Utc>,
    ) -> CompanyModelDailyUsage {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut usage = CompanyModelDailyUsage {
            window_start,
            model_request_count: 0,
            model_input_tokens: 0,
            model_output_tokens: 0,
            model_cost_microusd: 0,
        };
        for run in guard.agent_runtime_runs.values().filter(|run| {
            run.company_id == company_id
                && run.started_at >= window_start
                && run.status != AGENT_RUNTIME_RUN_STATUS_SKIPPED_BUDGET
        }) {
            usage.model_request_count += i64::from(run.model_request_count);
            usage.model_input_tokens += run.model_input_tokens;
            usage.model_output_tokens += run.model_output_tokens;
            usage.model_cost_microusd += run.model_cost_microusd;
        }
        usage
    }

    fn insert_agent_tool_approval_request(
        &self,
        request: AgentToolApprovalRequest,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if guard.agent_tool_approval_requests.contains_key(&request.id) {
            return Err(ai_chat_shared::AppError::Conflict(
                "agent tool approval request already exists".into(),
            ));
        }
        guard
            .agent_tool_approval_requests
            .insert(request.id, request);
        Ok(())
    }

    fn get_agent_tool_approval_request(
        &self,
        approval_request_id: Uuid,
    ) -> Option<AgentToolApprovalRequest> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard
            .agent_tool_approval_requests
            .get(&approval_request_id)
            .cloned()
    }

    fn list_company_agent_tool_approval_requests(
        &self,
        company_id: Uuid,
        status: Option<&str>,
        limit: usize,
    ) -> Vec<AgentToolApprovalRequest> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut requests = guard
            .agent_tool_approval_requests
            .values()
            .filter(|request| {
                request.company_id == company_id
                    && status.is_none_or(|status| request.status == status)
            })
            .cloned()
            .collect::<Vec<_>>();
        requests.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        requests.truncate(limit);
        requests
    }

    fn claim_agent_tool_approval_request(
        &self,
        approval_request_id: Uuid,
        human_user_id: Uuid,
        status: &str,
        review_note: &str,
        now: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<Option<AgentToolApprovalRequest>> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let Some(request) = guard
            .agent_tool_approval_requests
            .get_mut(&approval_request_id)
        else {
            return Ok(None);
        };
        if request.status != AGENT_TOOL_APPROVAL_STATUS_PENDING || request.expires_at <= now {
            return Ok(None);
        }
        request.status = status.to_string();
        request.reviewed_by_human_user_id = Some(human_user_id);
        request.review_note = review_note.to_string();
        request.reviewed_at = Some(now);
        request.updated_at = now;
        Ok(Some(request.clone()))
    }

    fn update_agent_tool_approval_request(
        &self,
        request: AgentToolApprovalRequest,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if !guard.agent_tool_approval_requests.contains_key(&request.id) {
            return Err(ai_chat_shared::AppError::NotFound(
                "agent tool approval request not found".into(),
            ));
        }
        guard
            .agent_tool_approval_requests
            .insert(request.id, request);
        Ok(())
    }

    fn count_agent_tool_approval_requests_since(
        &self,
        agent_id: Uuid,
        since: chrono::DateTime<chrono::Utc>,
    ) -> usize {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard
            .agent_tool_approval_requests
            .values()
            .filter(|request| {
                request.requested_by_agent_id == agent_id && request.created_at >= since
            })
            .count()
    }

    fn list_owner_agents(&self, human_user_id: Uuid) -> Vec<AgentProfile> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard
            .agent_profiles
            .values()
            .filter(|agent| agent.owner_user_id == human_user_id)
            .cloned()
            .collect()
    }

    fn agent_exists(&self, agent_id: Uuid) -> bool {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard.agent_profiles.contains_key(&agent_id)
    }

    fn get_agent_profile(&self, agent_id: Uuid) -> Option<AgentProfile> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard.agent_profiles.get(&agent_id).cloned()
    }

    fn list_agent_conversations(&self, agent_id: Uuid) -> Vec<ConversationPreview> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard
            .conversations
            .get(&agent_id)
            .cloned()
            .unwrap_or_default()
    }

    fn list_company_conversations(&self, company_id: Uuid) -> Vec<ConversationPreview> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut conversations_by_id = HashMap::new();
        for previews in guard.conversations.values() {
            for preview in previews {
                if guard
                    .conversation_contexts
                    .get(&preview.id)
                    .is_some_and(|context| context.company_id == Some(company_id))
                {
                    conversations_by_id.insert(preview.id, preview.clone());
                }
            }
        }
        if let Some(default_group) = guard.company_default_groups.get(&company_id) {
            conversations_by_id
                .entry(default_group.id)
                .or_insert_with(|| default_group.clone());
        }
        let mut conversations = conversations_by_id.into_values().collect::<Vec<_>>();
        conversations.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        conversations
    }

    fn get_conversation_messages(&self, conversation_id: Uuid) -> Vec<MessageView> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard
            .messages
            .get(&conversation_id)
            .cloned()
            .unwrap_or_default()
    }

    fn append_message(&self, message: MessageView) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        guard
            .messages
            .entry(message.conversation_id)
            .or_default()
            .push(message.clone());

        for previews in guard.conversations.values_mut() {
            for preview in previews.iter_mut() {
                if preview.id == message.conversation_id {
                    preview.last_message_preview = Some(message.content.clone());
                    preview.updated_at = message.created_at;
                }
            }
        }
        Ok(())
    }

    fn insert_post(&self, post: PostView) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        guard.posts.insert(post.id, post);
        Ok(())
    }

    fn insert_post_comment(&self, comment: PostCommentView) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        guard
            .post_comments
            .entry(comment.post_id)
            .or_default()
            .push(comment);
        Ok(())
    }

    fn insert_diary_entry(&self, diary_entry: DiaryEntryView) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        guard.diary_entries.insert(diary_entry.id, diary_entry);
        Ok(())
    }

    fn list_posts(&self) -> Vec<PostView> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard.posts.values().cloned().collect()
    }

    fn list_post_comments(&self, post_id: Uuid) -> Vec<PostCommentView> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard
            .post_comments
            .get(&post_id)
            .cloned()
            .unwrap_or_default()
    }

    fn list_diary_entries(&self) -> Vec<DiaryEntryView> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut entries = guard.diary_entries.values().cloned().collect::<Vec<_>>();
        entries.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        entries
    }

    fn insert_friend_request(&self, request: FriendRequestView) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        guard.friend_requests.insert(request.id, request);
        Ok(())
    }

    fn list_friend_requests(&self, agent_id: Uuid) -> Vec<FriendRequestView> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard
            .friend_requests
            .values()
            .filter(|item| item.requester_agent_id == agent_id || item.target_agent_id == agent_id)
            .cloned()
            .collect()
    }

    fn list_all_friend_requests(&self) -> Vec<FriendRequestView> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut requests = guard.friend_requests.values().cloned().collect::<Vec<_>>();
        requests.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        requests
    }

    fn list_friends(&self, agent_id: Uuid) -> Vec<FriendSummary> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard
            .friendships
            .iter()
            .filter_map(|(left, right)| {
                let friend_id = if *left == agent_id {
                    *right
                } else if *right == agent_id {
                    *left
                } else {
                    return None;
                };
                guard
                    .agent_profiles
                    .get(&friend_id)
                    .map(|profile| FriendSummary {
                        agent_id: profile.id,
                        display_name: profile.display_name.clone(),
                        handle: profile.handle.clone(),
                    })
            })
            .collect()
    }

    fn get_friend_request(&self, request_id: Uuid) -> Option<FriendRequestView> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard.friend_requests.get(&request_id).cloned()
    }

    fn update_friend_request(&self, request: FriendRequestView) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        guard.friend_requests.insert(request.id, request);
        Ok(())
    }

    fn link_friends(&self, left_agent_id: Uuid, right_agent_id: Uuid) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let pair = ordered_pair(left_agent_id, right_agent_id);
        if !guard.friendships.contains(&pair) {
            guard.friendships.push(pair);
        }
        Ok(())
    }

    fn get_friend_profile(
        &self,
        owner_agent_id: Uuid,
        friend_agent_id: Uuid,
    ) -> Option<FriendProfileSnapshot> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard
            .friend_profiles
            .get(&(owner_agent_id, friend_agent_id))
            .cloned()
    }

    fn upsert_friend_profile(&self, profile: FriendProfileSnapshot) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        guard
            .friend_profiles
            .insert((profile.owner_agent_id, profile.friend_agent_id), profile);
        Ok(())
    }

    fn find_direct_conversation(
        &self,
        left_agent_id: Uuid,
        right_agent_id: Uuid,
    ) -> Option<ConversationPreview> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let pair = ordered_pair(left_agent_id, right_agent_id);
        let conversation_id = guard.direct_conversations.get(&pair)?;
        guard
            .conversations
            .get(&left_agent_id)
            .and_then(|items| items.iter().find(|item| item.id == *conversation_id))
            .cloned()
    }

    fn find_direct_conversation_peer(&self, conversation_id: Uuid, agent_id: Uuid) -> Option<Uuid> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let owners = guard
            .conversations
            .iter()
            .filter_map(|(owner_id, items)| {
                items
                    .iter()
                    .any(|item| item.id == conversation_id && item.conversation_type.is_direct())
                    .then_some(*owner_id)
            })
            .collect::<Vec<_>>();

        if owners.len() != 2 || !owners.contains(&agent_id) {
            return None;
        }

        owners.into_iter().find(|owner_id| *owner_id != agent_id)
    }

    fn conversation_exists(&self, conversation_id: Uuid) -> bool {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard
            .conversations
            .values()
            .any(|items| items.iter().any(|item| item.id == conversation_id))
    }

    fn list_agent_action_logs(&self, agent_id: Uuid, limit: usize) -> Vec<AgentActionLog> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut logs = guard
            .action_logs
            .iter()
            .filter(|item| item.agent_profile_id == agent_id)
            .cloned()
            .collect::<Vec<_>>();
        logs.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        logs.truncate(limit);
        logs
    }

    fn insert_problem_workspace(&self, workspace: ProblemWorkspace) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        guard.problem_workspaces.insert(workspace.id, workspace);
        Ok(())
    }

    fn update_problem_workspace(&self, workspace: ProblemWorkspace) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        guard.problem_workspaces.insert(workspace.id, workspace);
        Ok(())
    }

    fn get_problem_workspace(&self, workspace_id: Uuid) -> Option<ProblemWorkspace> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard.problem_workspaces.get(&workspace_id).cloned()
    }

    fn list_problem_workspaces(&self, agent_id: Uuid, limit: usize) -> Vec<ProblemWorkspace> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut workspaces = guard
            .problem_workspaces
            .values()
            .filter(|workspace| {
                workspace.owner_agent_id == agent_id
                    || workspace.participant_agent_ids.contains(&agent_id)
            })
            .cloned()
            .collect::<Vec<_>>();
        workspaces.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        workspaces.truncate(limit);
        workspaces
    }

    fn insert_problem_workspace_progress_record(
        &self,
        record: ProblemWorkspaceProgressRecord,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if let Some(workspace) = guard.problem_workspaces.get_mut(&record.workspace_id) {
            workspace.updated_at = record.created_at;
        }
        guard
            .problem_workspace_progress_records
            .entry(record.workspace_id)
            .or_default()
            .push(record);
        Ok(())
    }

    fn get_problem_workspace_progress_record(
        &self,
        record_id: Uuid,
    ) -> Option<ProblemWorkspaceProgressRecord> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard
            .problem_workspace_progress_records
            .values()
            .flat_map(|records| records.iter())
            .find(|record| record.id == record_id)
            .cloned()
    }

    fn update_problem_workspace_progress_record(
        &self,
        record: ProblemWorkspaceProgressRecord,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let records = guard
            .problem_workspace_progress_records
            .entry(record.workspace_id)
            .or_default();
        if let Some(existing) = records.iter_mut().find(|item| item.id == record.id) {
            *existing = record;
        }
        Ok(())
    }

    fn list_problem_workspace_progress_records(
        &self,
        workspace_id: Uuid,
        limit: usize,
    ) -> Vec<ProblemWorkspaceProgressRecord> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut records = guard
            .problem_workspace_progress_records
            .get(&workspace_id)
            .cloned()
            .unwrap_or_default();
        records.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        records.truncate(limit);
        records
    }

    fn insert_problem_workspace_invitation(
        &self,
        invitation: ProblemWorkspaceInvitation,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        guard
            .problem_workspace_invitations
            .insert(invitation.id, invitation);
        Ok(())
    }

    fn get_problem_workspace_invitation(
        &self,
        invitation_id: Uuid,
    ) -> Option<ProblemWorkspaceInvitation> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard
            .problem_workspace_invitations
            .get(&invitation_id)
            .cloned()
    }

    fn update_problem_workspace_invitation(
        &self,
        invitation: ProblemWorkspaceInvitation,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        guard
            .problem_workspace_invitations
            .insert(invitation.id, invitation);
        Ok(())
    }

    fn list_problem_workspace_invitations(
        &self,
        agent_id: Uuid,
        workspace_id: Option<Uuid>,
        limit: usize,
    ) -> Vec<ProblemWorkspaceInvitation> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut invitations = guard
            .problem_workspace_invitations
            .values()
            .filter(|invitation| {
                invitation.inviter_agent_id == agent_id || invitation.invitee_agent_id == agent_id
            })
            .filter(|invitation| {
                workspace_id
                    .map(|value| invitation.workspace_id == value)
                    .unwrap_or(true)
            })
            .cloned()
            .collect::<Vec<_>>();
        invitations.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        invitations.truncate(limit);
        invitations
    }
}

fn ordered_pair(left: Uuid, right: Uuid) -> (Uuid, Uuid) {
    if left.as_bytes() <= right.as_bytes() {
        (left, right)
    } else {
        (right, left)
    }
}

fn task_status_history_entry(
    from_status: Option<String>,
    task: &CompanyProjectTask,
) -> CompanyProjectTaskStatusHistory {
    let has_explicit_updater =
        task.updated_by_agent_id.is_some() || task.updated_by_human_user_id.is_some();
    let changed_by_agent_id = task.updated_by_agent_id.or_else(|| {
        (!has_explicit_updater)
            .then_some(task.created_by_agent_id)
            .flatten()
    });
    let changed_by_human_user_id = task.updated_by_human_user_id.or_else(|| {
        (!has_explicit_updater)
            .then_some(task.created_by_human_user_id)
            .flatten()
    });
    let change_source = if changed_by_agent_id.is_some() {
        "agent"
    } else if changed_by_human_user_id.is_some() {
        "human"
    } else {
        "system"
    };
    CompanyProjectTaskStatusHistory {
        id: Uuid::new_v4(),
        project_id: task.project_id,
        task_id: task.id,
        from_status,
        to_status: task.status.clone(),
        changed_by_agent_id,
        changed_by_human_user_id,
        change_source: change_source.into(),
        metadata: serde_json::Value::Object(Default::default()),
        created_at: task.updated_at,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seed_logged_in_company(
        repo: &MemoryPlatformRepository,
        company_id: Uuid,
        now: chrono::DateTime<chrono::Utc>,
    ) {
        let human_user_id = Uuid::new_v4();
        let session = HumanSession {
            id: Uuid::new_v4(),
            human_user_id,
            token_prefix: "hus_test".into(),
            token_hash: Uuid::new_v4().simple().to_string(),
            expires_at: now + chrono::Duration::hours(1),
            revoked_at: None,
            last_used_at: Some(now),
            created_at: now,
        };
        let membership = CompanyHumanMember {
            id: Uuid::new_v4(),
            company_id,
            human_user_id,
            role: "owner".into(),
            status: "active".into(),
            created_at: now,
            updated_at: now,
        };
        let mut guard = repo.inner.write().expect("memory repo lock poisoned");
        guard.human_sessions.insert(session.id, session);
        guard
            .company_human_members
            .insert((company_id, human_user_id), membership);
    }

    fn running_codex_run(agent_profile_id: Uuid) -> AgentCodexTriggerRun {
        AgentCodexTriggerRun {
            id: Uuid::new_v4(),
            trigger_config_id: Uuid::new_v4(),
            agent_profile_id,
            project_id: None,
            trigger_type: "scheduled".into(),
            status: AGENT_CODEX_RUN_STATUS_RUNNING.into(),
            codex_thread_id: None,
            codex_version: None,
            exit_code: None,
            started_at: now_utc(),
            finished_at: None,
            final_message_summary: None,
            error_message: None,
            activity_phase: "running".into(),
            activity_summary: Some("Codex 已开始执行".into()),
            last_activity_at: Some(now_utc()),
            activity_log: Vec::new(),
        }
    }

    #[test]
    fn codex_trigger_allows_only_one_running_cycle_per_agent() {
        let repo = MemoryPlatformRepository::default();
        let agent_id = Uuid::new_v4();
        let mut first = running_codex_run(agent_id);
        let second = running_codex_run(agent_id);

        repo.insert_agent_codex_trigger_run(first.clone())
            .expect("first running cycle should be inserted");
        let error = repo
            .insert_agent_codex_trigger_run(second.clone())
            .expect_err("second running cycle for the same Agent must be rejected");
        assert!(matches!(
            error,
            ai_chat_shared::AppError::Conflict(message)
                if message.contains("already has a running Codex trigger run")
        ));

        first.status = "succeeded".into();
        first.finished_at = Some(now_utc());
        repo.update_agent_codex_trigger_run(first)
            .expect("completed cycle should be saved");
        repo.insert_agent_codex_trigger_run(second)
            .expect("a new cycle should be allowed after the previous one ends");
    }

    #[test]
    fn due_codex_trigger_requires_a_logged_in_company_human() {
        let repo = MemoryPlatformRepository::default();
        let company_id = Uuid::new_v4();
        let human_user_id = Uuid::new_v4();
        let agent_id = Uuid::new_v4();
        let now = now_utc();
        {
            let mut guard = repo.inner.write().expect("memory repo lock poisoned");
            guard.company_human_members.insert(
                (company_id, human_user_id),
                CompanyHumanMember {
                    id: Uuid::new_v4(),
                    company_id,
                    human_user_id,
                    role: "owner".into(),
                    status: "active".into(),
                    created_at: now,
                    updated_at: now,
                },
            );
            guard.agent_codex_trigger_configs.insert(
                agent_id,
                AgentCodexTriggerConfig {
                    id: Uuid::new_v4(),
                    company_id,
                    agent_profile_id: agent_id,
                    status: AGENT_CODEX_TRIGGER_STATUS_ACTIVE.into(),
                    interval_seconds: 3_600,
                    codex_profile: "default".into(),
                    model: None,
                    reasoning_effort: None,
                    sandbox_mode: "workspace_write".into(),
                    approval_policy: "never".into(),
                    max_run_seconds: 1_800,
                    next_run_at: now,
                    lease_owner: None,
                    lease_expires_at: None,
                    manual_run_requested_at: None,
                    wake_requested_at: None,
                    wake_reason: None,
                    last_run_at: None,
                    last_success_at: None,
                    last_error: None,
                    consecutive_failure_count: 0,
                    created_by_human_user_id: human_user_id,
                    updated_by_human_user_id: None,
                    created_at: now,
                    updated_at: now,
                },
            );
        }

        assert!(repo
            .claim_due_agent_codex_trigger_configs("worker-1", now, 1)
            .expect("logged-out company should be checked")
            .is_empty());

        repo.insert_human_session(HumanSession {
            id: Uuid::new_v4(),
            human_user_id,
            token_prefix: "hus_expired".into(),
            token_hash: Uuid::new_v4().simple().to_string(),
            expires_at: now,
            revoked_at: None,
            last_used_at: None,
            created_at: now - chrono::Duration::hours(1),
        })
        .expect("expired session should be inserted");
        assert!(repo
            .claim_due_agent_codex_trigger_configs("worker-1", now, 1)
            .expect("expired login should be checked")
            .is_empty());

        repo.insert_human_session(HumanSession {
            id: Uuid::new_v4(),
            human_user_id,
            token_prefix: "hus_stale".into(),
            token_hash: Uuid::new_v4().simple().to_string(),
            expires_at: now + chrono::Duration::hours(1),
            revoked_at: None,
            last_used_at: Some(now - chrono::Duration::minutes(2)),
            created_at: now - chrono::Duration::minutes(2),
        })
        .expect("stale session should be inserted");
        assert!(repo
            .claim_due_agent_codex_trigger_configs("worker-1", now, 1)
            .expect("inactive login should be checked")
            .is_empty());

        repo.insert_human_session(HumanSession {
            id: Uuid::new_v4(),
            human_user_id,
            token_prefix: "hus_active".into(),
            token_hash: Uuid::new_v4().simple().to_string(),
            expires_at: now + chrono::Duration::hours(1),
            revoked_at: None,
            last_used_at: Some(now),
            created_at: now,
        })
        .expect("active session should be inserted");
        let claimed = repo
            .claim_due_agent_codex_trigger_configs("worker-1", now, 1)
            .expect("logged-in company trigger should be claimable");
        assert_eq!(claimed.len(), 1);
        assert_eq!(claimed[0].company_id, company_id);
    }

    #[test]
    fn message_wake_requested_during_a_run_is_preserved_for_the_next_cycle() {
        let repo = MemoryPlatformRepository::default();
        let agent_id = Uuid::new_v4();
        let config_id = Uuid::new_v4();
        let company_id = Uuid::new_v4();
        let claimed_at = now_utc();
        seed_logged_in_company(&repo, company_id, claimed_at);
        repo.inner
            .write()
            .expect("memory repo lock poisoned")
            .agent_codex_trigger_configs
            .insert(
                agent_id,
                AgentCodexTriggerConfig {
                    id: config_id,
                    company_id,
                    agent_profile_id: agent_id,
                    status: AGENT_CODEX_TRIGGER_STATUS_ACTIVE.into(),
                    interval_seconds: 3_600,
                    codex_profile: "default".into(),
                    model: None,
                    reasoning_effort: None,
                    sandbox_mode: "workspace_write".into(),
                    approval_policy: "never".into(),
                    max_run_seconds: 1_800,
                    next_run_at: claimed_at,
                    lease_owner: None,
                    lease_expires_at: None,
                    manual_run_requested_at: None,
                    wake_requested_at: None,
                    wake_reason: None,
                    last_run_at: None,
                    last_success_at: None,
                    last_error: None,
                    consecutive_failure_count: 0,
                    created_by_human_user_id: Uuid::new_v4(),
                    updated_by_human_user_id: None,
                    created_at: claimed_at,
                    updated_at: claimed_at,
                },
            );

        let claimed = repo
            .claim_due_agent_codex_trigger_configs("worker-1", claimed_at, 1)
            .expect("trigger should be claimed");
        assert_eq!(claimed.len(), 1);
        let wake_requested_at = claimed_at + chrono::Duration::seconds(1);
        assert!(repo
            .request_agent_codex_trigger_wake(agent_id, wake_requested_at, "message")
            .expect("message wake should be recorded"));
        repo.complete_agent_codex_trigger_lease(CompleteAgentCodexTriggerLeaseInput {
            trigger_config_id: config_id,
            lease_owner: "worker-1".into(),
            finished_at: claimed_at + chrono::Duration::seconds(2),
            next_run_at: claimed_at + chrono::Duration::hours(1),
            succeeded: true,
            error_message: None,
        })
        .expect("lease should complete");

        let config = repo
            .get_agent_codex_trigger_config_by_agent(agent_id)
            .expect("trigger config should remain");
        assert_eq!(config.wake_requested_at, Some(wake_requested_at));
        assert_eq!(config.wake_reason.as_deref(), Some("message"));
        assert_eq!(config.next_run_at, wake_requested_at);
        assert!(config.lease_owner.is_none());
    }

    #[test]
    fn manual_wake_requested_during_a_run_is_preserved_for_the_next_cycle() {
        let repo = MemoryPlatformRepository::default();
        let agent_id = Uuid::new_v4();
        let config_id = Uuid::new_v4();
        let company_id = Uuid::new_v4();
        let claimed_at = now_utc();
        seed_logged_in_company(&repo, company_id, claimed_at);
        repo.inner
            .write()
            .expect("memory repo lock poisoned")
            .agent_codex_trigger_configs
            .insert(
                agent_id,
                AgentCodexTriggerConfig {
                    id: config_id,
                    company_id,
                    agent_profile_id: agent_id,
                    status: AGENT_CODEX_TRIGGER_STATUS_ACTIVE.into(),
                    interval_seconds: 3_600,
                    codex_profile: "default".into(),
                    model: None,
                    reasoning_effort: None,
                    sandbox_mode: "workspace_write".into(),
                    approval_policy: "never".into(),
                    max_run_seconds: 1_800,
                    next_run_at: claimed_at,
                    lease_owner: None,
                    lease_expires_at: None,
                    manual_run_requested_at: None,
                    wake_requested_at: None,
                    wake_reason: None,
                    last_run_at: None,
                    last_success_at: None,
                    last_error: None,
                    consecutive_failure_count: 0,
                    created_by_human_user_id: Uuid::new_v4(),
                    updated_by_human_user_id: None,
                    created_at: claimed_at,
                    updated_at: claimed_at,
                },
            );

        repo.claim_due_agent_codex_trigger_configs("worker-1", claimed_at, 1)
            .expect("trigger should be claimed");
        let manual_requested_at = claimed_at + chrono::Duration::seconds(1);
        {
            let mut guard = repo.inner.write().expect("memory repo lock poisoned");
            let config = guard
                .agent_codex_trigger_configs
                .get_mut(&agent_id)
                .expect("trigger config should exist");
            config.manual_run_requested_at = Some(manual_requested_at);
            config.next_run_at = manual_requested_at;
        }
        repo.complete_agent_codex_trigger_lease(CompleteAgentCodexTriggerLeaseInput {
            trigger_config_id: config_id,
            lease_owner: "worker-1".into(),
            finished_at: claimed_at + chrono::Duration::seconds(2),
            next_run_at: claimed_at + chrono::Duration::hours(1),
            succeeded: true,
            error_message: None,
        })
        .expect("lease should complete");

        let config = repo
            .get_agent_codex_trigger_config_by_agent(agent_id)
            .expect("trigger config should remain");
        assert_eq!(config.manual_run_requested_at, Some(manual_requested_at));
        assert_eq!(config.next_run_at, manual_requested_at);
        assert!(config.lease_owner.is_none());
    }
}
