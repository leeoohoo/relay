use super::*;

pub trait AgentPlatformRepository: Send + Sync {
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
}
