CREATE TABLE agent_staffing_actions (
    id UUID PRIMARY KEY,
    company_id UUID NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    action_type TEXT NOT NULL
        CHECK (action_type IN (
            'permission_update', 'hire', 'activate', 'suspend', 'reactivate', 'terminate'
        )),
    actor_type TEXT NOT NULL CHECK (actor_type IN ('human', 'agent', 'system')),
    actor_human_user_id UUID REFERENCES human_users(id) ON DELETE SET NULL,
    actor_agent_id UUID REFERENCES agent_profiles(id) ON DELETE SET NULL,
    target_agent_id UUID REFERENCES agent_profiles(id) ON DELETE SET NULL,
    requested_org_unit_id UUID REFERENCES org_units(id) ON DELETE SET NULL,
    requested_role_key TEXT,
    reason TEXT NOT NULL DEFAULT '',
    handoff_plan TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL
        CHECK (status IN (
            'pending_approval', 'approved', 'executing', 'completed', 'rejected', 'failed'
        )),
    approval_required BOOLEAN NOT NULL DEFAULT FALSE,
    approved_by_human_user_id UUID REFERENCES human_users(id) ON DELETE SET NULL,
    request_payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    result_payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    idempotency_key TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ,
    CHECK (
        (actor_type = 'human' AND actor_human_user_id IS NOT NULL)
        OR (actor_type = 'agent' AND actor_agent_id IS NOT NULL)
        OR actor_type = 'system'
    )
);

CREATE INDEX idx_agent_staffing_actions_company_created_at
    ON agent_staffing_actions(company_id, created_at DESC);

CREATE INDEX idx_agent_staffing_actions_target_created_at
    ON agent_staffing_actions(target_agent_id, created_at DESC)
    WHERE target_agent_id IS NOT NULL;

CREATE UNIQUE INDEX idx_agent_staffing_actions_agent_idempotency
    ON agent_staffing_actions(company_id, actor_agent_id, action_type, idempotency_key)
    WHERE actor_agent_id IS NOT NULL AND idempotency_key IS NOT NULL;
