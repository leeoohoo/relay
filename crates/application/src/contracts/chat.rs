use serde::{Deserialize, Serialize};
use uuid::Uuid;

use ai_chat_domain::agent_identity::AgentInboxEvent;
use ai_chat_domain::company::{Company, CompanyAgentMembership, OrgUnit};
use ai_chat_domain::social::{
    ConversationContext, ConversationPreview, MessageAttachmentView, MessageView,
};

use super::{CompanyAgentView, CompanyProjectView};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyConversationView {
    pub preview: ConversationPreview,
    pub context: ConversationContext,
    pub member_agent_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyAgentContextView {
    pub company: Company,
    pub actor_membership: CompanyAgentMembership,
    pub org_units: Vec<OrgUnit>,
    pub agents: Vec<CompanyAgentView>,
    pub conversations: Vec<CompanyConversationView>,
    pub projects: Vec<CompanyProjectView>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetCompanyAgentContextInput {
    pub actor_agent_id: Uuid,
    pub company_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenCompanyDirectConversationInput {
    pub actor_agent_id: Uuid,
    pub company_id: Uuid,
    pub target_agent_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenHumanCompanyDirectConversationInput {
    pub human_user_id: Uuid,
    pub company_id: Uuid,
    pub target_agent_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCompanyGroupConversationInput {
    pub actor_agent_id: Uuid,
    pub company_id: Uuid,
    pub title: String,
    pub member_agent_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendCompanyMessageInput {
    pub actor_agent_id: Uuid,
    pub company_id: Uuid,
    pub conversation_id: Uuid,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendHumanCompanyMessageInput {
    pub human_user_id: Uuid,
    pub company_id: Uuid,
    pub conversation_id: Uuid,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendCompanyMessageWithMentionsInput {
    pub actor_agent_id: Uuid,
    pub company_id: Uuid,
    pub conversation_id: Uuid,
    pub content: String,
    pub mentioned_agent_ids: Vec<Uuid>,
    pub mention_all: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendHumanCompanyMessageWithMentionsInput {
    pub human_user_id: Uuid,
    pub company_id: Uuid,
    pub conversation_id: Uuid,
    pub content: String,
    pub mentioned_agent_ids: Vec<Uuid>,
    pub mention_all: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendHumanCompanyMessageWithAttachmentsInput {
    pub human_user_id: Uuid,
    pub company_id: Uuid,
    pub conversation_id: Uuid,
    pub content: String,
    pub mentioned_agent_ids: Vec<Uuid>,
    pub mention_all: bool,
    pub attachments: Vec<MessageAttachmentView>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplyCompanyInboxMessageInput {
    pub actor_agent_id: Uuid,
    pub event_id: Uuid,
    pub content: String,
    pub auto_ack: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplyCompanyInboxMessageResult {
    pub message: MessageView,
    pub event: AgentInboxEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListCompanyGroupUnreadInput {
    pub actor_agent_id: Uuid,
    pub company_id: Uuid,
    pub conversation_id: Option<Uuid>,
    /// Exclusive per-conversation cursor returned by the previous unread page.
    pub after_message_id: Option<Uuid>,
    pub message_limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkCompanyGroupReadInput {
    pub actor_agent_id: Uuid,
    pub company_id: Uuid,
    pub conversation_id: Uuid,
    /// Refuse the bulk read when this Agent still has an unread mention in the
    /// protected read window.
    pub only_if_no_mentions: bool,
    /// When quick-marking after reviewing a page, mentions at or before this
    /// message are treated as reviewed; any later mention still blocks it.
    pub reviewed_through_message_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyGroupUnreadView {
    pub conversation: CompanyConversationView,
    pub unread_count: usize,
    pub mention_count: usize,
    pub page_unread_count: usize,
    pub page_mention_count: usize,
    pub page_mentioned_message_ids: Vec<Uuid>,
    pub remaining_unread_count: usize,
    pub remaining_mention_count: usize,
    pub remaining_has_mentions: bool,
    pub can_quick_mark_read: bool,
    pub next_cursor: Option<Uuid>,
    pub has_more: bool,
    pub unread_messages: Vec<MessageView>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyGroupUnreadResult {
    pub total_unread_count: usize,
    pub total_mention_count: usize,
    pub has_unread_mentions: bool,
    pub groups: Vec<CompanyGroupUnreadView>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkCompanyGroupReadResult {
    pub conversation_id: Uuid,
    pub marked_read_count: usize,
    pub quick_mark_read: bool,
    pub read_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub struct CompanyConversationMemberPreview {
    pub agent_id: Uuid,
    pub preview: ConversationPreview,
}

#[derive(Debug, Clone)]
pub struct CompanyConversationCreationBundle {
    pub company_id: Uuid,
    pub context_type: String,
    pub visibility: String,
    pub created_by_agent_id: Uuid,
    pub members: Vec<CompanyConversationMemberPreview>,
    pub direct_pair: Option<(Uuid, Uuid)>,
}

#[derive(Debug, Clone)]
pub struct ProjectDiscussionThreadCreationBundle {
    pub thread: ai_chat_domain::company::ProjectDiscussionThread,
    pub company_id: Uuid,
    pub title: String,
    pub member_agent_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenProjectDiscussionThreadForHumanInput {
    pub human_user_id: Uuid,
    pub company_id: Uuid,
    pub project_id: Uuid,
    pub scope_type: String,
    pub subject_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct HumanCompanyDirectConversationCreationBundle {
    pub company_id: Uuid,
    pub human_user_id: Uuid,
    pub target_agent_id: Uuid,
    pub preview: ConversationPreview,
}
