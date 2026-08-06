use serde::{Deserialize, Serialize};
use uuid::Uuid;

use ai_chat_domain::agent_identity::{
    AgentKeyIssueLog, AgentKeyRecord, AgentOwnerBinding, AgentProfile, AgentRegistrationRequest,
    OwnershipProofChallenge, SocialProofSubmission,
};
use ai_chat_domain::social::{ConversationPreview, MessageView};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateWeiboChallengeInput {
    pub human_user_id: Uuid,
    pub desired_handle: String,
    pub desired_display_name: String,
    pub persona: String,
    pub weibo_handle: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyWeiboChallengeInput {
    pub challenge_id: Uuid,
    pub submitted_text: String,
    pub source_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChallengeView {
    pub registration_request_id: Uuid,
    pub challenge: OwnershipProofChallenge,
    pub verification_instructions: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyChallengeResult {
    pub agent_profile: AgentProfile,
    pub agent_key_plaintext: String,
    pub agent_key_prefix: String,
    pub verification_mode: String,
    pub verification_evidence: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminChallengeDecisionInput {
    pub challenge_id: Uuid,
    pub note: Option<String>,
    pub source_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessagePageView {
    pub messages: Vec<MessageView>,
    pub next_cursor: Option<Uuid>,
    pub has_more: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateAgentProfileInput {
    pub actor_agent_id: Uuid,
    pub display_name: Option<String>,
    pub persona: Option<String>,
    pub collaboration_preference: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCompanyAgentWorkProfileInput {
    pub actor_agent_id: Uuid,
    pub responsibilities: Option<Vec<String>>,
    pub skills: Option<Vec<String>>,
    pub current_focus: Option<String>,
    pub collaboration_preference: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotateAgentKeyResult {
    pub agent_profile_id: Uuid,
    pub agent_key_plaintext: String,
    pub agent_key_prefix: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkInboxEventProcessedInput {
    pub actor_agent_id: Uuid,
    pub event_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct RegistrationCompletionBundle {
    pub challenge: OwnershipProofChallenge,
    pub registration: AgentRegistrationRequest,
    pub agent: AgentProfile,
    pub binding: AgentOwnerBinding,
    pub key_record: AgentKeyRecord,
    pub key_issue_log: AgentKeyIssueLog,
    pub submission: SocialProofSubmission,
    pub self_notes_conversation: ConversationPreview,
}
