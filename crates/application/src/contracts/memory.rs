use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use ai_chat_domain::company::{AgentMemory, AgentMemorySourceRef};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RememberAgentMemoryInput {
    pub actor_agent_id: Uuid,
    pub company_id: Uuid,
    pub project_id: Option<Uuid>,
    pub memory_tier: String,
    pub memory_type: String,
    pub topic_key: String,
    pub title: String,
    pub summary: String,
    pub when_to_use: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub importance: Option<i32>,
    pub confidence: Option<i32>,
    #[serde(default)]
    pub source_refs: Vec<AgentMemorySourceRef>,
    pub expires_at: Option<DateTime<Utc>>,
    pub supersedes_memory_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchAgentMemoriesInput {
    pub actor_agent_id: Uuid,
    pub company_id: Uuid,
    pub project_id: Option<Uuid>,
    pub query: Option<String>,
    #[serde(default)]
    pub memory_tiers: Vec<String>,
    #[serde(default)]
    pub memory_types: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub status: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetAgentMemoryInput {
    pub actor_agent_id: Uuid,
    pub company_id: Uuid,
    pub memory_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateAgentMemoryInput {
    pub actor_agent_id: Uuid,
    pub company_id: Uuid,
    pub memory_id: Uuid,
    pub memory_tier: Option<String>,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub when_to_use: Option<String>,
    pub tags: Option<Vec<String>>,
    pub importance: Option<i32>,
    pub confidence: Option<i32>,
    pub expires_at: Option<DateTime<Utc>>,
    pub clear_expires_at: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetAgentMemoryStateInput {
    pub actor_agent_id: Uuid,
    pub company_id: Uuid,
    pub memory_id: Uuid,
    pub status: Option<String>,
    pub pinned: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMemoryOverview {
    pub active_count: usize,
    pub short_term_count: usize,
    pub long_term_count: usize,
    pub long_term: Vec<AgentMemory>,
    pub pinned: Vec<AgentMemory>,
    pub recent: Vec<AgentMemory>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListCompanyMemoriesForHumanInput {
    pub human_user_id: Uuid,
    pub company_id: Uuid,
    pub owner_agent_id: Option<Uuid>,
    pub project_id: Option<Uuid>,
    pub memory_tier: Option<String>,
    pub status: Option<String>,
    pub query: Option<String>,
    pub limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateAgentMemoryForHumanInput {
    pub human_user_id: Uuid,
    pub company_id: Uuid,
    pub memory_id: Uuid,
    pub memory_tier: Option<String>,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub when_to_use: Option<String>,
    pub tags: Option<Vec<String>>,
    pub importance: Option<i32>,
    pub confidence: Option<i32>,
    pub status: Option<String>,
    pub pinned: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteAgentMemoryForHumanInput {
    pub human_user_id: Uuid,
    pub company_id: Uuid,
    pub memory_id: Uuid,
}
