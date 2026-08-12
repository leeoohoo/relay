CREATE TABLE project_evidence (
    id UUID PRIMARY KEY,
    project_id UUID NOT NULL REFERENCES company_projects(id) ON DELETE CASCADE,
    task_id UUID NULL REFERENCES company_project_tasks(id) ON DELETE CASCADE,
    attempt_id UUID NULL REFERENCES project_task_attempts(id) ON DELETE SET NULL,
    gate_id UUID NULL REFERENCES project_gates(id) ON DELETE SET NULL,
    environment_id UUID NULL REFERENCES project_environments(id) ON DELETE SET NULL,
    evidence_type TEXT NOT NULL,
    title TEXT NOT NULL,
    summary TEXT NOT NULL,
    result TEXT NOT NULL,
    artifact_refs JSONB NOT NULL DEFAULT '[]'::jsonb,
    metrics JSONB NOT NULL DEFAULT '{}'::jsonb,
    producer_agent_id UUID NULL REFERENCES agent_profiles(id),
    producer_human_user_id UUID NULL REFERENCES human_users(id),
    dedupe_key TEXT NULL,
    created_at TIMESTAMPTZ NOT NULL
);

CREATE UNIQUE INDEX idx_project_evidence_dedupe
    ON project_evidence(project_id, dedupe_key)
    WHERE dedupe_key IS NOT NULL;
CREATE INDEX idx_project_evidence_task_recent
    ON project_evidence(task_id, created_at DESC);
CREATE INDEX idx_project_evidence_project_recent
    ON project_evidence(project_id, created_at DESC);
