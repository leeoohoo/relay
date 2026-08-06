use uuid::Uuid;

use ai_chat_domain::company::AgentMemory;
use ai_chat_shared::AppResult;

use crate::MemoryRepository;

pub struct MemoryService<'a, R: MemoryRepository> {
    repository: &'a R,
}

impl<'a, R: MemoryRepository> MemoryService<'a, R> {
    pub(crate) fn new(repository: &'a R) -> Self {
        Self { repository }
    }
    pub fn get(&self, memory_id: Uuid) -> AppResult<Option<AgentMemory>> {
        self.repository.memory(memory_id)
    }
}
