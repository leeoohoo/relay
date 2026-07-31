CREATE TABLE company_runtime_policies (
    company_id UUID PRIMARY KEY REFERENCES companies(id) ON DELETE CASCADE,
    default_runtime_template_id UUID REFERENCES agent_runtime_templates(id) ON DELETE SET NULL,
    auto_apply_to_new_agents BOOLEAN NOT NULL DEFAULT TRUE,
    created_by_human_user_id UUID REFERENCES human_users(id) ON DELETE SET NULL,
    updated_by_human_user_id UUID REFERENCES human_users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_company_runtime_policies_default_template
    ON company_runtime_policies(default_runtime_template_id)
    WHERE default_runtime_template_id IS NOT NULL;

CREATE FUNCTION emit_company_runtime_policy_realtime_event()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'UPDATE'
       AND OLD.default_runtime_template_id IS NOT DISTINCT FROM NEW.default_runtime_template_id
       AND OLD.auto_apply_to_new_agents IS NOT DISTINCT FROM NEW.auto_apply_to_new_agents THEN
        RETURN NEW;
    END IF;
    INSERT INTO realtime_events (
        company_id, event_type, aggregate_type, aggregate_id,
        actor_human_user_id, payload, created_at
    ) VALUES (
        NEW.company_id,
        'company.runtime_policy.updated',
        'company_runtime_policy',
        NEW.company_id,
        COALESCE(NEW.updated_by_human_user_id, NEW.created_by_human_user_id),
        jsonb_build_object(
            'default_runtime_template_id', NEW.default_runtime_template_id,
            'auto_apply_to_new_agents', NEW.auto_apply_to_new_agents
        ),
        NEW.updated_at
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_company_runtime_policies_realtime_insert
AFTER INSERT ON company_runtime_policies
FOR EACH ROW EXECUTE FUNCTION emit_company_runtime_policy_realtime_event();

CREATE TRIGGER trg_company_runtime_policies_realtime_update
AFTER UPDATE OF default_runtime_template_id, auto_apply_to_new_agents
ON company_runtime_policies
FOR EACH ROW EXECUTE FUNCTION emit_company_runtime_policy_realtime_event();

CREATE FUNCTION clear_archived_company_default_runtime_template()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.status = 'archived' AND OLD.status IS DISTINCT FROM NEW.status THEN
        UPDATE company_runtime_policies
        SET default_runtime_template_id = NULL,
            updated_by_human_user_id = COALESCE(
                NEW.updated_by_human_user_id,
                updated_by_human_user_id
            ),
            updated_at = NEW.updated_at
        WHERE default_runtime_template_id = NEW.id;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_agent_runtime_templates_clear_company_default
AFTER UPDATE OF status ON agent_runtime_templates
FOR EACH ROW EXECUTE FUNCTION clear_archived_company_default_runtime_template();

CREATE FUNCTION emit_default_runtime_template_applied_realtime_event()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.runtime_template_id IS NULL THEN
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
            'runtime_template_id', NEW.runtime_template_id,
            'application_mode', 'company_default'
        ),
        NEW.updated_at
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_agent_runtime_configs_default_template_applied_realtime
AFTER INSERT ON agent_runtime_configs
FOR EACH ROW EXECUTE FUNCTION emit_default_runtime_template_applied_realtime_event();
