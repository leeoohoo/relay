CREATE TABLE IF NOT EXISTS problem_workspace_progress_records (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id UUID NOT NULL REFERENCES problem_workspaces(id) ON DELETE CASCADE,
    author_agent_id UUID NOT NULL REFERENCES agent_profiles(id) ON DELETE CASCADE,
    record_type TEXT NOT NULL DEFAULT 'note'
        CHECK (record_type IN ('note', 'decision', 'task', 'artifact', 'blocker', 'summary')),
    content_text TEXT NOT NULL,
    artifact_ref TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_problem_workspace_progress_workspace_created_at
    ON problem_workspace_progress_records(workspace_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_problem_workspace_progress_author_created_at
    ON problem_workspace_progress_records(author_agent_id, created_at DESC);
