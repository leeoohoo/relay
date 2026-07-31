pub mod codex_trigger;
pub mod config;
pub mod git_credentials;
pub mod git_workspace;
pub mod ownership_proof;
pub mod postgres;
pub mod realtime;
pub mod runtime_model;

use std::sync::Arc;

use anyhow::Context;

use ai_chat_application::{
    AgentRuntimeModelProvider, AgentStaffingHireBundle, AgentStaffingStatusChangeBundle,
    CompanyAgentActivationBundle, CompanyAgentCreationBundle, CompanyAgentMembershipUpdateBundle,
    CompanyConversationCreationBundle, CompanyCreationBundle, CompanyProjectCreationBundle,
    CompanyProjectMemberAddBundle, CompleteAgentCodexTriggerLeaseInput,
    HumanCompanyDirectConversationCreationBundle, MemoryPlatformRepository, PlatformRepository,
    RegistrationCompletionBundle,
};

use crate::config::{ApiConfig, RepositoryMode};
use crate::ownership_proof::OwnershipProofVerifierAdapter;
use crate::postgres::PostgresPlatformRepository;
use crate::runtime_model::OpenAiResponsesRuntimeModelProvider;

#[derive(Clone)]
pub enum RepositoryAdapter {
    Memory(MemoryPlatformRepository),
    Postgres(PostgresPlatformRepository),
}

impl RepositoryAdapter {
    pub fn build(config: &ApiConfig) -> anyhow::Result<Self> {
        match config.repository_mode {
            RepositoryMode::Memory => Ok(Self::Memory(MemoryPlatformRepository::default())),
            RepositoryMode::Postgres => Ok(Self::Postgres(
                PostgresPlatformRepository::connect(&config.database_url)
                    .context("failed to connect postgres repository")?,
            )),
        }
    }
}

pub fn build_ownership_proof_verifier(config: &ApiConfig) -> OwnershipProofVerifierAdapter {
    OwnershipProofVerifierAdapter::from_config(config)
        .expect("failed to build ownership proof verifier from config")
}

pub fn build_agent_runtime_model_provider(
    config: &ApiConfig,
) -> anyhow::Result<Arc<dyn AgentRuntimeModelProvider>> {
    Ok(Arc::new(
        OpenAiResponsesRuntimeModelProvider::with_secret_resolver_config(
            config.agent_runtime_openai_base_url.clone(),
            config.agent_runtime_allow_insecure_provider_http,
            config.agent_runtime_secret_resolver.clone(),
        )?,
    ))
}

impl PlatformRepository for RepositoryAdapter {
    fn health_check(&self) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.health_check(),
            Self::Postgres(repo) => repo.health_check(),
        }
    }

    fn find_human_user_by_email(
        &self,
        email: &str,
    ) -> Option<ai_chat_domain::agent_identity::HumanUser> {
        match self {
            Self::Memory(repo) => repo.find_human_user_by_email(email),
            Self::Postgres(repo) => repo.find_human_user_by_email(email),
        }
    }

    fn get_human_user(
        &self,
        user_id: uuid::Uuid,
    ) -> Option<ai_chat_domain::agent_identity::HumanUser> {
        match self {
            Self::Memory(repo) => repo.get_human_user(user_id),
            Self::Postgres(repo) => repo.get_human_user(user_id),
        }
    }

    fn list_human_users(&self) -> Vec<ai_chat_domain::agent_identity::HumanUser> {
        match self {
            Self::Memory(repo) => repo.list_human_users(),
            Self::Postgres(repo) => repo.list_human_users(),
        }
    }

    fn insert_human_user(
        &self,
        user: ai_chat_domain::agent_identity::HumanUser,
    ) -> ai_chat_shared::AppResult<ai_chat_domain::agent_identity::HumanUser> {
        match self {
            Self::Memory(repo) => repo.insert_human_user(user),
            Self::Postgres(repo) => repo.insert_human_user(user),
        }
    }

    fn human_user_exists(&self, user_id: uuid::Uuid) -> bool {
        match self {
            Self::Memory(repo) => repo.human_user_exists(user_id),
            Self::Postgres(repo) => repo.human_user_exists(user_id),
        }
    }

    fn get_human_credential(
        &self,
        user_id: uuid::Uuid,
    ) -> Option<ai_chat_domain::agent_identity::HumanCredential> {
        match self {
            Self::Memory(repo) => repo.get_human_credential(user_id),
            Self::Postgres(repo) => repo.get_human_credential(user_id),
        }
    }

    fn insert_human_auth_bundle(
        &self,
        user: ai_chat_domain::agent_identity::HumanUser,
        credential: ai_chat_domain::agent_identity::HumanCredential,
    ) -> ai_chat_shared::AppResult<ai_chat_domain::agent_identity::HumanUser> {
        match self {
            Self::Memory(repo) => repo.insert_human_auth_bundle(user, credential),
            Self::Postgres(repo) => repo.insert_human_auth_bundle(user, credential),
        }
    }

    fn insert_human_session(
        &self,
        session: ai_chat_domain::agent_identity::HumanSession,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.insert_human_session(session),
            Self::Postgres(repo) => repo.insert_human_session(session),
        }
    }

    fn find_human_session_by_token_hash(
        &self,
        token_hash: &str,
    ) -> Option<ai_chat_domain::agent_identity::HumanSession> {
        match self {
            Self::Memory(repo) => repo.find_human_session_by_token_hash(token_hash),
            Self::Postgres(repo) => repo.find_human_session_by_token_hash(token_hash),
        }
    }

    fn touch_human_session(
        &self,
        session_id: uuid::Uuid,
        used_at: chrono::DateTime<chrono::Utc>,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.touch_human_session(session_id, used_at),
            Self::Postgres(repo) => repo.touch_human_session(session_id, used_at),
        }
    }

    fn revoke_human_session(
        &self,
        session_id: uuid::Uuid,
        revoked_at: chrono::DateTime<chrono::Utc>,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.revoke_human_session(session_id, revoked_at),
            Self::Postgres(repo) => repo.revoke_human_session(session_id, revoked_at),
        }
    }

    fn get_human_session(
        &self,
        session_id: uuid::Uuid,
    ) -> Option<ai_chat_domain::agent_identity::HumanSession> {
        match self {
            Self::Memory(repo) => repo.get_human_session(session_id),
            Self::Postgres(repo) => repo.get_human_session(session_id),
        }
    }

    fn list_human_sessions(
        &self,
        human_user_id: uuid::Uuid,
    ) -> Vec<ai_chat_domain::agent_identity::HumanSession> {
        match self {
            Self::Memory(repo) => repo.list_human_sessions(human_user_id),
            Self::Postgres(repo) => repo.list_human_sessions(human_user_id),
        }
    }

    fn revoke_human_sessions(
        &self,
        human_user_id: uuid::Uuid,
        except_session_id: Option<uuid::Uuid>,
        revoked_at: chrono::DateTime<chrono::Utc>,
    ) -> ai_chat_shared::AppResult<usize> {
        match self {
            Self::Memory(repo) => {
                repo.revoke_human_sessions(human_user_id, except_session_id, revoked_at)
            }
            Self::Postgres(repo) => {
                repo.revoke_human_sessions(human_user_id, except_session_id, revoked_at)
            }
        }
    }

    fn update_human_password_hash(
        &self,
        human_user_id: uuid::Uuid,
        password_hash: String,
        updated_at: chrono::DateTime<chrono::Utc>,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => {
                repo.update_human_password_hash(human_user_id, password_hash, updated_at)
            }
            Self::Postgres(repo) => {
                repo.update_human_password_hash(human_user_id, password_hash, updated_at)
            }
        }
    }

    fn delete_expired_human_sessions(
        &self,
        now: chrono::DateTime<chrono::Utc>,
    ) -> ai_chat_shared::AppResult<usize> {
        match self {
            Self::Memory(repo) => repo.delete_expired_human_sessions(now),
            Self::Postgres(repo) => repo.delete_expired_human_sessions(now),
        }
    }

    fn insert_human_account_token(
        &self,
        token: ai_chat_domain::agent_identity::HumanAccountToken,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.insert_human_account_token(token),
            Self::Postgres(repo) => repo.insert_human_account_token(token),
        }
    }

    fn find_human_account_token_by_hash(
        &self,
        token_hash: &str,
    ) -> Option<ai_chat_domain::agent_identity::HumanAccountToken> {
        match self {
            Self::Memory(repo) => repo.find_human_account_token_by_hash(token_hash),
            Self::Postgres(repo) => repo.find_human_account_token_by_hash(token_hash),
        }
    }

    fn mark_human_account_token_used(
        &self,
        token_id: uuid::Uuid,
        used_at: chrono::DateTime<chrono::Utc>,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.mark_human_account_token_used(token_id, used_at),
            Self::Postgres(repo) => repo.mark_human_account_token_used(token_id, used_at),
        }
    }

    fn mark_human_email_verified(
        &self,
        human_user_id: uuid::Uuid,
        verified_at: chrono::DateTime<chrono::Utc>,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.mark_human_email_verified(human_user_id, verified_at),
            Self::Postgres(repo) => repo.mark_human_email_verified(human_user_id, verified_at),
        }
    }

    fn is_human_email_verified(&self, human_user_id: uuid::Uuid) -> bool {
        match self {
            Self::Memory(repo) => repo.is_human_email_verified(human_user_id),
            Self::Postgres(repo) => repo.is_human_email_verified(human_user_id),
        }
    }

    fn delete_expired_human_account_tokens(
        &self,
        now: chrono::DateTime<chrono::Utc>,
    ) -> ai_chat_shared::AppResult<usize> {
        match self {
            Self::Memory(repo) => repo.delete_expired_human_account_tokens(now),
            Self::Postgres(repo) => repo.delete_expired_human_account_tokens(now),
        }
    }

    fn insert_company_bundle(
        &self,
        bundle: CompanyCreationBundle,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.insert_company_bundle(bundle),
            Self::Postgres(repo) => repo.insert_company_bundle(bundle),
        }
    }

    fn get_company(&self, company_id: uuid::Uuid) -> Option<ai_chat_domain::company::Company> {
        match self {
            Self::Memory(repo) => repo.get_company(company_id),
            Self::Postgres(repo) => repo.get_company(company_id),
        }
    }

    fn get_company_by_slug(&self, slug: &str) -> Option<ai_chat_domain::company::Company> {
        match self {
            Self::Memory(repo) => repo.get_company_by_slug(slug),
            Self::Postgres(repo) => repo.get_company_by_slug(slug),
        }
    }

    fn list_human_companies(
        &self,
        human_user_id: uuid::Uuid,
    ) -> Vec<ai_chat_domain::company::Company> {
        match self {
            Self::Memory(repo) => repo.list_human_companies(human_user_id),
            Self::Postgres(repo) => repo.list_human_companies(human_user_id),
        }
    }

    fn get_company_human_member(
        &self,
        company_id: uuid::Uuid,
        human_user_id: uuid::Uuid,
    ) -> Option<ai_chat_domain::company::CompanyHumanMember> {
        match self {
            Self::Memory(repo) => repo.get_company_human_member(company_id, human_user_id),
            Self::Postgres(repo) => repo.get_company_human_member(company_id, human_user_id),
        }
    }

    fn get_org_unit(&self, org_unit_id: uuid::Uuid) -> Option<ai_chat_domain::company::OrgUnit> {
        match self {
            Self::Memory(repo) => repo.get_org_unit(org_unit_id),
            Self::Postgres(repo) => repo.get_org_unit(org_unit_id),
        }
    }

    fn insert_org_unit(
        &self,
        org_unit: ai_chat_domain::company::OrgUnit,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.insert_org_unit(org_unit),
            Self::Postgres(repo) => repo.insert_org_unit(org_unit),
        }
    }

    fn list_company_org_units(
        &self,
        company_id: uuid::Uuid,
    ) -> Vec<ai_chat_domain::company::OrgUnit> {
        match self {
            Self::Memory(repo) => repo.list_company_org_units(company_id),
            Self::Postgres(repo) => repo.list_company_org_units(company_id),
        }
    }

    fn get_company_agent_membership(
        &self,
        agent_id: uuid::Uuid,
    ) -> Option<ai_chat_domain::company::CompanyAgentMembership> {
        match self {
            Self::Memory(repo) => repo.get_company_agent_membership(agent_id),
            Self::Postgres(repo) => repo.get_company_agent_membership(agent_id),
        }
    }

    fn list_company_agent_memberships(
        &self,
        company_id: uuid::Uuid,
    ) -> Vec<ai_chat_domain::company::CompanyAgentMembership> {
        match self {
            Self::Memory(repo) => repo.list_company_agent_memberships(company_id),
            Self::Postgres(repo) => repo.list_company_agent_memberships(company_id),
        }
    }

    fn update_company_agent_work_profile(
        &self,
        agent_id: uuid::Uuid,
        responsibilities: Vec<String>,
        skills: Vec<String>,
        current_focus: String,
        collaboration_preference: String,
        updated_at: chrono::DateTime<chrono::Utc>,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.update_company_agent_work_profile(
                agent_id,
                responsibilities,
                skills,
                current_focus,
                collaboration_preference,
                updated_at,
            ),
            Self::Postgres(repo) => repo.update_company_agent_work_profile(
                agent_id,
                responsibilities,
                skills,
                current_focus,
                collaboration_preference,
                updated_at,
            ),
        }
    }

    fn complete_company_agent_creation_bundle(
        &self,
        bundle: CompanyAgentCreationBundle,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.complete_company_agent_creation_bundle(bundle),
            Self::Postgres(repo) => repo.complete_company_agent_creation_bundle(bundle),
        }
    }

    fn complete_company_agent_membership_update(
        &self,
        bundle: CompanyAgentMembershipUpdateBundle,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.complete_company_agent_membership_update(bundle),
            Self::Postgres(repo) => repo.complete_company_agent_membership_update(bundle),
        }
    }

    fn complete_agent_staffing_hire(
        &self,
        bundle: AgentStaffingHireBundle,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.complete_agent_staffing_hire(bundle),
            Self::Postgres(repo) => repo.complete_agent_staffing_hire(bundle),
        }
    }

    fn complete_company_agent_activation(
        &self,
        bundle: CompanyAgentActivationBundle,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.complete_company_agent_activation(bundle),
            Self::Postgres(repo) => repo.complete_company_agent_activation(bundle),
        }
    }

    fn complete_agent_staffing_status_change(
        &self,
        bundle: AgentStaffingStatusChangeBundle,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.complete_agent_staffing_status_change(bundle),
            Self::Postgres(repo) => repo.complete_agent_staffing_status_change(bundle),
        }
    }

    fn get_agent_staffing_action(
        &self,
        action_id: uuid::Uuid,
    ) -> Option<ai_chat_domain::company::AgentStaffingAction> {
        match self {
            Self::Memory(repo) => repo.get_agent_staffing_action(action_id),
            Self::Postgres(repo) => repo.get_agent_staffing_action(action_id),
        }
    }

    fn list_agent_staffing_actions(
        &self,
        company_id: uuid::Uuid,
    ) -> Vec<ai_chat_domain::company::AgentStaffingAction> {
        match self {
            Self::Memory(repo) => repo.list_agent_staffing_actions(company_id),
            Self::Postgres(repo) => repo.list_agent_staffing_actions(company_id),
        }
    }

    fn insert_registration_request(
        &self,
        request: ai_chat_domain::agent_identity::AgentRegistrationRequest,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.insert_registration_request(request),
            Self::Postgres(repo) => repo.insert_registration_request(request),
        }
    }

    fn insert_ownership_challenge(
        &self,
        challenge: ai_chat_domain::agent_identity::OwnershipProofChallenge,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.insert_ownership_challenge(challenge),
            Self::Postgres(repo) => repo.insert_ownership_challenge(challenge),
        }
    }

    fn get_challenge(
        &self,
        challenge_id: uuid::Uuid,
    ) -> Option<ai_chat_domain::agent_identity::OwnershipProofChallenge> {
        match self {
            Self::Memory(repo) => repo.get_challenge(challenge_id),
            Self::Postgres(repo) => repo.get_challenge(challenge_id),
        }
    }

    fn list_challenges(&self) -> Vec<ai_chat_domain::agent_identity::OwnershipProofChallenge> {
        match self {
            Self::Memory(repo) => repo.list_challenges(),
            Self::Postgres(repo) => repo.list_challenges(),
        }
    }

    fn update_challenge(
        &self,
        challenge: ai_chat_domain::agent_identity::OwnershipProofChallenge,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.update_challenge(challenge),
            Self::Postgres(repo) => repo.update_challenge(challenge),
        }
    }

    fn get_registration_request(
        &self,
        request_id: uuid::Uuid,
    ) -> Option<ai_chat_domain::agent_identity::AgentRegistrationRequest> {
        match self {
            Self::Memory(repo) => repo.get_registration_request(request_id),
            Self::Postgres(repo) => repo.get_registration_request(request_id),
        }
    }

    fn list_registration_requests(
        &self,
    ) -> Vec<ai_chat_domain::agent_identity::AgentRegistrationRequest> {
        match self {
            Self::Memory(repo) => repo.list_registration_requests(),
            Self::Postgres(repo) => repo.list_registration_requests(),
        }
    }

    fn update_registration_request(
        &self,
        request: ai_chat_domain::agent_identity::AgentRegistrationRequest,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.update_registration_request(request),
            Self::Postgres(repo) => repo.update_registration_request(request),
        }
    }

    fn insert_agent_profile(
        &self,
        profile: ai_chat_domain::agent_identity::AgentProfile,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.insert_agent_profile(profile),
            Self::Postgres(repo) => repo.insert_agent_profile(profile),
        }
    }

    fn complete_registration_bundle(
        &self,
        bundle: RegistrationCompletionBundle,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.complete_registration_bundle(bundle),
            Self::Postgres(repo) => repo.complete_registration_bundle(bundle),
        }
    }

    fn list_agent_profiles(&self) -> Vec<ai_chat_domain::agent_identity::AgentProfile> {
        match self {
            Self::Memory(repo) => repo.list_agent_profiles(),
            Self::Postgres(repo) => repo.list_agent_profiles(),
        }
    }

    fn update_agent_profile_status(
        &self,
        agent_id: uuid::Uuid,
        status: ai_chat_domain::agent_identity::AgentStatus,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.update_agent_profile_status(agent_id, status),
            Self::Postgres(repo) => repo.update_agent_profile_status(agent_id, status),
        }
    }

    fn update_agent_profile(
        &self,
        agent_id: uuid::Uuid,
        display_name: String,
        persona: String,
        collaboration_preference: String,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => {
                repo.update_agent_profile(agent_id, display_name, persona, collaboration_preference)
            }
            Self::Postgres(repo) => {
                repo.update_agent_profile(agent_id, display_name, persona, collaboration_preference)
            }
        }
    }

    fn insert_owner_binding(
        &self,
        binding: ai_chat_domain::agent_identity::AgentOwnerBinding,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.insert_owner_binding(binding),
            Self::Postgres(repo) => repo.insert_owner_binding(binding),
        }
    }

    fn insert_agent_key(
        &self,
        key_record: ai_chat_domain::agent_identity::AgentKeyRecord,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.insert_agent_key(key_record),
            Self::Postgres(repo) => repo.insert_agent_key(key_record),
        }
    }

    fn find_agent_key_by_plaintext(
        &self,
        plaintext_key: &str,
    ) -> Option<ai_chat_domain::agent_identity::AgentKeyRecord> {
        match self {
            Self::Memory(repo) => repo.find_agent_key_by_plaintext(plaintext_key),
            Self::Postgres(repo) => repo.find_agent_key_by_plaintext(plaintext_key),
        }
    }

    fn list_agent_keys(
        &self,
        agent_id: uuid::Uuid,
    ) -> Vec<ai_chat_domain::agent_identity::AgentKeyRecord> {
        match self {
            Self::Memory(repo) => repo.list_agent_keys(agent_id),
            Self::Postgres(repo) => repo.list_agent_keys(agent_id),
        }
    }

    fn touch_agent_key_usage(
        &self,
        key_id: uuid::Uuid,
        used_at: chrono::DateTime<chrono::Utc>,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.touch_agent_key_usage(key_id, used_at),
            Self::Postgres(repo) => repo.touch_agent_key_usage(key_id, used_at),
        }
    }

    fn revoke_agent_keys(
        &self,
        agent_id: uuid::Uuid,
        revoked_at: chrono::DateTime<chrono::Utc>,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.revoke_agent_keys(agent_id, revoked_at),
            Self::Postgres(repo) => repo.revoke_agent_keys(agent_id, revoked_at),
        }
    }

    fn insert_agent_key_issue_log(
        &self,
        log: ai_chat_domain::agent_identity::AgentKeyIssueLog,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.insert_agent_key_issue_log(log),
            Self::Postgres(repo) => repo.insert_agent_key_issue_log(log),
        }
    }

    fn list_agent_key_issue_logs(
        &self,
        agent_id: uuid::Uuid,
        limit: usize,
    ) -> Vec<ai_chat_domain::agent_identity::AgentKeyIssueLog> {
        match self {
            Self::Memory(repo) => repo.list_agent_key_issue_logs(agent_id, limit),
            Self::Postgres(repo) => repo.list_agent_key_issue_logs(agent_id, limit),
        }
    }

    fn insert_social_proof_submission(
        &self,
        submission: ai_chat_domain::agent_identity::SocialProofSubmission,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.insert_social_proof_submission(submission),
            Self::Postgres(repo) => repo.insert_social_proof_submission(submission),
        }
    }

    fn list_social_proof_submissions(
        &self,
    ) -> Vec<ai_chat_domain::agent_identity::SocialProofSubmission> {
        match self {
            Self::Memory(repo) => repo.list_social_proof_submissions(),
            Self::Postgres(repo) => repo.list_social_proof_submissions(),
        }
    }

    fn insert_agent_inbox_event(
        &self,
        event: ai_chat_domain::agent_identity::AgentInboxEvent,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.insert_agent_inbox_event(event),
            Self::Postgres(repo) => repo.insert_agent_inbox_event(event),
        }
    }

    fn list_agent_inbox_events(
        &self,
        agent_id: uuid::Uuid,
        status: Option<ai_chat_domain::agent_identity::AgentInboxEventStatus>,
        limit: usize,
    ) -> Vec<ai_chat_domain::agent_identity::AgentInboxEvent> {
        match self {
            Self::Memory(repo) => repo.list_agent_inbox_events(agent_id, status, limit),
            Self::Postgres(repo) => repo.list_agent_inbox_events(agent_id, status, limit),
        }
    }

    fn get_agent_inbox_event(
        &self,
        event_id: uuid::Uuid,
    ) -> Option<ai_chat_domain::agent_identity::AgentInboxEvent> {
        match self {
            Self::Memory(repo) => repo.get_agent_inbox_event(event_id),
            Self::Postgres(repo) => repo.get_agent_inbox_event(event_id),
        }
    }

    fn update_agent_inbox_event(
        &self,
        event: ai_chat_domain::agent_identity::AgentInboxEvent,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.update_agent_inbox_event(event),
            Self::Postgres(repo) => repo.update_agent_inbox_event(event),
        }
    }

    fn insert_agent_action_log(
        &self,
        log: ai_chat_domain::agent_identity::AgentActionLog,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.insert_agent_action_log(log),
            Self::Postgres(repo) => repo.insert_agent_action_log(log),
        }
    }

    fn list_all_agent_action_logs(&self) -> Vec<ai_chat_domain::agent_identity::AgentActionLog> {
        match self {
            Self::Memory(repo) => repo.list_all_agent_action_logs(),
            Self::Postgres(repo) => repo.list_all_agent_action_logs(),
        }
    }

    fn count_agent_actions_since(
        &self,
        agent_id: uuid::Uuid,
        since: chrono::DateTime<chrono::Utc>,
    ) -> usize {
        match self {
            Self::Memory(repo) => repo.count_agent_actions_since(agent_id, since),
            Self::Postgres(repo) => repo.count_agent_actions_since(agent_id, since),
        }
    }

    fn get_agent_idempotency_record(
        &self,
        agent_id: uuid::Uuid,
        operation: &str,
        idempotency_key: &str,
    ) -> Option<ai_chat_domain::agent_identity::AgentIdempotencyRecord> {
        match self {
            Self::Memory(repo) => {
                repo.get_agent_idempotency_record(agent_id, operation, idempotency_key)
            }
            Self::Postgres(repo) => {
                repo.get_agent_idempotency_record(agent_id, operation, idempotency_key)
            }
        }
    }

    fn insert_agent_idempotency_record(
        &self,
        record: ai_chat_domain::agent_identity::AgentIdempotencyRecord,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.insert_agent_idempotency_record(record),
            Self::Postgres(repo) => repo.insert_agent_idempotency_record(record),
        }
    }

    fn delete_expired_agent_idempotency_records(
        &self,
        now: chrono::DateTime<chrono::Utc>,
    ) -> ai_chat_shared::AppResult<usize> {
        match self {
            Self::Memory(repo) => repo.delete_expired_agent_idempotency_records(now),
            Self::Postgres(repo) => repo.delete_expired_agent_idempotency_records(now),
        }
    }

    fn ensure_agent_conversation_bucket(
        &self,
        agent_id: uuid::Uuid,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.ensure_agent_conversation_bucket(agent_id),
            Self::Postgres(repo) => repo.ensure_agent_conversation_bucket(agent_id),
        }
    }

    fn insert_conversation_preview(
        &self,
        agent_id: uuid::Uuid,
        preview: ai_chat_domain::social::ConversationPreview,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.insert_conversation_preview(agent_id, preview),
            Self::Postgres(repo) => repo.insert_conversation_preview(agent_id, preview),
        }
    }

    fn complete_company_conversation_creation(
        &self,
        bundle: CompanyConversationCreationBundle,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.complete_company_conversation_creation(bundle),
            Self::Postgres(repo) => repo.complete_company_conversation_creation(bundle),
        }
    }

    fn complete_human_company_direct_conversation_creation(
        &self,
        bundle: HumanCompanyDirectConversationCreationBundle,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.complete_human_company_direct_conversation_creation(bundle),
            Self::Postgres(repo) => {
                repo.complete_human_company_direct_conversation_creation(bundle)
            }
        }
    }

    fn find_company_direct_conversation(
        &self,
        company_id: uuid::Uuid,
        left_agent_id: uuid::Uuid,
        right_agent_id: uuid::Uuid,
    ) -> Option<ai_chat_domain::social::ConversationPreview> {
        match self {
            Self::Memory(repo) => {
                repo.find_company_direct_conversation(company_id, left_agent_id, right_agent_id)
            }
            Self::Postgres(repo) => {
                repo.find_company_direct_conversation(company_id, left_agent_id, right_agent_id)
            }
        }
    }

    fn find_human_company_direct_conversation(
        &self,
        company_id: uuid::Uuid,
        human_user_id: uuid::Uuid,
        target_agent_id: uuid::Uuid,
    ) -> Option<ai_chat_domain::social::ConversationPreview> {
        match self {
            Self::Memory(repo) => repo.find_human_company_direct_conversation(
                company_id,
                human_user_id,
                target_agent_id,
            ),
            Self::Postgres(repo) => repo.find_human_company_direct_conversation(
                company_id,
                human_user_id,
                target_agent_id,
            ),
        }
    }

    fn get_conversation_context(
        &self,
        conversation_id: uuid::Uuid,
    ) -> Option<ai_chat_domain::social::ConversationContext> {
        match self {
            Self::Memory(repo) => repo.get_conversation_context(conversation_id),
            Self::Postgres(repo) => repo.get_conversation_context(conversation_id),
        }
    }

    fn list_conversation_member_ids(&self, conversation_id: uuid::Uuid) -> Vec<uuid::Uuid> {
        match self {
            Self::Memory(repo) => repo.list_conversation_member_ids(conversation_id),
            Self::Postgres(repo) => repo.list_conversation_member_ids(conversation_id),
        }
    }

    fn complete_company_project_creation(
        &self,
        bundle: CompanyProjectCreationBundle,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.complete_company_project_creation(bundle),
            Self::Postgres(repo) => repo.complete_company_project_creation(bundle),
        }
    }

    fn get_company_project(
        &self,
        project_id: uuid::Uuid,
    ) -> Option<ai_chat_domain::company::CompanyProject> {
        match self {
            Self::Memory(repo) => repo.get_company_project(project_id),
            Self::Postgres(repo) => repo.get_company_project(project_id),
        }
    }

    fn list_company_projects(
        &self,
        company_id: uuid::Uuid,
    ) -> Vec<ai_chat_domain::company::CompanyProject> {
        match self {
            Self::Memory(repo) => repo.list_company_projects(company_id),
            Self::Postgres(repo) => repo.list_company_projects(company_id),
        }
    }

    fn save_company_project_git_config(
        &self,
        config: ai_chat_domain::company::CompanyProjectGitConfig,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.save_company_project_git_config(config),
            Self::Postgres(repo) => repo.save_company_project_git_config(config),
        }
    }

    fn get_company_project_git_config(
        &self,
        project_id: uuid::Uuid,
    ) -> Option<ai_chat_domain::company::CompanyProjectGitConfig> {
        match self {
            Self::Memory(repo) => repo.get_company_project_git_config(project_id),
            Self::Postgres(repo) => repo.get_company_project_git_config(project_id),
        }
    }

    fn delete_company_project_git_config(
        &self,
        project_id: uuid::Uuid,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.delete_company_project_git_config(project_id),
            Self::Postgres(repo) => repo.delete_company_project_git_config(project_id),
        }
    }

    fn save_company_project_rule(
        &self,
        rule: ai_chat_domain::company::CompanyProjectRule,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.save_company_project_rule(rule),
            Self::Postgres(repo) => repo.save_company_project_rule(rule),
        }
    }

    fn get_company_project_rule(
        &self,
        project_id: uuid::Uuid,
    ) -> Option<ai_chat_domain::company::CompanyProjectRule> {
        match self {
            Self::Memory(repo) => repo.get_company_project_rule(project_id),
            Self::Postgres(repo) => repo.get_company_project_rule(project_id),
        }
    }

    fn replace_company_project_assets(
        &self,
        project_id: uuid::Uuid,
        assets: Vec<ai_chat_domain::company::CompanyProjectAsset>,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.replace_company_project_assets(project_id, assets),
            Self::Postgres(repo) => repo.replace_company_project_assets(project_id, assets),
        }
    }

    fn list_company_project_assets(
        &self,
        project_id: uuid::Uuid,
    ) -> Vec<ai_chat_domain::company::CompanyProjectAsset> {
        match self {
            Self::Memory(repo) => repo.list_company_project_assets(project_id),
            Self::Postgres(repo) => repo.list_company_project_assets(project_id),
        }
    }

    fn save_company_project_asset_refresh_config(
        &self,
        config: ai_chat_domain::company::CompanyProjectAssetRefreshConfig,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.save_company_project_asset_refresh_config(config),
            Self::Postgres(repo) => repo.save_company_project_asset_refresh_config(config),
        }
    }

    fn get_company_project_asset_refresh_config(
        &self,
        project_id: uuid::Uuid,
    ) -> Option<ai_chat_domain::company::CompanyProjectAssetRefreshConfig> {
        match self {
            Self::Memory(repo) => repo.get_company_project_asset_refresh_config(project_id),
            Self::Postgres(repo) => repo.get_company_project_asset_refresh_config(project_id),
        }
    }

    fn claim_due_company_project_asset_refresh(
        &self,
        agent_id: uuid::Uuid,
        now: chrono::DateTime<chrono::Utc>,
    ) -> ai_chat_shared::AppResult<Option<ai_chat_domain::company::CompanyProjectAssetRefreshConfig>>
    {
        match self {
            Self::Memory(repo) => repo.claim_due_company_project_asset_refresh(agent_id, now),
            Self::Postgres(repo) => repo.claim_due_company_project_asset_refresh(agent_id, now),
        }
    }

    fn mark_company_project_asset_refresh_completed(
        &self,
        project_id: uuid::Uuid,
        completed_at: chrono::DateTime<chrono::Utc>,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => {
                repo.mark_company_project_asset_refresh_completed(project_id, completed_at)
            }
            Self::Postgres(repo) => {
                repo.mark_company_project_asset_refresh_completed(project_id, completed_at)
            }
        }
    }

    fn save_company_codex_runner_profile(
        &self,
        profile: ai_chat_domain::company::CompanyCodexRunnerProfile,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.save_company_codex_runner_profile(profile),
            Self::Postgres(repo) => repo.save_company_codex_runner_profile(profile),
        }
    }

    fn get_company_codex_runner_profile(
        &self,
        profile_id: uuid::Uuid,
    ) -> Option<ai_chat_domain::company::CompanyCodexRunnerProfile> {
        match self {
            Self::Memory(repo) => repo.get_company_codex_runner_profile(profile_id),
            Self::Postgres(repo) => repo.get_company_codex_runner_profile(profile_id),
        }
    }

    fn list_company_codex_runner_profiles(
        &self,
        company_id: uuid::Uuid,
    ) -> Vec<ai_chat_domain::company::CompanyCodexRunnerProfile> {
        match self {
            Self::Memory(repo) => repo.list_company_codex_runner_profiles(company_id),
            Self::Postgres(repo) => repo.list_company_codex_runner_profiles(company_id),
        }
    }

    fn delete_company_codex_runner_profile(
        &self,
        profile_id: uuid::Uuid,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.delete_company_codex_runner_profile(profile_id),
            Self::Postgres(repo) => repo.delete_company_codex_runner_profile(profile_id),
        }
    }

    fn clear_company_codex_runner_profile_defaults(
        &self,
        company_id: uuid::Uuid,
        except_profile_id: uuid::Uuid,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => {
                repo.clear_company_codex_runner_profile_defaults(company_id, except_profile_id)
            }
            Self::Postgres(repo) => {
                repo.clear_company_codex_runner_profile_defaults(company_id, except_profile_id)
            }
        }
    }

    fn assign_agent_codex_runner_profile(
        &self,
        agent_id: uuid::Uuid,
        profile_id: uuid::Uuid,
        human_user_id: uuid::Uuid,
        assigned_at: chrono::DateTime<chrono::Utc>,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.assign_agent_codex_runner_profile(
                agent_id,
                profile_id,
                human_user_id,
                assigned_at,
            ),
            Self::Postgres(repo) => repo.assign_agent_codex_runner_profile(
                agent_id,
                profile_id,
                human_user_id,
                assigned_at,
            ),
        }
    }

    fn get_agent_codex_runner_profile_assignment(
        &self,
        agent_id: uuid::Uuid,
    ) -> Option<uuid::Uuid> {
        match self {
            Self::Memory(repo) => repo.get_agent_codex_runner_profile_assignment(agent_id),
            Self::Postgres(repo) => repo.get_agent_codex_runner_profile_assignment(agent_id),
        }
    }

    fn list_codex_runner_profile_agent_ids(&self, profile_id: uuid::Uuid) -> Vec<uuid::Uuid> {
        match self {
            Self::Memory(repo) => repo.list_codex_runner_profile_agent_ids(profile_id),
            Self::Postgres(repo) => repo.list_codex_runner_profile_agent_ids(profile_id),
        }
    }

    fn save_agent_codex_trigger_config(
        &self,
        config: ai_chat_domain::company::AgentCodexTriggerConfig,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.save_agent_codex_trigger_config(config),
            Self::Postgres(repo) => repo.save_agent_codex_trigger_config(config),
        }
    }

    fn get_agent_codex_trigger_config_by_agent(
        &self,
        agent_id: uuid::Uuid,
    ) -> Option<ai_chat_domain::company::AgentCodexTriggerConfig> {
        match self {
            Self::Memory(repo) => repo.get_agent_codex_trigger_config_by_agent(agent_id),
            Self::Postgres(repo) => repo.get_agent_codex_trigger_config_by_agent(agent_id),
        }
    }

    fn claim_due_agent_codex_trigger_configs(
        &self,
        lease_owner: &str,
        now: chrono::DateTime<chrono::Utc>,
        limit: usize,
    ) -> ai_chat_shared::AppResult<Vec<ai_chat_domain::company::AgentCodexTriggerConfig>> {
        match self {
            Self::Memory(repo) => {
                repo.claim_due_agent_codex_trigger_configs(lease_owner, now, limit)
            }
            Self::Postgres(repo) => {
                repo.claim_due_agent_codex_trigger_configs(lease_owner, now, limit)
            }
        }
    }

    fn request_agent_codex_trigger_wake(
        &self,
        agent_id: uuid::Uuid,
        requested_at: chrono::DateTime<chrono::Utc>,
        reason: &str,
    ) -> ai_chat_shared::AppResult<bool> {
        match self {
            Self::Memory(repo) => {
                repo.request_agent_codex_trigger_wake(agent_id, requested_at, reason)
            }
            Self::Postgres(repo) => {
                repo.request_agent_codex_trigger_wake(agent_id, requested_at, reason)
            }
        }
    }

    fn complete_agent_codex_trigger_lease(
        &self,
        input: CompleteAgentCodexTriggerLeaseInput,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.complete_agent_codex_trigger_lease(input),
            Self::Postgres(repo) => repo.complete_agent_codex_trigger_lease(input),
        }
    }

    fn insert_agent_codex_trigger_run(
        &self,
        run: ai_chat_domain::company::AgentCodexTriggerRun,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.insert_agent_codex_trigger_run(run),
            Self::Postgres(repo) => repo.insert_agent_codex_trigger_run(run),
        }
    }

    fn has_running_agent_codex_trigger_run(&self, agent_id: uuid::Uuid) -> bool {
        match self {
            Self::Memory(repo) => repo.has_running_agent_codex_trigger_run(agent_id),
            Self::Postgres(repo) => repo.has_running_agent_codex_trigger_run(agent_id),
        }
    }

    fn update_agent_codex_trigger_run(
        &self,
        run: ai_chat_domain::company::AgentCodexTriggerRun,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.update_agent_codex_trigger_run(run),
            Self::Postgres(repo) => repo.update_agent_codex_trigger_run(run),
        }
    }

    fn append_agent_codex_trigger_run_activity(
        &self,
        run_id: uuid::Uuid,
        activity: ai_chat_domain::company::AgentCodexRunActivity,
        codex_thread_id: Option<String>,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => {
                repo.append_agent_codex_trigger_run_activity(run_id, activity, codex_thread_id)
            }
            Self::Postgres(repo) => {
                repo.append_agent_codex_trigger_run_activity(run_id, activity, codex_thread_id)
            }
        }
    }

    fn list_agent_codex_trigger_runs(
        &self,
        agent_id: uuid::Uuid,
        limit: usize,
    ) -> Vec<ai_chat_domain::company::AgentCodexTriggerRun> {
        match self {
            Self::Memory(repo) => repo.list_agent_codex_trigger_runs(agent_id, limit),
            Self::Postgres(repo) => repo.list_agent_codex_trigger_runs(agent_id, limit),
        }
    }

    fn save_agent_codex_session(
        &self,
        session: ai_chat_domain::company::AgentCodexSession,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.save_agent_codex_session(session),
            Self::Postgres(repo) => repo.save_agent_codex_session(session),
        }
    }

    fn get_agent_codex_session(
        &self,
        agent_id: uuid::Uuid,
    ) -> Option<ai_chat_domain::company::AgentCodexSession> {
        match self {
            Self::Memory(repo) => repo.get_agent_codex_session(agent_id),
            Self::Postgres(repo) => repo.get_agent_codex_session(agent_id),
        }
    }

    fn insert_agent_codex_run_token(
        &self,
        token: ai_chat_domain::company::AgentCodexRunToken,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.insert_agent_codex_run_token(token),
            Self::Postgres(repo) => repo.insert_agent_codex_run_token(token),
        }
    }

    fn find_agent_codex_run_token_by_hash(
        &self,
        token_hash: &str,
    ) -> Option<ai_chat_domain::company::AgentCodexRunToken> {
        match self {
            Self::Memory(repo) => repo.find_agent_codex_run_token_by_hash(token_hash),
            Self::Postgres(repo) => repo.find_agent_codex_run_token_by_hash(token_hash),
        }
    }

    fn revoke_agent_codex_run_tokens(
        &self,
        run_id: uuid::Uuid,
        revoked_at: chrono::DateTime<chrono::Utc>,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.revoke_agent_codex_run_tokens(run_id, revoked_at),
            Self::Postgres(repo) => repo.revoke_agent_codex_run_tokens(run_id, revoked_at),
        }
    }

    fn delete_expired_agent_codex_run_tokens(
        &self,
        now: chrono::DateTime<chrono::Utc>,
    ) -> ai_chat_shared::AppResult<usize> {
        match self {
            Self::Memory(repo) => repo.delete_expired_agent_codex_run_tokens(now),
            Self::Postgres(repo) => repo.delete_expired_agent_codex_run_tokens(now),
        }
    }

    fn update_company_project(
        &self,
        project: ai_chat_domain::company::CompanyProject,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.update_company_project(project),
            Self::Postgres(repo) => repo.update_company_project(project),
        }
    }

    fn update_company_project_metadata(
        &self,
        project: ai_chat_domain::company::CompanyProject,
        project_group_title: String,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => {
                repo.update_company_project_metadata(project, project_group_title)
            }
            Self::Postgres(repo) => {
                repo.update_company_project_metadata(project, project_group_title)
            }
        }
    }

    fn get_company_project_member(
        &self,
        project_id: uuid::Uuid,
        agent_id: uuid::Uuid,
    ) -> Option<ai_chat_domain::company::CompanyProjectMember> {
        match self {
            Self::Memory(repo) => repo.get_company_project_member(project_id, agent_id),
            Self::Postgres(repo) => repo.get_company_project_member(project_id, agent_id),
        }
    }

    fn list_company_project_members(
        &self,
        project_id: uuid::Uuid,
    ) -> Vec<ai_chat_domain::company::CompanyProjectMember> {
        match self {
            Self::Memory(repo) => repo.list_company_project_members(project_id),
            Self::Postgres(repo) => repo.list_company_project_members(project_id),
        }
    }

    fn complete_company_project_member_add(
        &self,
        bundle: CompanyProjectMemberAddBundle,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.complete_company_project_member_add(bundle),
            Self::Postgres(repo) => repo.complete_company_project_member_add(bundle),
        }
    }

    fn complete_company_project_member_remove(
        &self,
        project_id: uuid::Uuid,
        agent_id: uuid::Uuid,
        conversation_id: uuid::Uuid,
        left_at: chrono::DateTime<chrono::Utc>,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.complete_company_project_member_remove(
                project_id,
                agent_id,
                conversation_id,
                left_at,
            ),
            Self::Postgres(repo) => repo.complete_company_project_member_remove(
                project_id,
                agent_id,
                conversation_id,
                left_at,
            ),
        }
    }

    fn insert_company_project_task(
        &self,
        task: ai_chat_domain::company::CompanyProjectTask,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.insert_company_project_task(task),
            Self::Postgres(repo) => repo.insert_company_project_task(task),
        }
    }

    fn get_company_project_task(
        &self,
        task_id: uuid::Uuid,
    ) -> Option<ai_chat_domain::company::CompanyProjectTask> {
        match self {
            Self::Memory(repo) => repo.get_company_project_task(task_id),
            Self::Postgres(repo) => repo.get_company_project_task(task_id),
        }
    }

    fn list_company_project_tasks(
        &self,
        project_id: uuid::Uuid,
    ) -> Vec<ai_chat_domain::company::CompanyProjectTask> {
        match self {
            Self::Memory(repo) => repo.list_company_project_tasks(project_id),
            Self::Postgres(repo) => repo.list_company_project_tasks(project_id),
        }
    }

    fn update_company_project_task(
        &self,
        task: ai_chat_domain::company::CompanyProjectTask,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.update_company_project_task(task),
            Self::Postgres(repo) => repo.update_company_project_task(task),
        }
    }

    fn update_company_project_tasks(
        &self,
        tasks: Vec<ai_chat_domain::company::CompanyProjectTask>,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.update_company_project_tasks(tasks),
            Self::Postgres(repo) => repo.update_company_project_tasks(tasks),
        }
    }

    fn insert_company_project_task_dependency(
        &self,
        dependency: ai_chat_domain::company::CompanyProjectTaskDependency,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.insert_company_project_task_dependency(dependency),
            Self::Postgres(repo) => repo.insert_company_project_task_dependency(dependency),
        }
    }

    fn remove_company_project_task_dependency(
        &self,
        project_id: uuid::Uuid,
        task_id: uuid::Uuid,
        depends_on_task_id: uuid::Uuid,
        removed_by_agent_id: Option<uuid::Uuid>,
        removed_by_human_user_id: Option<uuid::Uuid>,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.remove_company_project_task_dependency(
                project_id,
                task_id,
                depends_on_task_id,
                removed_by_agent_id,
                removed_by_human_user_id,
            ),
            Self::Postgres(repo) => repo.remove_company_project_task_dependency(
                project_id,
                task_id,
                depends_on_task_id,
                removed_by_agent_id,
                removed_by_human_user_id,
            ),
        }
    }

    fn list_company_project_task_dependencies(
        &self,
        project_id: uuid::Uuid,
    ) -> Vec<ai_chat_domain::company::CompanyProjectTaskDependency> {
        match self {
            Self::Memory(repo) => repo.list_company_project_task_dependencies(project_id),
            Self::Postgres(repo) => repo.list_company_project_task_dependencies(project_id),
        }
    }

    fn list_company_project_task_status_history(
        &self,
        project_id: uuid::Uuid,
    ) -> Vec<ai_chat_domain::company::CompanyProjectTaskStatusHistory> {
        match self {
            Self::Memory(repo) => repo.list_company_project_task_status_history(project_id),
            Self::Postgres(repo) => repo.list_company_project_task_status_history(project_id),
        }
    }

    fn insert_company_project_status_update(
        &self,
        update: ai_chat_domain::company::CompanyProjectStatusUpdate,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.insert_company_project_status_update(update),
            Self::Postgres(repo) => repo.insert_company_project_status_update(update),
        }
    }

    fn list_company_project_status_updates(
        &self,
        project_id: uuid::Uuid,
    ) -> Vec<ai_chat_domain::company::CompanyProjectStatusUpdate> {
        match self {
            Self::Memory(repo) => repo.list_company_project_status_updates(project_id),
            Self::Postgres(repo) => repo.list_company_project_status_updates(project_id),
        }
    }

    fn list_company_realtime_events(
        &self,
        company_id: uuid::Uuid,
        after_sequence_id: i64,
        limit: usize,
    ) -> Vec<ai_chat_domain::company::CompanyRealtimeEvent> {
        match self {
            Self::Memory(repo) => {
                repo.list_company_realtime_events(company_id, after_sequence_id, limit)
            }
            Self::Postgres(repo) => {
                repo.list_company_realtime_events(company_id, after_sequence_id, limit)
            }
        }
    }

    fn latest_company_realtime_sequence(&self, company_id: uuid::Uuid) -> i64 {
        match self {
            Self::Memory(repo) => repo.latest_company_realtime_sequence(company_id),
            Self::Postgres(repo) => repo.latest_company_realtime_sequence(company_id),
        }
    }

    fn save_agent_runtime_config(
        &self,
        config: ai_chat_domain::company::AgentRuntimeConfig,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.save_agent_runtime_config(config),
            Self::Postgres(repo) => repo.save_agent_runtime_config(config),
        }
    }

    fn get_agent_runtime_config(
        &self,
        runtime_config_id: uuid::Uuid,
    ) -> Option<ai_chat_domain::company::AgentRuntimeConfig> {
        match self {
            Self::Memory(repo) => repo.get_agent_runtime_config(runtime_config_id),
            Self::Postgres(repo) => repo.get_agent_runtime_config(runtime_config_id),
        }
    }

    fn get_agent_runtime_config_by_agent(
        &self,
        agent_id: uuid::Uuid,
    ) -> Option<ai_chat_domain::company::AgentRuntimeConfig> {
        match self {
            Self::Memory(repo) => repo.get_agent_runtime_config_by_agent(agent_id),
            Self::Postgres(repo) => repo.get_agent_runtime_config_by_agent(agent_id),
        }
    }

    fn list_company_agent_runtime_configs(
        &self,
        company_id: uuid::Uuid,
    ) -> Vec<ai_chat_domain::company::AgentRuntimeConfig> {
        match self {
            Self::Memory(repo) => repo.list_company_agent_runtime_configs(company_id),
            Self::Postgres(repo) => repo.list_company_agent_runtime_configs(company_id),
        }
    }

    fn save_agent_runtime_template(
        &self,
        template: ai_chat_domain::company::AgentRuntimeTemplate,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.save_agent_runtime_template(template),
            Self::Postgres(repo) => repo.save_agent_runtime_template(template),
        }
    }

    fn get_agent_runtime_template(
        &self,
        template_id: uuid::Uuid,
    ) -> Option<ai_chat_domain::company::AgentRuntimeTemplate> {
        match self {
            Self::Memory(repo) => repo.get_agent_runtime_template(template_id),
            Self::Postgres(repo) => repo.get_agent_runtime_template(template_id),
        }
    }

    fn list_company_agent_runtime_templates(
        &self,
        company_id: uuid::Uuid,
    ) -> Vec<ai_chat_domain::company::AgentRuntimeTemplate> {
        match self {
            Self::Memory(repo) => repo.list_company_agent_runtime_templates(company_id),
            Self::Postgres(repo) => repo.list_company_agent_runtime_templates(company_id),
        }
    }

    fn save_company_runtime_policy(
        &self,
        policy: ai_chat_domain::company::CompanyRuntimePolicy,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.save_company_runtime_policy(policy),
            Self::Postgres(repo) => repo.save_company_runtime_policy(policy),
        }
    }

    fn get_company_runtime_policy(
        &self,
        company_id: uuid::Uuid,
    ) -> Option<ai_chat_domain::company::CompanyRuntimePolicy> {
        match self {
            Self::Memory(repo) => repo.get_company_runtime_policy(company_id),
            Self::Postgres(repo) => repo.get_company_runtime_policy(company_id),
        }
    }

    fn publish_agent_model_price_catalog_entry(
        &self,
        entry: ai_chat_domain::company::AgentModelPriceCatalogEntry,
    ) -> ai_chat_shared::AppResult<ai_chat_domain::company::AgentModelPriceCatalogEntry> {
        match self {
            Self::Memory(repo) => repo.publish_agent_model_price_catalog_entry(entry),
            Self::Postgres(repo) => repo.publish_agent_model_price_catalog_entry(entry),
        }
    }

    fn save_agent_model_price_catalog_entry(
        &self,
        entry: ai_chat_domain::company::AgentModelPriceCatalogEntry,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.save_agent_model_price_catalog_entry(entry),
            Self::Postgres(repo) => repo.save_agent_model_price_catalog_entry(entry),
        }
    }

    fn get_agent_model_price_catalog_entry(
        &self,
        entry_id: uuid::Uuid,
    ) -> Option<ai_chat_domain::company::AgentModelPriceCatalogEntry> {
        match self {
            Self::Memory(repo) => repo.get_agent_model_price_catalog_entry(entry_id),
            Self::Postgres(repo) => repo.get_agent_model_price_catalog_entry(entry_id),
        }
    }

    fn list_company_agent_model_price_catalog_entries(
        &self,
        company_id: uuid::Uuid,
    ) -> Vec<ai_chat_domain::company::AgentModelPriceCatalogEntry> {
        match self {
            Self::Memory(repo) => repo.list_company_agent_model_price_catalog_entries(company_id),
            Self::Postgres(repo) => repo.list_company_agent_model_price_catalog_entries(company_id),
        }
    }

    fn publish_company_governance_policy_version(
        &self,
        version: ai_chat_domain::company::CompanyGovernancePolicyVersion,
    ) -> ai_chat_shared::AppResult<ai_chat_domain::company::CompanyGovernancePolicyVersion> {
        match self {
            Self::Memory(repo) => repo.publish_company_governance_policy_version(version),
            Self::Postgres(repo) => repo.publish_company_governance_policy_version(version),
        }
    }

    fn get_active_company_governance_policy_version(
        &self,
        company_id: uuid::Uuid,
    ) -> Option<ai_chat_domain::company::CompanyGovernancePolicyVersion> {
        match self {
            Self::Memory(repo) => repo.get_active_company_governance_policy_version(company_id),
            Self::Postgres(repo) => repo.get_active_company_governance_policy_version(company_id),
        }
    }

    fn list_company_governance_policy_versions(
        &self,
        company_id: uuid::Uuid,
    ) -> Vec<ai_chat_domain::company::CompanyGovernancePolicyVersion> {
        match self {
            Self::Memory(repo) => repo.list_company_governance_policy_versions(company_id),
            Self::Postgres(repo) => repo.list_company_governance_policy_versions(company_id),
        }
    }

    fn list_due_agent_runtime_configs(
        &self,
        now: chrono::DateTime<chrono::Utc>,
        limit: usize,
    ) -> Vec<ai_chat_domain::company::AgentRuntimeConfig> {
        match self {
            Self::Memory(repo) => repo.list_due_agent_runtime_configs(now, limit),
            Self::Postgres(repo) => repo.list_due_agent_runtime_configs(now, limit),
        }
    }

    fn claim_agent_runtime_config(
        &self,
        runtime_config_id: uuid::Uuid,
        now: chrono::DateTime<chrono::Utc>,
        next_run_at: chrono::DateTime<chrono::Utc>,
    ) -> ai_chat_shared::AppResult<bool> {
        match self {
            Self::Memory(repo) => {
                repo.claim_agent_runtime_config(runtime_config_id, now, next_run_at)
            }
            Self::Postgres(repo) => {
                repo.claim_agent_runtime_config(runtime_config_id, now, next_run_at)
            }
        }
    }

    fn insert_agent_runtime_run(
        &self,
        run: ai_chat_domain::company::AgentRuntimeRun,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.insert_agent_runtime_run(run),
            Self::Postgres(repo) => repo.insert_agent_runtime_run(run),
        }
    }

    fn update_agent_runtime_run(
        &self,
        run: ai_chat_domain::company::AgentRuntimeRun,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.update_agent_runtime_run(run),
            Self::Postgres(repo) => repo.update_agent_runtime_run(run),
        }
    }

    fn list_agent_runtime_runs(
        &self,
        runtime_config_id: uuid::Uuid,
        limit: usize,
    ) -> Vec<ai_chat_domain::company::AgentRuntimeRun> {
        match self {
            Self::Memory(repo) => repo.list_agent_runtime_runs(runtime_config_id, limit),
            Self::Postgres(repo) => repo.list_agent_runtime_runs(runtime_config_id, limit),
        }
    }

    fn get_agent_runtime_daily_usage(
        &self,
        runtime_config_id: uuid::Uuid,
        window_start: chrono::DateTime<chrono::Utc>,
    ) -> ai_chat_domain::company::AgentRuntimeDailyUsage {
        match self {
            Self::Memory(repo) => {
                repo.get_agent_runtime_daily_usage(runtime_config_id, window_start)
            }
            Self::Postgres(repo) => {
                repo.get_agent_runtime_daily_usage(runtime_config_id, window_start)
            }
        }
    }

    fn save_company_model_budget_policy(
        &self,
        policy: ai_chat_domain::company::CompanyModelBudgetPolicy,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.save_company_model_budget_policy(policy),
            Self::Postgres(repo) => repo.save_company_model_budget_policy(policy),
        }
    }

    fn get_company_model_budget_policy(
        &self,
        company_id: uuid::Uuid,
    ) -> Option<ai_chat_domain::company::CompanyModelBudgetPolicy> {
        match self {
            Self::Memory(repo) => repo.get_company_model_budget_policy(company_id),
            Self::Postgres(repo) => repo.get_company_model_budget_policy(company_id),
        }
    }

    fn get_company_model_daily_usage(
        &self,
        company_id: uuid::Uuid,
        window_start: chrono::DateTime<chrono::Utc>,
    ) -> ai_chat_domain::company::CompanyModelDailyUsage {
        match self {
            Self::Memory(repo) => repo.get_company_model_daily_usage(company_id, window_start),
            Self::Postgres(repo) => repo.get_company_model_daily_usage(company_id, window_start),
        }
    }

    fn insert_agent_tool_approval_request(
        &self,
        request: ai_chat_domain::company::AgentToolApprovalRequest,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.insert_agent_tool_approval_request(request),
            Self::Postgres(repo) => repo.insert_agent_tool_approval_request(request),
        }
    }

    fn get_agent_tool_approval_request(
        &self,
        approval_request_id: uuid::Uuid,
    ) -> Option<ai_chat_domain::company::AgentToolApprovalRequest> {
        match self {
            Self::Memory(repo) => repo.get_agent_tool_approval_request(approval_request_id),
            Self::Postgres(repo) => repo.get_agent_tool_approval_request(approval_request_id),
        }
    }

    fn list_company_agent_tool_approval_requests(
        &self,
        company_id: uuid::Uuid,
        status: Option<&str>,
        limit: usize,
    ) -> Vec<ai_chat_domain::company::AgentToolApprovalRequest> {
        match self {
            Self::Memory(repo) => {
                repo.list_company_agent_tool_approval_requests(company_id, status, limit)
            }
            Self::Postgres(repo) => {
                repo.list_company_agent_tool_approval_requests(company_id, status, limit)
            }
        }
    }

    fn claim_agent_tool_approval_request(
        &self,
        approval_request_id: uuid::Uuid,
        human_user_id: uuid::Uuid,
        status: &str,
        review_note: &str,
        now: chrono::DateTime<chrono::Utc>,
    ) -> ai_chat_shared::AppResult<Option<ai_chat_domain::company::AgentToolApprovalRequest>> {
        match self {
            Self::Memory(repo) => repo.claim_agent_tool_approval_request(
                approval_request_id,
                human_user_id,
                status,
                review_note,
                now,
            ),
            Self::Postgres(repo) => repo.claim_agent_tool_approval_request(
                approval_request_id,
                human_user_id,
                status,
                review_note,
                now,
            ),
        }
    }

    fn update_agent_tool_approval_request(
        &self,
        request: ai_chat_domain::company::AgentToolApprovalRequest,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.update_agent_tool_approval_request(request),
            Self::Postgres(repo) => repo.update_agent_tool_approval_request(request),
        }
    }

    fn count_agent_tool_approval_requests_since(
        &self,
        agent_id: uuid::Uuid,
        since: chrono::DateTime<chrono::Utc>,
    ) -> usize {
        match self {
            Self::Memory(repo) => repo.count_agent_tool_approval_requests_since(agent_id, since),
            Self::Postgres(repo) => repo.count_agent_tool_approval_requests_since(agent_id, since),
        }
    }

    fn list_owner_agents(
        &self,
        human_user_id: uuid::Uuid,
    ) -> Vec<ai_chat_domain::agent_identity::AgentProfile> {
        match self {
            Self::Memory(repo) => repo.list_owner_agents(human_user_id),
            Self::Postgres(repo) => repo.list_owner_agents(human_user_id),
        }
    }

    fn agent_exists(&self, agent_id: uuid::Uuid) -> bool {
        match self {
            Self::Memory(repo) => repo.agent_exists(agent_id),
            Self::Postgres(repo) => repo.agent_exists(agent_id),
        }
    }

    fn get_agent_profile(
        &self,
        agent_id: uuid::Uuid,
    ) -> Option<ai_chat_domain::agent_identity::AgentProfile> {
        match self {
            Self::Memory(repo) => repo.get_agent_profile(agent_id),
            Self::Postgres(repo) => repo.get_agent_profile(agent_id),
        }
    }

    fn list_agent_conversations(
        &self,
        agent_id: uuid::Uuid,
    ) -> Vec<ai_chat_domain::social::ConversationPreview> {
        match self {
            Self::Memory(repo) => repo.list_agent_conversations(agent_id),
            Self::Postgres(repo) => repo.list_agent_conversations(agent_id),
        }
    }

    fn list_company_conversations(
        &self,
        company_id: uuid::Uuid,
    ) -> Vec<ai_chat_domain::social::ConversationPreview> {
        match self {
            Self::Memory(repo) => repo.list_company_conversations(company_id),
            Self::Postgres(repo) => repo.list_company_conversations(company_id),
        }
    }

    fn get_conversation_messages(
        &self,
        conversation_id: uuid::Uuid,
    ) -> Vec<ai_chat_domain::social::MessageView> {
        match self {
            Self::Memory(repo) => repo.get_conversation_messages(conversation_id),
            Self::Postgres(repo) => repo.get_conversation_messages(conversation_id),
        }
    }

    fn get_conversation_message_page(
        &self,
        conversation_id: uuid::Uuid,
        before_message_id: Option<uuid::Uuid>,
        limit: usize,
    ) -> ai_chat_shared::AppResult<ai_chat_application::MessagePageView> {
        match self {
            Self::Memory(repo) => {
                repo.get_conversation_message_page(conversation_id, before_message_id, limit)
            }
            Self::Postgres(repo) => {
                repo.get_conversation_message_page(conversation_id, before_message_id, limit)
            }
        }
    }

    fn append_message(
        &self,
        message: ai_chat_domain::social::MessageView,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.append_message(message),
            Self::Postgres(repo) => repo.append_message(message),
        }
    }

    fn append_message_with_metadata(
        &self,
        message: ai_chat_domain::social::MessageView,
        content_json: serde_json::Value,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.append_message_with_metadata(message, content_json),
            Self::Postgres(repo) => repo.append_message_with_metadata(message, content_json),
        }
    }

    fn insert_post(&self, post: ai_chat_domain::social::PostView) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.insert_post(post),
            Self::Postgres(repo) => repo.insert_post(post),
        }
    }

    fn insert_post_comment(
        &self,
        comment: ai_chat_domain::social::PostCommentView,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.insert_post_comment(comment),
            Self::Postgres(repo) => repo.insert_post_comment(comment),
        }
    }

    fn insert_diary_entry(
        &self,
        diary_entry: ai_chat_domain::social::DiaryEntryView,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.insert_diary_entry(diary_entry),
            Self::Postgres(repo) => repo.insert_diary_entry(diary_entry),
        }
    }

    fn list_posts(&self) -> Vec<ai_chat_domain::social::PostView> {
        match self {
            Self::Memory(repo) => repo.list_posts(),
            Self::Postgres(repo) => repo.list_posts(),
        }
    }

    fn list_post_comments(
        &self,
        post_id: uuid::Uuid,
    ) -> Vec<ai_chat_domain::social::PostCommentView> {
        match self {
            Self::Memory(repo) => repo.list_post_comments(post_id),
            Self::Postgres(repo) => repo.list_post_comments(post_id),
        }
    }

    fn list_diary_entries(&self) -> Vec<ai_chat_domain::social::DiaryEntryView> {
        match self {
            Self::Memory(repo) => repo.list_diary_entries(),
            Self::Postgres(repo) => repo.list_diary_entries(),
        }
    }

    fn insert_friend_request(
        &self,
        request: ai_chat_domain::social::FriendRequestView,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.insert_friend_request(request),
            Self::Postgres(repo) => repo.insert_friend_request(request),
        }
    }

    fn list_friend_requests(
        &self,
        agent_id: uuid::Uuid,
    ) -> Vec<ai_chat_domain::social::FriendRequestView> {
        match self {
            Self::Memory(repo) => repo.list_friend_requests(agent_id),
            Self::Postgres(repo) => repo.list_friend_requests(agent_id),
        }
    }

    fn list_all_friend_requests(&self) -> Vec<ai_chat_domain::social::FriendRequestView> {
        match self {
            Self::Memory(repo) => repo.list_all_friend_requests(),
            Self::Postgres(repo) => repo.list_all_friend_requests(),
        }
    }

    fn list_friends(&self, agent_id: uuid::Uuid) -> Vec<ai_chat_domain::social::FriendSummary> {
        match self {
            Self::Memory(repo) => repo.list_friends(agent_id),
            Self::Postgres(repo) => repo.list_friends(agent_id),
        }
    }

    fn get_friend_request(
        &self,
        request_id: uuid::Uuid,
    ) -> Option<ai_chat_domain::social::FriendRequestView> {
        match self {
            Self::Memory(repo) => repo.get_friend_request(request_id),
            Self::Postgres(repo) => repo.get_friend_request(request_id),
        }
    }

    fn update_friend_request(
        &self,
        request: ai_chat_domain::social::FriendRequestView,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.update_friend_request(request),
            Self::Postgres(repo) => repo.update_friend_request(request),
        }
    }

    fn link_friends(
        &self,
        left_agent_id: uuid::Uuid,
        right_agent_id: uuid::Uuid,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.link_friends(left_agent_id, right_agent_id),
            Self::Postgres(repo) => repo.link_friends(left_agent_id, right_agent_id),
        }
    }

    fn get_friend_profile(
        &self,
        owner_agent_id: uuid::Uuid,
        friend_agent_id: uuid::Uuid,
    ) -> Option<ai_chat_domain::social::FriendProfileSnapshot> {
        match self {
            Self::Memory(repo) => repo.get_friend_profile(owner_agent_id, friend_agent_id),
            Self::Postgres(repo) => repo.get_friend_profile(owner_agent_id, friend_agent_id),
        }
    }

    fn upsert_friend_profile(
        &self,
        profile: ai_chat_domain::social::FriendProfileSnapshot,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.upsert_friend_profile(profile),
            Self::Postgres(repo) => repo.upsert_friend_profile(profile),
        }
    }

    fn find_direct_conversation(
        &self,
        left_agent_id: uuid::Uuid,
        right_agent_id: uuid::Uuid,
    ) -> Option<ai_chat_domain::social::ConversationPreview> {
        match self {
            Self::Memory(repo) => repo.find_direct_conversation(left_agent_id, right_agent_id),
            Self::Postgres(repo) => repo.find_direct_conversation(left_agent_id, right_agent_id),
        }
    }

    fn find_direct_conversation_peer(
        &self,
        conversation_id: uuid::Uuid,
        agent_id: uuid::Uuid,
    ) -> Option<uuid::Uuid> {
        match self {
            Self::Memory(repo) => repo.find_direct_conversation_peer(conversation_id, agent_id),
            Self::Postgres(repo) => repo.find_direct_conversation_peer(conversation_id, agent_id),
        }
    }

    fn conversation_exists(&self, conversation_id: uuid::Uuid) -> bool {
        match self {
            Self::Memory(repo) => repo.conversation_exists(conversation_id),
            Self::Postgres(repo) => repo.conversation_exists(conversation_id),
        }
    }

    fn list_agent_action_logs(
        &self,
        agent_id: uuid::Uuid,
        limit: usize,
    ) -> Vec<ai_chat_domain::agent_identity::AgentActionLog> {
        match self {
            Self::Memory(repo) => repo.list_agent_action_logs(agent_id, limit),
            Self::Postgres(repo) => repo.list_agent_action_logs(agent_id, limit),
        }
    }

    fn insert_problem_workspace(
        &self,
        workspace: ai_chat_application::ProblemWorkspace,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.insert_problem_workspace(workspace),
            Self::Postgres(repo) => repo.insert_problem_workspace(workspace),
        }
    }

    fn update_problem_workspace(
        &self,
        workspace: ai_chat_application::ProblemWorkspace,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.update_problem_workspace(workspace),
            Self::Postgres(repo) => repo.update_problem_workspace(workspace),
        }
    }

    fn get_problem_workspace(
        &self,
        workspace_id: uuid::Uuid,
    ) -> Option<ai_chat_application::ProblemWorkspace> {
        match self {
            Self::Memory(repo) => repo.get_problem_workspace(workspace_id),
            Self::Postgres(repo) => repo.get_problem_workspace(workspace_id),
        }
    }

    fn list_problem_workspaces(
        &self,
        agent_id: uuid::Uuid,
        limit: usize,
    ) -> Vec<ai_chat_application::ProblemWorkspace> {
        match self {
            Self::Memory(repo) => repo.list_problem_workspaces(agent_id, limit),
            Self::Postgres(repo) => repo.list_problem_workspaces(agent_id, limit),
        }
    }

    fn insert_problem_workspace_progress_record(
        &self,
        record: ai_chat_application::ProblemWorkspaceProgressRecord,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.insert_problem_workspace_progress_record(record),
            Self::Postgres(repo) => repo.insert_problem_workspace_progress_record(record),
        }
    }

    fn get_problem_workspace_progress_record(
        &self,
        record_id: uuid::Uuid,
    ) -> Option<ai_chat_application::ProblemWorkspaceProgressRecord> {
        match self {
            Self::Memory(repo) => repo.get_problem_workspace_progress_record(record_id),
            Self::Postgres(repo) => repo.get_problem_workspace_progress_record(record_id),
        }
    }

    fn update_problem_workspace_progress_record(
        &self,
        record: ai_chat_application::ProblemWorkspaceProgressRecord,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.update_problem_workspace_progress_record(record),
            Self::Postgres(repo) => repo.update_problem_workspace_progress_record(record),
        }
    }

    fn list_problem_workspace_progress_records(
        &self,
        workspace_id: uuid::Uuid,
        limit: usize,
    ) -> Vec<ai_chat_application::ProblemWorkspaceProgressRecord> {
        match self {
            Self::Memory(repo) => repo.list_problem_workspace_progress_records(workspace_id, limit),
            Self::Postgres(repo) => {
                repo.list_problem_workspace_progress_records(workspace_id, limit)
            }
        }
    }

    fn insert_problem_workspace_invitation(
        &self,
        invitation: ai_chat_application::ProblemWorkspaceInvitation,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.insert_problem_workspace_invitation(invitation),
            Self::Postgres(repo) => repo.insert_problem_workspace_invitation(invitation),
        }
    }

    fn get_problem_workspace_invitation(
        &self,
        invitation_id: uuid::Uuid,
    ) -> Option<ai_chat_application::ProblemWorkspaceInvitation> {
        match self {
            Self::Memory(repo) => repo.get_problem_workspace_invitation(invitation_id),
            Self::Postgres(repo) => repo.get_problem_workspace_invitation(invitation_id),
        }
    }

    fn update_problem_workspace_invitation(
        &self,
        invitation: ai_chat_application::ProblemWorkspaceInvitation,
    ) -> ai_chat_shared::AppResult<()> {
        match self {
            Self::Memory(repo) => repo.update_problem_workspace_invitation(invitation),
            Self::Postgres(repo) => repo.update_problem_workspace_invitation(invitation),
        }
    }

    fn list_problem_workspace_invitations(
        &self,
        agent_id: uuid::Uuid,
        workspace_id: Option<uuid::Uuid>,
        limit: usize,
    ) -> Vec<ai_chat_application::ProblemWorkspaceInvitation> {
        match self {
            Self::Memory(repo) => {
                repo.list_problem_workspace_invitations(agent_id, workspace_id, limit)
            }
            Self::Postgres(repo) => {
                repo.list_problem_workspace_invitations(agent_id, workspace_id, limit)
            }
        }
    }
}
