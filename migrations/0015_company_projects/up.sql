CREATE TABLE company_projects (
    id UUID PRIMARY KEY,
    company_id UUID NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'planned'
        CHECK (status IN ('planned', 'active', 'blocked', 'completed', 'cancelled')),
    owner_agent_id UUID NOT NULL REFERENCES agent_profiles(id) ON DELETE RESTRICT,
    project_group_conversation_id UUID NOT NULL UNIQUE REFERENCES conversations(id) ON DELETE RESTRICT,
    created_by_agent_id UUID NOT NULL REFERENCES agent_profiles(id) ON DELETE RESTRICT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ
);

CREATE INDEX idx_company_projects_company_status_updated
    ON company_projects(company_id, status, updated_at DESC);

ALTER TABLE conversations
    DROP CONSTRAINT conversations_context_type_check,
    ADD COLUMN project_id UUID REFERENCES company_projects(id) ON DELETE CASCADE,
    ADD CONSTRAINT conversations_context_type_check
        CHECK (context_type IN (
            'self_notes', 'external', 'company_direct', 'company_group', 'project_group'
        ));

CREATE UNIQUE INDEX idx_conversations_project_group
    ON conversations(project_id)
    WHERE context_type = 'project_group' AND project_id IS NOT NULL;

CREATE TABLE company_project_members (
    id UUID PRIMARY KEY,
    project_id UUID NOT NULL REFERENCES company_projects(id) ON DELETE CASCADE,
    agent_profile_id UUID NOT NULL REFERENCES agent_profiles(id) ON DELETE RESTRICT,
    role TEXT NOT NULL DEFAULT 'member' CHECK (role IN ('owner', 'member')),
    joined_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    left_at TIMESTAMPTZ,
    added_by_agent_id UUID NOT NULL REFERENCES agent_profiles(id) ON DELETE RESTRICT,
    UNIQUE (project_id, agent_profile_id)
);

CREATE INDEX idx_company_project_members_agent_active
    ON company_project_members(agent_profile_id, project_id)
    WHERE left_at IS NULL;

CREATE TABLE company_project_tasks (
    id UUID PRIMARY KEY,
    project_id UUID NOT NULL REFERENCES company_projects(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'todo'
        CHECK (status IN ('todo', 'in_progress', 'blocked', 'done', 'cancelled')),
    priority TEXT NOT NULL DEFAULT 'normal'
        CHECK (priority IN ('low', 'normal', 'high', 'urgent')),
    assignee_agent_id UUID REFERENCES agent_profiles(id) ON DELETE SET NULL,
    created_by_agent_id UUID NOT NULL REFERENCES agent_profiles(id) ON DELETE RESTRICT,
    due_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_company_project_tasks_project_status_updated
    ON company_project_tasks(project_id, status, updated_at DESC);

CREATE INDEX idx_company_project_tasks_assignee_status
    ON company_project_tasks(assignee_agent_id, status, updated_at DESC)
    WHERE assignee_agent_id IS NOT NULL;

CREATE TABLE company_project_status_updates (
    id UUID PRIMARY KEY,
    project_id UUID NOT NULL REFERENCES company_projects(id) ON DELETE CASCADE,
    author_agent_id UUID NOT NULL REFERENCES agent_profiles(id) ON DELETE RESTRICT,
    summary TEXT NOT NULL,
    progress_percent SMALLINT NOT NULL DEFAULT 0
        CHECK (progress_percent BETWEEN 0 AND 100),
    blockers JSONB NOT NULL DEFAULT '[]'::jsonb,
    next_steps JSONB NOT NULL DEFAULT '[]'::jsonb,
    project_status TEXT
        CHECK (project_status IS NULL OR project_status IN (
            'planned', 'active', 'blocked', 'completed', 'cancelled'
        )),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_company_project_status_updates_project_created
    ON company_project_status_updates(project_id, created_at DESC);
