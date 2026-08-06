use super::mapping::*;
use super::*;

impl MemoryPlatformRepositoryPort for PostgresPlatformRepository {
    fn insert_agent_memory(&self, memory: AgentMemory) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO agent_memories (
                    id, company_id, owner_agent_id, scope, project_id, memory_tier, memory_type,
                    topic_key, title, summary, when_to_use, tags, importance, confidence,
                    pinned, status, source_refs, supersedes_memory_id, expires_at,
                    verified_by_agent_id, verified_by_human_user_id, verified_at,
                    created_by_agent_id, created_by_human_user_id, updated_by_agent_id,
                    updated_by_human_user_id, created_at, updated_at
                )
                VALUES (
                    $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13,
                    $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24,
                    $25, $26, $27, $28
                )
                "#,
                &[
                    &memory.id,
                    &memory.company_id,
                    &memory.owner_agent_id,
                    &memory.scope,
                    &memory.project_id,
                    &memory.memory_tier,
                    &memory.memory_type,
                    &memory.topic_key,
                    &memory.title,
                    &memory.summary,
                    &memory.when_to_use,
                    &Json(memory.tags),
                    &memory.importance,
                    &memory.confidence,
                    &memory.pinned,
                    &memory.status,
                    &Json(memory.source_refs),
                    &memory.supersedes_memory_id,
                    &memory.expires_at,
                    &memory.verified_by_agent_id,
                    &memory.verified_by_human_user_id,
                    &memory.verified_at,
                    &memory.created_by_agent_id,
                    &memory.created_by_human_user_id,
                    &memory.updated_by_agent_id,
                    &memory.updated_by_human_user_id,
                    &memory.created_at,
                    &memory.updated_at,
                ],
            )?;
            Ok(())
        })
    }

    fn update_agent_memory(&self, memory: AgentMemory) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                UPDATE agent_memories
                SET memory_tier = $2,
                    title = $3,
                    summary = $4,
                    when_to_use = $5,
                    tags = $6,
                    importance = $7,
                    confidence = $8,
                    pinned = $9,
                    status = $10,
                    source_refs = $11,
                    supersedes_memory_id = $12,
                    expires_at = $13,
                    verified_by_agent_id = $14,
                    verified_by_human_user_id = $15,
                    verified_at = $16,
                    updated_by_agent_id = $17,
                    updated_by_human_user_id = $18,
                    updated_at = $19
                WHERE id = $1
                "#,
                &[
                    &memory.id,
                    &memory.memory_tier,
                    &memory.title,
                    &memory.summary,
                    &memory.when_to_use,
                    &Json(memory.tags),
                    &memory.importance,
                    &memory.confidence,
                    &memory.pinned,
                    &memory.status,
                    &Json(memory.source_refs),
                    &memory.supersedes_memory_id,
                    &memory.expires_at,
                    &memory.verified_by_agent_id,
                    &memory.verified_by_human_user_id,
                    &memory.verified_at,
                    &memory.updated_by_agent_id,
                    &memory.updated_by_human_user_id,
                    &memory.updated_at,
                ],
            )?;
            Ok(())
        })
    }

    fn get_agent_memory(&self, memory_id: Uuid) -> Option<AgentMemory> {
        self.get_agent_memory_result(memory_id).ok().flatten()
    }

    fn get_agent_memory_result(&self, memory_id: Uuid) -> AppResult<Option<AgentMemory>> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT id, company_id, owner_agent_id, scope, project_id, memory_tier, memory_type,
                       topic_key, title, summary, when_to_use, tags, importance, confidence,
                       pinned, status, source_refs, supersedes_memory_id, expires_at,
                       verified_by_agent_id, verified_by_human_user_id, verified_at,
                       created_by_agent_id, created_by_human_user_id, updated_by_agent_id,
                       updated_by_human_user_id, created_at, updated_at
                FROM agent_memories
                WHERE id = $1
                "#,
                &[&memory_id],
            )
        })
        .map(|row| row.map(map_agent_memory))
    }

    fn list_company_agent_memories(&self, company_id: Uuid) -> Vec<AgentMemory> {
        self.list_company_agent_memories_result(company_id)
            .unwrap_or_default()
    }

    fn list_company_agent_memories_result(&self, company_id: Uuid) -> AppResult<Vec<AgentMemory>> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT id, company_id, owner_agent_id, scope, project_id, memory_tier, memory_type,
                       topic_key, title, summary, when_to_use, tags, importance, confidence,
                       pinned, status, source_refs, supersedes_memory_id, expires_at,
                       verified_by_agent_id, verified_by_human_user_id, verified_at,
                       created_by_agent_id, created_by_human_user_id, updated_by_agent_id,
                       updated_by_human_user_id, created_at, updated_at
                FROM agent_memories
                WHERE company_id = $1
                ORDER BY pinned DESC, importance DESC, updated_at DESC
                "#,
                &[&company_id],
            )
        })
        .map(|rows| rows.into_iter().map(map_agent_memory).collect())
    }

    fn delete_agent_memory(&self, memory_id: Uuid) -> AppResult<()> {
        self.with_client(|client| {
            client.execute("DELETE FROM agent_memories WHERE id = $1", &[&memory_id])?;
            Ok(())
        })
    }
}
