use super::mapping::*;
use super::*;

impl MemoryPlatformRepositoryPort for PostgresPlatformRepository {
    fn insert_agent_memory(&self, memory: AgentMemory) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO agent_memories (
                    id, company_id, owner_agent_id, scope, project_id, session_id,
                    memory_tier, injection_mode, classification_reason, estimated_ttl_days,
                    injection_cost_chars, visibility, memory_type,
                    topic_key, title, summary, when_to_use, tags, importance, confidence,
                    pinned, status, source_refs, supersedes_memory_id, expires_at, archived_at,
                    verified_by_agent_id, verified_by_human_user_id, verified_at,
                    created_by_agent_id, created_by_human_user_id, updated_by_agent_id,
                    updated_by_human_user_id, created_at, updated_at
                )
                VALUES (
                    $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13,
                    $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24,
                    $25, $26, $27, $28, $29, $30, $31, $32, $33, $34, $35
                )
                "#,
                &[
                    &memory.id,
                    &memory.company_id,
                    &memory.owner_agent_id,
                    &memory.scope,
                    &memory.project_id,
                    &memory.session_id,
                    &memory.memory_tier,
                    &memory.injection_mode,
                    &memory.classification_reason,
                    &memory.estimated_ttl_days,
                    &memory.injection_cost_chars,
                    &memory.visibility,
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
                    &memory.archived_at,
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
                    classification_reason = $3,
                    estimated_ttl_days = $4,
                    injection_cost_chars = $5,
                    injection_mode = $6,
                    visibility = $7,
                    title = $8,
                    summary = $9,
                    when_to_use = $10,
                    tags = $11,
                    importance = $12,
                    confidence = $13,
                    pinned = $14,
                    status = $15,
                    source_refs = $16,
                    supersedes_memory_id = $17,
                    expires_at = $18,
                    archived_at = $19,
                    verified_by_agent_id = $20,
                    verified_by_human_user_id = $21,
                    verified_at = $22,
                    updated_by_agent_id = $23,
                    updated_by_human_user_id = $24,
                    updated_at = $25
                WHERE id = $1
                "#,
                &[
                    &memory.id,
                    &memory.memory_tier,
                    &memory.classification_reason,
                    &memory.estimated_ttl_days,
                    &memory.injection_cost_chars,
                    &memory.injection_mode,
                    &memory.visibility,
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
                    &memory.archived_at,
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
                SELECT id, company_id, owner_agent_id, scope, project_id, session_id,
                       memory_tier, injection_mode, classification_reason, estimated_ttl_days,
                       injection_cost_chars, visibility, memory_type,
                       topic_key, title, summary, when_to_use, tags, importance, confidence,
                       pinned, status, source_refs, supersedes_memory_id, expires_at, archived_at,
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
                SELECT id, company_id, owner_agent_id, scope, project_id, session_id,
                       memory_tier, injection_mode, classification_reason, estimated_ttl_days,
                       injection_cost_chars, visibility, memory_type,
                       topic_key, title, summary, when_to_use, tags, importance, confidence,
                       pinned, status, source_refs, supersedes_memory_id, expires_at, archived_at,
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

    fn archive_expired_agent_memories(
        &self,
        company_id: Uuid,
        now: DateTime<Utc>,
    ) -> AppResult<usize> {
        self.with_client(|client| {
            client.execute(
                "UPDATE agent_memories SET status = 'archived', archived_at = $2, updated_at = $2 WHERE company_id = $1 AND status = 'active' AND expires_at IS NOT NULL AND expires_at <= $2",
                &[&company_id, &now],
            )
        })
        .map(|count| count as usize)
    }
}
