use uuid::Uuid;

use ai_chat_domain::company::{AgentCodexTriggerConfig, AgentCodexTriggerRun};
use ai_chat_shared::AppResult;

use crate::CodexRepository;

pub struct CodexService<'a, R: CodexRepository> {
    repository: &'a R,
}

impl<'a, R: CodexRepository> CodexService<'a, R> {
    pub(crate) fn new(repository: &'a R) -> Self {
        Self { repository }
    }
    pub fn trigger(&self, agent_id: Uuid) -> AppResult<Option<AgentCodexTriggerConfig>> {
        self.repository.agent_trigger(agent_id)
    }
    pub fn runs(&self, agent_id: Uuid, limit: usize) -> AppResult<Vec<AgentCodexTriggerRun>> {
        self.repository.agent_runs(agent_id, limit)
    }
}
