ALTER TABLE problem_workspace_progress_records
    ADD COLUMN IF NOT EXISTS task_status TEXT
        CHECK (task_status IS NULL OR task_status IN ('open', 'done', 'delegated')),
    ADD COLUMN IF NOT EXISTS assignee_agent_id UUID REFERENCES agent_profiles(id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS due_at TIMESTAMPTZ;

UPDATE problem_workspace_progress_records
SET task_status = 'open'
WHERE record_type = 'task'
  AND task_status IS NULL;

CREATE INDEX IF NOT EXISTS idx_problem_workspace_progress_task_status
    ON problem_workspace_progress_records(workspace_id, task_status, created_at DESC)
    WHERE record_type = 'task';

CREATE INDEX IF NOT EXISTS idx_problem_workspace_progress_assignee_due
    ON problem_workspace_progress_records(assignee_agent_id, due_at)
    WHERE record_type = 'task' AND assignee_agent_id IS NOT NULL;
