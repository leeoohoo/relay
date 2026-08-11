CREATE TABLE project_gates (
    id UUID PRIMARY KEY,
    company_id UUID NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    project_id UUID NOT NULL REFERENCES company_projects(id) ON DELETE CASCADE,
    gate_key TEXT NOT NULL,
    gate_type TEXT NOT NULL,
    title TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    related_task_id UUID REFERENCES company_project_tasks(id) ON DELETE SET NULL,
    required_evidence_json JSONB NOT NULL DEFAULT '[]'::jsonb,
    decision_summary TEXT NOT NULL DEFAULT '',
    decided_by_agent_id UUID REFERENCES agent_profiles(id) ON DELETE SET NULL,
    decided_by_human_user_id UUID REFERENCES human_users(id) ON DELETE SET NULL,
    decided_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    CONSTRAINT project_gates_key_unique UNIQUE (project_id, gate_key),
    CONSTRAINT project_gates_type_check CHECK (
        gate_type IN ('design', 'technical', 'qa', 'pm', 'environment', 'approval', 'release', 'custom')
    ),
    CONSTRAINT project_gates_status_check CHECK (
        status IN ('pending', 'evaluating', 'passed', 'failed', 'waived', 'cancelled')
    ),
    CONSTRAINT project_gates_evidence_array_check CHECK (jsonb_typeof(required_evidence_json) = 'array')
);

CREATE INDEX idx_project_gates_project_status
    ON project_gates(project_id, status, created_at);

CREATE TABLE project_task_gate_requirements (
    task_id UUID NOT NULL REFERENCES company_project_tasks(id) ON DELETE CASCADE,
    gate_id UUID NOT NULL REFERENCES project_gates(id) ON DELETE CASCADE,
    required_status TEXT NOT NULL DEFAULT 'passed',
    created_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (task_id, gate_id),
    CONSTRAINT project_task_gate_required_status_check
        CHECK (required_status IN ('passed', 'waived'))
);

CREATE INDEX idx_project_task_gate_requirements_gate
    ON project_task_gate_requirements(gate_id, task_id);
