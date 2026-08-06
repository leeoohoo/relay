use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

pub const AGENT_COLLABORATION_PREFERENCE_AVAILABLE: &str = "available";
pub const AGENT_COLLABORATION_PREFERENCE_LOW_COST_ONLY: &str = "low_cost_only";
pub const AGENT_COLLABORATION_PREFERENCE_UNAVAILABLE: &str = "unavailable";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HumanUser {
    pub id: Uuid,
    pub email: String,
    pub display_name: String,
    pub created_at: DateTime<Utc>,
}

pub const HUMAN_HARNESS_STATUS_PENDING: &str = "pending";
pub const HUMAN_HARNESS_STATUS_PROVISIONING: &str = "provisioning";
pub const HUMAN_HARNESS_STATUS_ACTIVE: &str = "active";
pub const HUMAN_HARNESS_STATUS_FAILED: &str = "failed";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HumanHarnessAccount {
    pub human_user_id: Uuid,
    pub provider_mode: String,
    pub harness_base_url: String,
    pub harness_uid: String,
    pub harness_email: String,
    pub space_identifier: String,
    pub status: String,
    pub attempt_count: i32,
    pub last_error: Option<String>,
    pub last_attempt_at: Option<DateTime<Utc>>,
    pub provisioned_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HumanCredential {
    pub human_user_id: Uuid,
    pub password_hash: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HumanSession {
    pub id: Uuid,
    pub human_user_id: Uuid,
    pub token_prefix: String,
    pub token_hash: String,
    pub expires_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HumanAccountToken {
    pub id: Uuid,
    pub human_user_id: Uuid,
    pub purpose: String,
    pub token_prefix: String,
    pub token_hash: String,
    pub expires_at: DateTime<Utc>,
    pub used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentProfile {
    pub id: Uuid,
    pub owner_user_id: Uuid,
    pub display_name: String,
    pub handle: String,
    pub persona: String,
    pub collaboration_preference: String,
    pub status: AgentStatus,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    PendingVerification,
    Active,
    Frozen,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentOwnerBinding {
    pub id: Uuid,
    pub human_user_id: Uuid,
    pub agent_profile_id: Uuid,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentKeyRecord {
    pub id: Uuid,
    pub agent_profile_id: Uuid,
    pub key_name: String,
    pub key_prefix: String,
    pub key_hash: String,
    pub last_used_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentKeyIssueType {
    Issued,
    Rotated,
    Revoked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentKeyIssueLog {
    pub id: Uuid,
    pub agent_profile_id: Uuid,
    pub agent_key_id: Option<Uuid>,
    pub issue_type: AgentKeyIssueType,
    pub issued_by_user_id: Option<Uuid>,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRegistrationRequest {
    pub id: Uuid,
    pub human_user_id: Uuid,
    pub desired_handle: String,
    pub desired_display_name: String,
    pub persona: String,
    pub weibo_handle: String,
    pub status: RegistrationStatus,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RegistrationStatus {
    PendingProof,
    Verified,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OwnershipProofChallenge {
    pub id: Uuid,
    pub human_user_id: Uuid,
    pub registration_request_id: Uuid,
    pub provider: OwnershipProofProvider,
    pub account_handle: String,
    pub verification_code: String,
    pub template_text: String,
    pub expires_at: DateTime<Utc>,
    pub status: ChallengeStatus,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OwnershipProofProvider {
    Weibo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChallengeStatus {
    Pending,
    Verified,
    Expired,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocialProofSubmission {
    pub id: Uuid,
    pub challenge_id: Uuid,
    pub submitted_text: String,
    pub source_url: Option<String>,
    pub provider_post_id: Option<String>,
    pub verification_mode: String,
    pub verification_evidence: Option<String>,
    pub raw_payload: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentActionStatus {
    Success,
    Failed,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentInboxEventStatus {
    Pending,
    Processing,
    Processed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInboxEvent {
    pub id: Uuid,
    pub agent_profile_id: Uuid,
    pub event_type: String,
    pub payload_json: Value,
    pub priority: i32,
    pub available_at: DateTime<Utc>,
    pub processed_at: Option<DateTime<Utc>>,
    pub status: AgentInboxEventStatus,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentActionLog {
    pub id: Uuid,
    pub agent_profile_id: Uuid,
    pub action_type: String,
    pub target_ref: Option<String>,
    pub request_payload: Value,
    pub result_payload: Value,
    pub status: AgentActionStatus,
    pub trace_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentIdempotencyRecord {
    pub id: Uuid,
    pub agent_profile_id: Uuid,
    pub operation: String,
    pub idempotency_key: String,
    pub request_hash: String,
    pub response_json: Value,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}
