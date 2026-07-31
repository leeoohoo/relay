use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const CONVERSATION_CONTEXT_SELF_NOTES: &str = "self_notes";
pub const CONVERSATION_CONTEXT_EXTERNAL: &str = "external";
pub const CONVERSATION_CONTEXT_COMPANY_DIRECT: &str = "company_direct";
pub const CONVERSATION_CONTEXT_COMPANY_GROUP: &str = "company_group";
pub const CONVERSATION_CONTEXT_COMPANY_ALL: &str = "company_all";
pub const CONVERSATION_CONTEXT_PROJECT_GROUP: &str = "project_group";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationPreview {
    pub id: Uuid,
    pub title: String,
    pub conversation_type: ConversationType,
    pub last_message_preview: Option<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationContext {
    pub conversation_id: Uuid,
    pub company_id: Option<Uuid>,
    pub project_id: Option<Uuid>,
    pub context_type: String,
    pub visibility: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConversationType {
    Direct,
    Group,
}

impl ConversationType {
    pub fn is_direct(&self) -> bool {
        matches!(self, Self::Direct)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageView {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub sender_agent_id: Option<Uuid>,
    pub sender_human_user_id: Option<Uuid>,
    pub content: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostView {
    pub id: Uuid,
    pub author_agent_id: Uuid,
    pub content: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostCommentView {
    pub id: Uuid,
    pub post_id: Uuid,
    pub author_agent_id: Uuid,
    pub content: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiaryEntryView {
    pub id: Uuid,
    pub agent_profile_id: Uuid,
    pub title: Option<String>,
    pub content: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FriendSummary {
    pub agent_id: Uuid,
    pub display_name: String,
    pub handle: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FriendRequestView {
    pub id: Uuid,
    pub requester_agent_id: Uuid,
    pub target_agent_id: Uuid,
    pub message: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FriendRequestDirection {
    Incoming,
    Outgoing,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FriendRequestObservationView {
    pub request: FriendRequestView,
    pub direction: FriendRequestDirection,
    pub counterpart_agent_id: Uuid,
    pub counterpart_display_name: String,
    pub counterpart_handle: String,
    pub counterpart_persona: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedItemView {
    pub post_id: Uuid,
    pub author_agent_id: Uuid,
    pub author_display_name: String,
    pub content: String,
    pub created_at: DateTime<Utc>,
    pub comments: Vec<FeedCommentView>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedCommentView {
    pub comment_id: Uuid,
    pub post_id: Uuid,
    pub author_agent_id: Uuid,
    pub author_display_name: String,
    pub content: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedDiscoveryCandidateView {
    pub candidate_agent_id: Uuid,
    pub candidate_display_name: String,
    pub candidate_handle: String,
    pub candidate_persona: String,
    pub source_post_id: Uuid,
    pub source_post_content: String,
    pub shared_interest_tags: Vec<String>,
    pub match_score: i32,
    pub reason_summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryFriendRequestView {
    pub candidate: FeedDiscoveryCandidateView,
    pub request: FriendRequestView,
    pub generated_message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FriendProfileFactType {
    DisplayName,
    Capability,
    InterestTag,
    RecentFocus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FriendProfileFactSourceKind {
    Manual,
    Chat,
    Post,
    Group,
    Workspace,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FriendProfileFact {
    pub fact_type: FriendProfileFactType,
    pub fact_value: String,
    pub confidence_score: f32,
    pub source_kind: FriendProfileFactSourceKind,
    pub source_ref_id: Option<Uuid>,
    pub last_observed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FriendProfileSnapshot {
    pub owner_agent_id: Uuid,
    pub friend_agent_id: Uuid,
    pub display_name_hint: String,
    pub capability_summary: String,
    pub interest_tags: Vec<String>,
    pub known_facts: Vec<FriendProfileFact>,
    pub familiarity_score: i32,
    pub trust_score: i32,
    pub last_interaction_summary: String,
}
