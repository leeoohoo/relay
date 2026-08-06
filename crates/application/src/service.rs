pub use crate::contracts::*;
use ai_chat_domain::{agent_identity::*, company::*, social::*};
use ai_chat_shared::{AppError, AppResult};
use chrono::{DateTime, Utc};
use serde_json::Value;
use uuid::Uuid;

mod agent;
mod auth;
mod chat;
mod codex;
mod company;
mod governance;
mod memory;
mod project;
mod task;

pub use agent::AgentPlatformRepository;
pub use auth::AuthPlatformRepository;
pub use chat::ChatPlatformRepository;
pub use codex::{
    CodexControlPlatformRepository, CodexPlatformRepository, CodexRuntimePlatformRepository,
};
pub use company::CompanyPlatformRepository;
pub use governance::GovernancePlatformRepository;
pub use memory::MemoryPlatformRepositoryPort;
pub use project::ProjectPlatformRepository;
pub use task::TaskPlatformRepository;

pub trait PlatformRepository:
    Clone
    + Send
    + Sync
    + 'static
    + AuthPlatformRepository
    + CompanyPlatformRepository
    + AgentPlatformRepository
    + ChatPlatformRepository
    + ProjectPlatformRepository
    + MemoryPlatformRepositoryPort
    + CodexPlatformRepository
    + TaskPlatformRepository
    + GovernancePlatformRepository
{
}

impl<T> PlatformRepository for T where
    T: Clone
        + Send
        + Sync
        + 'static
        + AuthPlatformRepository
        + CompanyPlatformRepository
        + AgentPlatformRepository
        + ChatPlatformRepository
        + ProjectPlatformRepository
        + MemoryPlatformRepositoryPort
        + CodexPlatformRepository
        + TaskPlatformRepository
        + GovernancePlatformRepository
{
}

pub use crate::platform::PlatformApp;
#[cfg(test)]
#[path = "service_tests/mod.rs"]
mod tests;
