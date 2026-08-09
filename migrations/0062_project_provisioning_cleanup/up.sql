CREATE TABLE project_provisioning_cleanup_jobs (
    id UUID PRIMARY KEY,
    human_user_id UUID NOT NULL REFERENCES human_users(id) ON DELETE CASCADE,
    company_id UUID NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    project_id UUID NOT NULL,
    managed_local_path TEXT NOT NULL,
    repository_identifier TEXT NOT NULL,
    access_token_identifier TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending', 'running', 'failed', 'completed')),
    attempts INTEGER NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    next_attempt_at TIMESTAMPTZ NOT NULL,
    lease_expires_at TIMESTAMPTZ,
    last_error TEXT,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    completed_at TIMESTAMPTZ
);

CREATE UNIQUE INDEX idx_project_provisioning_cleanup_active_project
    ON project_provisioning_cleanup_jobs(project_id)
    WHERE status <> 'completed';

CREATE INDEX idx_project_provisioning_cleanup_due
    ON project_provisioning_cleanup_jobs(next_attempt_at, created_at)
    WHERE status <> 'completed';
