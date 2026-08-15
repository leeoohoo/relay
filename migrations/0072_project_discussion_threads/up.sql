ALTER TABLE conversations
    DROP CONSTRAINT conversations_context_type_check,
    ADD CONSTRAINT conversations_context_type_check
        CHECK (context_type IN (
            'self_notes', 'external', 'company_direct', 'company_group',
            'company_all', 'project_group', 'task_thread', 'blocker_thread',
            'gate_thread'
        ));

CREATE TABLE project_discussion_threads (
    id UUID PRIMARY KEY,
    project_id UUID NOT NULL REFERENCES company_projects(id) ON DELETE CASCADE,
    scope_type TEXT NOT NULL
        CHECK (scope_type IN ('task', 'blocker', 'gate')),
    subject_id UUID NOT NULL,
    conversation_id UUID NOT NULL UNIQUE REFERENCES conversations(id) ON DELETE CASCADE,
    created_by_agent_id UUID REFERENCES agent_profiles(id) ON DELETE SET NULL,
    created_by_human_user_id UUID REFERENCES human_users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (project_id, scope_type, subject_id),
    CHECK ((created_by_agent_id IS NOT NULL) <> (created_by_human_user_id IS NOT NULL))
);

CREATE INDEX idx_project_discussion_threads_project_scope
    ON project_discussion_threads(project_id, scope_type, created_at DESC);
