CREATE TABLE agent_codex_sessions_v2 (
    id UUID PRIMARY KEY,
    agent_profile_id UUID NOT NULL REFERENCES agent_profiles(id) ON DELETE CASCADE,
    session_kind TEXT NOT NULL CHECK (session_kind IN ('control', 'project')),
    scope_key TEXT NOT NULL,
    project_id UUID REFERENCES company_projects(id) ON DELETE CASCADE,
    generation INTEGER NOT NULL DEFAULT 1 CHECK (generation >= 1),
    codex_thread_id TEXT NOT NULL,
    workspace_key TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'archived')),
    summary_short TEXT NOT NULL DEFAULT '',
    checkpoint_json JSONB NOT NULL DEFAULT '{}'::jsonb,
    skill_bundle_version TEXT NOT NULL DEFAULT '',
    memory_snapshot_version TEXT NOT NULL DEFAULT '',
    policy_version TEXT NOT NULL DEFAULT '',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_used_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    archived_at TIMESTAMPTZ,
    CHECK ((session_kind = 'project') = (project_id IS NOT NULL)),
    CHECK (char_length(scope_key) BETWEEN 1 AND 200),
    CHECK (char_length(summary_short) <= 1000),
    CHECK (jsonb_typeof(checkpoint_json) = 'object')
);

INSERT INTO agent_codex_sessions_v2 (
    id, agent_profile_id, session_kind, scope_key, project_id, generation,
    codex_thread_id, workspace_key, status, policy_version,
    created_at, last_used_at
)
SELECT gen_random_uuid(),
       agent_profile_id,
       CASE WHEN current_project_id IS NULL THEN 'control' ELSE 'project' END,
       CASE
           WHEN current_project_id IS NULL THEN 'control'
           ELSE 'project:' || current_project_id::text
       END,
       current_project_id,
       1,
       codex_thread_id,
       worktree_key,
       'active',
       split_part(worktree_key, ':', 1),
       last_used_at,
       last_used_at
FROM agent_codex_sessions;

DROP TABLE agent_codex_sessions;
ALTER TABLE agent_codex_sessions_v2 RENAME TO agent_codex_sessions;

CREATE UNIQUE INDEX idx_agent_codex_sessions_scope_generation
    ON agent_codex_sessions(agent_profile_id, scope_key, generation);

CREATE UNIQUE INDEX idx_agent_codex_sessions_active_scope
    ON agent_codex_sessions(agent_profile_id, scope_key)
    WHERE status = 'active';

CREATE INDEX idx_agent_codex_sessions_agent_recent
    ON agent_codex_sessions(agent_profile_id, last_used_at DESC);

CREATE INDEX idx_agent_codex_sessions_project_recent
    ON agent_codex_sessions(project_id, last_used_at DESC)
    WHERE project_id IS NOT NULL;

CREATE TABLE agent_execution_intents (
    id UUID PRIMARY KEY,
    company_id UUID NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    agent_profile_id UUID NOT NULL REFERENCES agent_profiles(id) ON DELETE CASCADE,
    project_id UUID NOT NULL REFERENCES company_projects(id) ON DELETE CASCADE,
    worker_session_id UUID REFERENCES agent_codex_sessions(id) ON DELETE SET NULL,
    source_event_ids JSONB NOT NULL DEFAULT '[]'::jsonb,
    task_ids JSONB NOT NULL DEFAULT '[]'::jsonb,
    action_type TEXT NOT NULL DEFAULT 'execute'
        CHECK (action_type IN ('execute', 'coordinate', 'reply', 'no_action')),
    objective TEXT NOT NULL,
    acceptance_criteria JSONB NOT NULL DEFAULT '[]'::jsonb,
    priority TEXT NOT NULL DEFAULT 'normal'
        CHECK (priority IN ('low', 'normal', 'high', 'urgent')),
    dedupe_key TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'running', 'completed', 'failed', 'cancelled')),
    result_summary TEXT NOT NULL DEFAULT '',
    error_message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    claimed_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    CHECK (jsonb_typeof(source_event_ids) = 'array'),
    CHECK (jsonb_typeof(task_ids) = 'array'),
    CHECK (jsonb_typeof(acceptance_criteria) = 'array'),
    CHECK (char_length(objective) BETWEEN 1 AND 4000),
    CHECK (char_length(dedupe_key) BETWEEN 3 AND 200),
    CHECK (char_length(result_summary) <= 4000)
);

CREATE UNIQUE INDEX idx_agent_execution_intents_dedupe
    ON agent_execution_intents(agent_profile_id, dedupe_key);

CREATE INDEX idx_agent_execution_intents_pending
    ON agent_execution_intents(agent_profile_id, status, created_at)
    WHERE status IN ('pending', 'running');

CREATE INDEX idx_agent_execution_intents_project_recent
    ON agent_execution_intents(project_id, created_at DESC);

ALTER TABLE agent_memories
    ADD COLUMN session_id UUID REFERENCES agent_codex_sessions(id) ON DELETE CASCADE,
    ADD COLUMN injection_mode TEXT NOT NULL DEFAULT 'on_demand'
        CHECK (injection_mode IN ('always', 'on_demand', 'never')),
    ADD COLUMN visibility TEXT NOT NULL DEFAULT 'both'
        CHECK (visibility IN ('control', 'worker', 'both'));

UPDATE agent_memories
SET injection_mode = CASE
        WHEN memory_tier = 'long_term' THEN 'always'
        ELSE 'on_demand'
    END,
    visibility = 'both';

ALTER TABLE agent_memories
    DROP CONSTRAINT IF EXISTS agent_memories_agent_private_scope_check;

ALTER TABLE agent_memories
    ADD CONSTRAINT agent_memories_scoped_scope_check CHECK (
        (scope = 'agent' AND project_id IS NULL AND session_id IS NULL)
        OR (scope = 'control' AND project_id IS NULL AND session_id IS NULL)
        OR (scope = 'project' AND project_id IS NOT NULL AND session_id IS NULL)
        OR (scope = 'session' AND project_id IS NULL AND session_id IS NOT NULL)
    );

DROP INDEX IF EXISTS idx_agent_memories_active_agent_topic;

CREATE UNIQUE INDEX idx_agent_memories_active_agent_topic
    ON agent_memories(owner_agent_id, topic_key)
    WHERE scope = 'agent' AND status IN ('draft', 'active');

CREATE UNIQUE INDEX idx_agent_memories_active_control_topic
    ON agent_memories(owner_agent_id, topic_key)
    WHERE scope = 'control' AND status IN ('draft', 'active');

CREATE UNIQUE INDEX idx_agent_memories_active_project_topic
    ON agent_memories(owner_agent_id, project_id, topic_key)
    WHERE scope = 'project' AND status IN ('draft', 'active');

CREATE UNIQUE INDEX idx_agent_memories_active_session_topic
    ON agent_memories(owner_agent_id, session_id, topic_key)
    WHERE scope = 'session' AND status IN ('draft', 'active');
