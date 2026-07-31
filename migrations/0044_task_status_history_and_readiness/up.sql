CREATE TABLE company_project_task_status_history (
    id UUID PRIMARY KEY,
    project_id UUID NOT NULL REFERENCES company_projects(id) ON DELETE CASCADE,
    task_id UUID NOT NULL,
    from_status TEXT,
    to_status TEXT NOT NULL,
    changed_by_agent_id UUID REFERENCES agent_profiles(id) ON DELETE SET NULL,
    changed_by_human_user_id UUID REFERENCES human_users(id) ON DELETE SET NULL,
    change_source TEXT NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY (task_id, project_id)
        REFERENCES company_project_tasks(id, project_id) ON DELETE CASCADE,
    CHECK (from_status IS NULL OR from_status IN ('todo', 'in_progress', 'blocked', 'done', 'failed', 'cancelled')),
    CHECK (to_status IN ('todo', 'in_progress', 'blocked', 'done', 'failed', 'cancelled')),
    CHECK (change_source IN ('agent', 'human', 'system')),
    CHECK (NOT (changed_by_agent_id IS NOT NULL AND changed_by_human_user_id IS NOT NULL)),
    CHECK (jsonb_typeof(metadata) = 'object')
);

CREATE INDEX idx_company_project_task_status_history_task_created
    ON company_project_task_status_history(task_id, created_at DESC);

INSERT INTO company_project_task_status_history (
    id, project_id, task_id, from_status, to_status,
    changed_by_agent_id, changed_by_human_user_id, change_source, metadata, created_at
)
SELECT
    gen_random_uuid(), project_id, id, NULL, status,
    updated_by_agent_id, updated_by_human_user_id,
    CASE
        WHEN updated_by_agent_id IS NOT NULL THEN 'agent'
        WHEN updated_by_human_user_id IS NOT NULL THEN 'human'
        ELSE 'system'
    END,
    '{"reason":"history_backfill"}'::jsonb,
    updated_at
FROM company_project_tasks;

WITH inconsistent AS (
    SELECT DISTINCT task.id, task.project_id, task.status,
           task.updated_by_agent_id, task.updated_by_human_user_id
    FROM company_project_tasks task
    JOIN company_project_task_dependencies dependency ON dependency.task_id = task.id
    JOIN company_project_tasks prerequisite ON prerequisite.id = dependency.depends_on_task_id
    WHERE task.status = 'in_progress'
      AND prerequisite.status NOT IN ('done', 'cancelled')
), recorded AS (
    INSERT INTO company_project_task_status_history (
        id, project_id, task_id, from_status, to_status,
        changed_by_agent_id, changed_by_human_user_id, change_source, metadata, created_at
    )
    SELECT
        gen_random_uuid(), project_id, id, status, 'todo',
        NULL, NULL, 'system',
        '{"reason":"unresolved_dependency_repair"}'::jsonb,
        now()
    FROM inconsistent
    RETURNING task_id
)
UPDATE company_project_tasks task
SET status = 'todo', completed_at = NULL, updated_at = now(),
    updated_by_agent_id = NULL, updated_by_human_user_id = NULL
WHERE task.id IN (SELECT task_id FROM recorded);
