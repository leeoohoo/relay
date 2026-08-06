ALTER TABLE agent_memories
    ADD COLUMN IF NOT EXISTS memory_tier TEXT NOT NULL DEFAULT 'short_term'
    CHECK (memory_tier IN ('short_term', 'long_term'));

WITH ranked AS (
    SELECT id,
           ROW_NUMBER() OVER (
               PARTITION BY owner_agent_id, topic_key
               ORDER BY (status = 'active') DESC, updated_at DESC, id DESC
           ) AS row_number
    FROM agent_memories
    WHERE status IN ('draft', 'active')
)
UPDATE agent_memories AS memory
SET status = 'archived',
    updated_at = NOW()
FROM ranked
WHERE memory.id = ranked.id
  AND ranked.row_number > 1;

DO $$
DECLARE
    constraint_name TEXT;
BEGIN
    FOR constraint_name IN
        SELECT conname
        FROM pg_constraint
        WHERE conrelid = 'agent_memories'::regclass
          AND contype = 'c'
          AND pg_get_constraintdef(oid) ILIKE '%scope%project_id%'
    LOOP
        EXECUTE format('ALTER TABLE agent_memories DROP CONSTRAINT %I', constraint_name);
    END LOOP;
END $$;

UPDATE agent_memories
SET scope = 'agent',
    status = CASE WHEN status = 'draft' THEN 'active' ELSE status END,
    verified_by_agent_id = CASE
        WHEN status = 'draft' THEN owner_agent_id
        ELSE verified_by_agent_id
    END,
    verified_at = CASE
        WHEN status = 'draft' THEN COALESCE(verified_at, NOW())
        ELSE verified_at
    END,
    updated_at = NOW()
WHERE scope <> 'agent' OR status = 'draft';

ALTER TABLE agent_memories
    ADD CONSTRAINT agent_memories_agent_private_scope_check CHECK (scope = 'agent');

DROP INDEX IF EXISTS idx_agent_memories_active_project_topic;
DROP INDEX IF EXISTS idx_agent_memories_active_company_topic;

CREATE INDEX IF NOT EXISTS idx_agent_memories_owner_tier_status
    ON agent_memories(owner_agent_id, memory_tier, status, pinned DESC, importance DESC, updated_at DESC);

UPDATE company_agent_memberships
SET permissions = permissions - 'project.memory.manage' - 'company.memory.manage'
WHERE permissions ?| ARRAY['project.memory.manage', 'company.memory.manage'];
