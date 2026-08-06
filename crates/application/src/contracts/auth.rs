use serde::{Deserialize, Serialize};
use uuid::Uuid;

use ai_chat_domain::agent_identity::HumanUser;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevLoginInput {
    pub email: String,
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterHumanInput {
    pub email: String,
    pub display_name: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginHumanInput {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HumanAuthResult {
    pub user: HumanUser,
    pub session_token: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeHumanPasswordInput {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HumanSessionView {
    pub id: Uuid,
    pub token_prefix: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub last_used_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub is_current: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HumanAccountTokenResult {
    pub token: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResetHumanPasswordInput {
    pub token: String,
    pub new_password: String,
}
