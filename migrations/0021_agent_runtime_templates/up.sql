CREATE TABLE agent_runtime_templates (
    id UUID PRIMARY KEY,
    company_id UUID NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    name TEXT NOT NULL CHECK (char_length(name) BETWEEN 1 AND 80),
    description TEXT NOT NULL DEFAULT '' CHECK (char_length(description) <= 1000),
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'archived')),
    settings JSONB NOT NULL CHECK (jsonb_typeof(settings) = 'object'),
    source_agent_profile_id UUID REFERENCES agent_profiles(id) ON DELETE SET NULL,
    created_by_human_user_id UUID NOT NULL REFERENCES human_users(id) ON DELETE RESTRICT,
    updated_by_human_user_id UUID REFERENCES human_users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX uq_agent_runtime_templates_active_name
    ON agent_runtime_templates(company_id, LOWER(name))
    WHERE status = 'active';

CREATE INDEX idx_agent_runtime_templates_company_status
    ON agent_runtime_templates(company_id, status, updated_at DESC);

ALTER TABLE agent_runtime_configs
    ADD COLUMN runtime_template_id UUID
        REFERENCES agent_runtime_templates(id) ON DELETE SET NULL;

CREATE INDEX idx_agent_runtime_configs_template
    ON agent_runtime_configs(runtime_template_id)
    WHERE runtime_template_id IS NOT NULL;

CREATE FUNCTION emit_agent_runtime_template_realtime_event()
RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO realtime_events (
        company_id, event_type, aggregate_type, aggregate_id,
        actor_human_user_id, payload, created_at
    ) VALUES (
        NEW.company_id,
        CASE
            WHEN TG_OP = 'INSERT' THEN 'agent.runtime.template_created'
            WHEN NEW.status = 'archived' AND OLD.status IS DISTINCT FROM NEW.status
                THEN 'agent.runtime.template_archived'
            ELSE 'agent.runtime.template_updated'
        END,
        'agent_runtime_template',
        NEW.id,
        COALESCE(NEW.updated_by_human_user_id, NEW.created_by_human_user_id),
        jsonb_build_object(
            'runtime_template_id', NEW.id,
            'name', NEW.name,
            'status', NEW.status,
            'source_agent_profile_id', NEW.source_agent_profile_id
        ),
        NEW.updated_at
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_agent_runtime_templates_realtime_insert
AFTER INSERT ON agent_runtime_templates
FOR EACH ROW EXECUTE FUNCTION emit_agent_runtime_template_realtime_event();

CREATE TRIGGER trg_agent_runtime_templates_realtime_update
AFTER UPDATE OF name, description, status, settings, source_agent_profile_id
ON agent_runtime_templates
FOR EACH ROW EXECUTE FUNCTION emit_agent_runtime_template_realtime_event();

CREATE FUNCTION emit_agent_runtime_template_applied_realtime_event()
RETURNS TRIGGER AS $$
BEGIN
    IF OLD.runtime_template_id IS NOT DISTINCT FROM NEW.runtime_template_id THEN
        RETURN NEW;
    END IF;
    INSERT INTO realtime_events (
        company_id, event_type, aggregate_type, aggregate_id,
        actor_human_user_id, payload, created_at
    ) VALUES (
        NEW.company_id,
        'agent.runtime.template_applied',
        'agent_runtime_config',
        NEW.id,
        NEW.updated_by_human_user_id,
        jsonb_build_object(
            'runtime_config_id', NEW.id,
            'agent_profile_id', NEW.agent_profile_id,
            'runtime_template_id', NEW.runtime_template_id
        ),
        NEW.updated_at
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_agent_runtime_configs_template_applied_realtime
AFTER UPDATE OF runtime_template_id ON agent_runtime_configs
FOR EACH ROW EXECUTE FUNCTION emit_agent_runtime_template_applied_realtime_event();
