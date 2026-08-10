pub use crate::contracts::*;
use ai_chat_domain::{agent_identity::*, company::*, social::*};
use ai_chat_shared::{AppError, AppResult};

fn cursor_page_by_id<T>(
    items: Vec<T>,
    after_id: Option<Uuid>,
    limit: usize,
    id: impl Fn(&T) -> Uuid,
) -> AppResult<CursorPage<T>> {
    let limit = limit.clamp(1, 100);
    let start = match after_id {
        Some(cursor) => items
            .iter()
            .position(|item| id(item) == cursor)
            .map(|index| index + 1)
            .ok_or_else(|| AppError::Validation("cursor does not belong to this list".into()))?,
        None => 0,
    };
    let mut page_items = items
        .into_iter()
        .skip(start)
        .take(limit + 1)
        .collect::<Vec<_>>();
    let has_more = page_items.len() > limit;
    if has_more {
        page_items.pop();
    }
    let next_cursor = has_more.then(|| page_items.last().map(&id)).flatten();
    Ok(CursorPage {
        items: page_items,
        next_cursor,
        has_more,
    })
}
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
