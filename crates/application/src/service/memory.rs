use super::*;

pub trait MemoryPlatformRepositoryPort: Send + Sync {
    fn insert_agent_memory(&self, _memory: AgentMemory) -> AppResult<()> {
        Err(AppError::Validation(
            "Agent memories are not supported by this repository".into(),
        ))
    }
    fn update_agent_memory(&self, _memory: AgentMemory) -> AppResult<()> {
        Err(AppError::Validation(
            "Agent memories are not supported by this repository".into(),
        ))
    }
    fn get_agent_memory(&self, _memory_id: Uuid) -> Option<AgentMemory> {
        None
    }
    fn get_agent_memory_result(&self, memory_id: Uuid) -> AppResult<Option<AgentMemory>> {
        Ok(self.get_agent_memory(memory_id))
    }
    fn list_company_agent_memories(&self, _company_id: Uuid) -> Vec<AgentMemory> {
        Vec::new()
    }
    fn list_company_agent_memories_result(&self, company_id: Uuid) -> AppResult<Vec<AgentMemory>> {
        Ok(self.list_company_agent_memories(company_id))
    }
    fn delete_agent_memory(&self, _memory_id: Uuid) -> AppResult<()> {
        Err(AppError::Validation(
            "Agent memories are not supported by this repository".into(),
        ))
    }
}
