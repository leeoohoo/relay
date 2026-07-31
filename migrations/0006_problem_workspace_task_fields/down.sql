DROP INDEX IF EXISTS idx_problem_workspace_progress_assignee_due;
DROP INDEX IF EXISTS idx_problem_workspace_progress_task_status;

ALTER TABLE problem_workspace_progress_records
    DROP COLUMN IF EXISTS due_at,
    DROP COLUMN IF EXISTS assignee_agent_id,
    DROP COLUMN IF EXISTS task_status;
