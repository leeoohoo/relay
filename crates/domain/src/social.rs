use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const CONVERSATION_CONTEXT_SELF_NOTES: &str = "self_notes";
pub const CONVERSATION_CONTEXT_EXTERNAL: &str = "external";
pub const CONVERSATION_CONTEXT_COMPANY_DIRECT: &str = "company_direct";
pub const CONVERSATION_CONTEXT_COMPANY_GROUP: &str = "company_group";
pub const CONVERSATION_CONTEXT_COMPANY_ALL: &str = "company_all";
pub const CONVERSATION_CONTEXT_PROJECT_GROUP: &str = "project_group";
pub const CONVERSATION_CONTEXT_TASK_THREAD: &str = "task_thread";
pub const CONVERSATION_CONTEXT_BLOCKER_THREAD: &str = "blocker_thread";
pub const CONVERSATION_CONTEXT_GATE_THREAD: &str = "gate_thread";

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
pub struct MessageAttachmentView {
    pub id: Uuid,
    pub kind: String,
    pub file_name: String,
    pub relative_path: Option<String>,
    pub content_type: String,
    pub byte_size: i64,
    pub local_path: Option<String>,
    #[serde(default)]
    pub directory_entries: Vec<String>,
    #[serde(default)]
    pub purpose: Option<String>,
    #[serde(default)]
    pub storage_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageView {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub sender_agent_id: Option<Uuid>,
    pub sender_human_user_id: Option<Uuid>,
    pub content: String,
    #[serde(default)]
    pub attachments: Vec<MessageAttachmentView>,
    pub created_at: DateTime<Utc>,
}
