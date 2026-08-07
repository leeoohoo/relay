DROP INDEX IF EXISTS idx_agent_memories_active_session_topic;
DROP INDEX IF EXISTS idx_agent_memories_active_project_topic;
DROP INDEX IF EXISTS idx_agent_memories_active_control_topic;
DROP INDEX IF EXISTS idx_agent_memories_active_agent_topic;

ALTER TABLE agent_memories
    DROP CONSTRAINT IF EXISTS agent_memories_scoped_scope_check;

-- The legacy schema permits only one active/draft topic per Agent. Scoped
-- memories can legitimately reuse a topic across control and projects, so
-- archive the older duplicates before collapsing every scope back to agent.
WITH ranked_memories AS (
    SELECT id,
           ROW_NUMBER() OVER (
               PARTITION BY owner_agent_id, topic_key
               ORDER BY pinned DESC, updated_at DESC, id DESC
           ) AS topic_rank
    FROM agent_memories
    WHERE status IN ('draft', 'active')
)
UPDATE agent_memories
SET status = 'archived',
    updated_at = NOW()
WHERE id IN (
    SELECT id
    FROM ranked_memories
    WHERE topic_rank > 1
);

UPDATE agent_memories
SET scope = 'agent',
    project_id = CASE
        WHEN scope = 'project' THEN project_id
        WHEN scope = 'session' THEN (
            SELECT session.project_id
            FROM agent_codex_sessions AS session
            WHERE session.id = agent_memories.session_id
        )
        ELSE NULL
    END,
    session_id = NULL,
    visibility = 'both',
    injection_mode = CASE
        WHEN memory_tier = 'long_term' THEN 'always'
        ELSE 'on_demand'
    END;

ALTER TABLE agent_memories
    ADD CONSTRAINT agent_memories_scope_check
        CHECK (scope IN ('agent', 'project', 'company')),
    ADD CONSTRAINT agent_memories_agent_private_scope_check CHECK (scope = 'agent');

ALTER TABLE agent_memories
    DROP COLUMN visibility,
    DROP COLUMN injection_mode,
    DROP COLUMN session_id;

CREATE UNIQUE INDEX idx_agent_memories_active_agent_topic
    ON agent_memories(owner_agent_id, topic_key)
    WHERE scope = 'agent' AND status IN ('draft', 'active');

DROP TABLE IF EXISTS agent_execution_intents;

CREATE TABLE agent_codex_sessions_v1 (
    agent_profile_id UUID PRIMARY KEY REFERENCES agent_profiles(id) ON DELETE CASCADE,
    current_project_id UUID REFERENCES company_projects(id) ON DELETE SET NULL,
    codex_thread_id TEXT NOT NULL,
    worktree_key TEXT NOT NULL,
    last_used_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

INSERT INTO agent_codex_sessions_v1 (
    agent_profile_id, current_project_id, codex_thread_id, worktree_key, last_used_at
)
SELECT DISTINCT ON (agent_profile_id)
       agent_profile_id, project_id, codex_thread_id, workspace_key, last_used_at
FROM agent_codex_sessions
WHERE status = 'active'
ORDER BY agent_profile_id, last_used_at DESC;

DROP TABLE agent_codex_sessions;
ALTER TABLE agent_codex_sessions_v1 RENAME TO agent_codex_sessions;
