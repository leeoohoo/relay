use super::*;

impl MemoryPlatformRepositoryPort for MemoryPlatformRepository {
    fn insert_agent_memory(&self, memory: AgentMemory) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if guard.agent_memories.contains_key(&memory.id) {
            return Err(ai_chat_shared::AppError::Conflict(
                "Agent memory already exists".into(),
            ));
        }
        let duplicate = guard.agent_memories.values().any(|existing| {
            existing.company_id == memory.company_id
                && existing.scope == memory.scope
                && existing.topic_key == memory.topic_key
                && matches!(existing.status.as_str(), "draft" | "active")
                && match memory.scope.as_str() {
                    "agent" => existing.owner_agent_id == memory.owner_agent_id,
                    "project" => existing.project_id == memory.project_id,
                    "company" => true,
                    _ => false,
                }
        });
        if duplicate {
            return Err(ai_chat_shared::AppError::Conflict(
                "an active memory already exists for this topic".into(),
            ));
        }
        guard.agent_memories.insert(memory.id, memory);
        Ok(())
    }

    fn update_agent_memory(&self, memory: AgentMemory) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if !guard.agent_memories.contains_key(&memory.id) {
            return Err(ai_chat_shared::AppError::NotFound(
                "Agent memory not found".into(),
            ));
        }
        guard.agent_memories.insert(memory.id, memory);
        Ok(())
    }

    fn get_agent_memory(&self, memory_id: Uuid) -> Option<AgentMemory> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard.agent_memories.get(&memory_id).cloned()
    }

    fn list_company_agent_memories(&self, company_id: Uuid) -> Vec<AgentMemory> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut memories = guard
            .agent_memories
            .values()
            .filter(|memory| memory.company_id == company_id)
            .cloned()
            .collect::<Vec<_>>();
        memories.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        memories
    }

    fn archive_expired_agent_memories(
        &self,
        company_id: Uuid,
        now: DateTime<Utc>,
    ) -> AppResult<usize> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let mut archived = 0;
        for memory in guard.agent_memories.values_mut() {
            if memory.company_id == company_id
                && memory.status == AGENT_MEMORY_STATUS_ACTIVE
                && memory
                    .expires_at
                    .is_some_and(|expires_at| expires_at <= now)
            {
                memory.status = AGENT_MEMORY_STATUS_ARCHIVED.into();
                memory.archived_at = Some(now);
                memory.updated_at = now;
                archived += 1;
            }
        }
        Ok(archived)
    }

    fn delete_agent_memory(&self, memory_id: Uuid) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        guard.agent_memories.remove(&memory_id);
        Ok(())
    }
}
