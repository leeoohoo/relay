CREATE TABLE company_governance_policy_versions (
    id UUID PRIMARY KEY,
    company_id UUID NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    version INTEGER NOT NULL CHECK (version >= 1),
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'archived')),
    settings JSONB NOT NULL CHECK (
        jsonb_typeof(settings) = 'object'
        AND settings ?& ARRAY[
            'agent_staff_limit',
            'delegated_agent_hiring_enabled',
            'delegated_agent_suspension_enabled',
            'delegated_agent_termination_enabled',
            'max_active_projects',
            'max_project_members'
        ]
        AND jsonb_typeof(settings -> 'agent_staff_limit') = 'number'
        AND (settings ->> 'agent_staff_limit')::INTEGER BETWEEN 1 AND 1000
        AND jsonb_typeof(settings -> 'delegated_agent_hiring_enabled') = 'boolean'
        AND jsonb_typeof(settings -> 'delegated_agent_suspension_enabled') = 'boolean'
        AND jsonb_typeof(settings -> 'delegated_agent_termination_enabled') = 'boolean'
        AND jsonb_typeof(settings -> 'max_active_projects') = 'number'
        AND (settings ->> 'max_active_projects')::INTEGER BETWEEN 1 AND 1000
        AND jsonb_typeof(settings -> 'max_project_members') = 'number'
        AND (settings ->> 'max_project_members')::INTEGER BETWEEN 1 AND 200
    ),
    notes TEXT NOT NULL DEFAULT '' CHECK (char_length(notes) <= 1000),
    created_by_human_user_id UUID NOT NULL REFERENCES human_users(id) ON DELETE RESTRICT,
    updated_by_human_user_id UUID REFERENCES human_users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (company_id, version)
);

CREATE UNIQUE INDEX uq_company_governance_policy_active
    ON company_governance_policy_versions(company_id)
    WHERE status = 'active';

CREATE INDEX idx_company_governance_policy_versions
    ON company_governance_policy_versions(company_id, version DESC);

CREATE FUNCTION emit_company_governance_policy_realtime_event()
RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO realtime_events (
        company_id, event_type, aggregate_type, aggregate_id,
        actor_human_user_id, payload, created_at
    ) VALUES (
        NEW.company_id,
        CASE
            WHEN TG_OP = 'INSERT' THEN 'company.governance_policy.published'
            WHEN NEW.status = 'archived' AND OLD.status IS DISTINCT FROM NEW.status
                THEN 'company.governance_policy.archived'
            ELSE 'company.governance_policy.updated'
        END,
        'company_governance_policy_version',
        NEW.id,
        COALESCE(NEW.updated_by_human_user_id, NEW.created_by_human_user_id),
        jsonb_build_object(
            'policy_version_id', NEW.id,
            'version', NEW.version,
            'status', NEW.status,
            'settings', NEW.settings
        ),
        NEW.updated_at
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_company_governance_policy_realtime_insert
AFTER INSERT ON company_governance_policy_versions
FOR EACH ROW EXECUTE FUNCTION emit_company_governance_policy_realtime_event();

CREATE TRIGGER trg_company_governance_policy_realtime_update
AFTER UPDATE OF status, settings, notes ON company_governance_policy_versions
FOR EACH ROW EXECUTE FUNCTION emit_company_governance_policy_realtime_event();
