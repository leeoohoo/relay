ALTER TABLE agent_memories
    DROP CONSTRAINT IF EXISTS agent_memories_agent_private_scope_check;

DROP INDEX IF EXISTS idx_agent_memories_owner_tier_status;

UPDATE agent_memories
SET scope = CASE WHEN project_id IS NULL THEN 'agent' ELSE 'project' END,
    updated_at = NOW();

WITH ranked AS (
    SELECT id,
           ROW_NUMBER() OVER (
               PARTITION BY project_id, topic_key
               ORDER BY (status = 'active') DESC, updated_at DESC, id DESC
           ) AS row_number
    FROM agent_memories
    WHERE scope = 'project'
      AND status IN ('draft', 'active')
)
UPDATE agent_memories AS memory
SET status = 'archived',
    updated_at = NOW()
FROM ranked
WHERE memory.id = ranked.id
  AND ranked.row_number > 1;

CREATE UNIQUE INDEX IF NOT EXISTS idx_agent_memories_active_project_topic
    ON agent_memories(project_id, topic_key)
    WHERE scope = 'project' AND status IN ('draft', 'active');

CREATE UNIQUE INDEX IF NOT EXISTS idx_agent_memories_active_company_topic
    ON agent_memories(company_id, topic_key)
    WHERE scope = 'company' AND status IN ('draft', 'active');

ALTER TABLE agent_memories
    ADD CONSTRAINT agent_memories_project_scope_check
    CHECK ((scope = 'project') = (project_id IS NOT NULL));

ALTER TABLE agent_memories DROP COLUMN IF EXISTS memory_tier;

UPDATE company_agent_memberships
SET permissions = permissions || '["project.memory.manage"]'::jsonb
WHERE (
        role_key = 'company_manager'
        OR LOWER(BTRIM(job_title)) IN (
            '项目经理', '产品经理', '技术经理',
            'project manager', 'product manager', 'technical manager',
            'engineering manager', 'tech lead'
        )
    )
  AND NOT permissions @> '["project.memory.manage"]'::jsonb;

UPDATE company_agent_memberships
SET permissions = permissions || '["company.memory.manage"]'::jsonb
WHERE role_key = 'company_manager'
  AND NOT permissions @> '["company.memory.manage"]'::jsonb;
