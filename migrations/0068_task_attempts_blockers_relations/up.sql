CREATE TABLE project_task_attempts (
    id UUID PRIMARY KEY,
    project_id UUID NOT NULL REFERENCES company_projects(id) ON DELETE CASCADE,
    task_id UUID NOT NULL REFERENCES company_project_tasks(id) ON DELETE CASCADE,
    agent_id UUID NOT NULL REFERENCES agent_profiles(id),
    intent_id UUID NULL REFERENCES agent_execution_intents(id) ON DELETE SET NULL,
    attempt_number INTEGER NOT NULL CHECK (attempt_number > 0),
    attempt_type TEXT NOT NULL,
    status TEXT NOT NULL,
    objective TEXT NOT NULL,
    result_summary TEXT NULL,
    failure_category TEXT NULL,
    started_at TIMESTAMPTZ NULL,
    finished_at TIMESTAMPTZ NULL,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    UNIQUE (task_id, attempt_number)
);

CREATE UNIQUE INDEX idx_project_task_attempts_one_active
    ON project_task_attempts(task_id)
    WHERE status IN ('queued', 'running');
CREATE INDEX idx_project_task_attempts_task_recent
    ON project_task_attempts(task_id, created_at DESC);

CREATE TABLE project_task_blockers (
    id UUID PRIMARY KEY,
    project_id UUID NOT NULL REFERENCES company_projects(id) ON DELETE CASCADE,
    task_id UUID NOT NULL REFERENCES company_project_tasks(id) ON DELETE CASCADE,
    attempt_id UUID NULL REFERENCES project_task_attempts(id) ON DELETE SET NULL,
    blocker_type TEXT NOT NULL,
    status TEXT NOT NULL,
    summary TEXT NOT NULL,
    owner_agent_id UUID NULL REFERENCES agent_profiles(id),
    owner_human_user_id UUID NULL REFERENCES human_users(id),
    resolution_condition TEXT NOT NULL,
    resolution_summary TEXT NULL,
    resolved_at TIMESTAMPTZ NULL,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX idx_project_task_blockers_task_status
    ON project_task_blockers(task_id, status, created_at DESC);

CREATE TABLE project_task_relations (
    id UUID PRIMARY KEY,
    project_id UUID NOT NULL REFERENCES company_projects(id) ON DELETE CASCADE,
    source_task_id UUID NOT NULL REFERENCES company_project_tasks(id) ON DELETE CASCADE,
    target_task_id UUID NOT NULL REFERENCES company_project_tasks(id) ON DELETE CASCADE,
    relation_type TEXT NOT NULL,
    created_by_agent_id UUID NULL REFERENCES agent_profiles(id),
    created_by_human_user_id UUID NULL REFERENCES human_users(id),
    created_at TIMESTAMPTZ NOT NULL,
    CHECK (source_task_id <> target_task_id),
    UNIQUE (source_task_id, target_task_id, relation_type)
);

CREATE INDEX idx_project_task_relations_project
    ON project_task_relations(project_id, created_at DESC);
