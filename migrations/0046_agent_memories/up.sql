DO $$
BEGIN
    IF to_regclass('public.agent_memories') IS NOT NULL
       AND EXISTS (
           SELECT 1
           FROM information_schema.columns
           WHERE table_schema = 'public'
             AND table_name = 'agent_memories'
             AND column_name = 'agent_profile_id'
       )
       AND NOT EXISTS (
           SELECT 1
           FROM information_schema.columns
           WHERE table_schema = 'public'
             AND table_name = 'agent_memories'
             AND column_name = 'company_id'
       ) THEN
        ALTER TABLE agent_memories RENAME TO agent_memories_legacy_v1;
        ALTER TABLE agent_memories_legacy_v1
            RENAME CONSTRAINT agent_memories_pkey TO agent_memories_legacy_v1_pkey;
        IF to_regclass('public.idx_agent_memories_agent_profile_id_created_at') IS NOT NULL THEN
            ALTER INDEX idx_agent_memories_agent_profile_id_created_at
                RENAME TO idx_agent_memories_legacy_v1_agent_created_at;
        END IF;
    END IF;
END $$;

CREATE TABLE IF NOT EXISTS agent_memories (
    id UUID PRIMARY KEY,
    company_id UUID NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    owner_agent_id UUID NOT NULL REFERENCES agent_profiles(id) ON DELETE RESTRICT,
    scope TEXT NOT NULL CHECK (scope IN ('agent', 'project', 'company')),
    project_id UUID REFERENCES company_projects(id) ON DELETE CASCADE,
    memory_type TEXT NOT NULL CHECK (
        memory_type IN ('fact', 'decision', 'lesson', 'preference', 'procedure', 'relationship', 'handoff')
    ),
    topic_key TEXT NOT NULL,
    title TEXT NOT NULL,
    summary TEXT NOT NULL,
    when_to_use TEXT NOT NULL DEFAULT '',
    tags JSONB NOT NULL DEFAULT '[]'::jsonb,
    importance INTEGER NOT NULL DEFAULT 3 CHECK (importance BETWEEN 1 AND 5),
    confidence INTEGER NOT NULL DEFAULT 80 CHECK (confidence BETWEEN 0 AND 100),
    pinned BOOLEAN NOT NULL DEFAULT FALSE,
    status TEXT NOT NULL DEFAULT 'draft'
        CHECK (status IN ('draft', 'active', 'archived', 'superseded')),
    source_refs JSONB NOT NULL DEFAULT '[]'::jsonb,
    supersedes_memory_id UUID REFERENCES agent_memories(id) ON DELETE SET NULL,
    expires_at TIMESTAMPTZ,
    verified_by_agent_id UUID REFERENCES agent_profiles(id) ON DELETE SET NULL,
    verified_by_human_user_id UUID REFERENCES human_users(id) ON DELETE SET NULL,
    verified_at TIMESTAMPTZ,
    created_by_agent_id UUID REFERENCES agent_profiles(id) ON DELETE SET NULL,
    created_by_human_user_id UUID REFERENCES human_users(id) ON DELETE SET NULL,
    updated_by_agent_id UUID REFERENCES agent_profiles(id) ON DELETE SET NULL,
    updated_by_human_user_id UUID REFERENCES human_users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK ((scope = 'project') = (project_id IS NOT NULL)),
    CHECK (char_length(topic_key) BETWEEN 3 AND 120),
    CHECK (char_length(title) BETWEEN 1 AND 200),
    CHECK (char_length(summary) BETWEEN 10 AND 2000),
    CHECK (char_length(when_to_use) <= 1000),
    CHECK (jsonb_typeof(tags) = 'array'),
    CHECK (jsonb_typeof(source_refs) = 'array')
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_agent_memories_active_agent_topic
    ON agent_memories(owner_agent_id, topic_key)
    WHERE scope = 'agent' AND status IN ('draft', 'active');

CREATE UNIQUE INDEX IF NOT EXISTS idx_agent_memories_active_project_topic
    ON agent_memories(project_id, topic_key)
    WHERE scope = 'project' AND status IN ('draft', 'active');

CREATE UNIQUE INDEX IF NOT EXISTS idx_agent_memories_active_company_topic
    ON agent_memories(company_id, topic_key)
    WHERE scope = 'company' AND status IN ('draft', 'active');

CREATE INDEX IF NOT EXISTS idx_agent_memories_company_scope_status
    ON agent_memories(company_id, scope, status, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_agent_memories_owner_status
    ON agent_memories(owner_agent_id, status, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_agent_memories_project_status
    ON agent_memories(project_id, status, updated_at DESC)
    WHERE project_id IS NOT NULL;

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
