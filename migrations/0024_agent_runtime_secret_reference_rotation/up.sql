CREATE FUNCTION emit_agent_runtime_secret_reference_rotated_event()
RETURNS TRIGGER AS $$
BEGIN
    IF OLD.provider_secret_ref IS NOT DISTINCT FROM NEW.provider_secret_ref THEN
        RETURN NEW;
    END IF;
    INSERT INTO realtime_events (
        company_id, event_type, aggregate_type, aggregate_id,
        actor_human_user_id, payload, created_at
    ) VALUES (
        NEW.company_id,
        'agent.runtime.secret_ref_rotated',
        'agent_runtime_config',
        NEW.id,
        NEW.updated_by_human_user_id,
        jsonb_build_object(
            'runtime_config_id', NEW.id,
            'agent_profile_id', NEW.agent_profile_id,
            'reference_configured', NEW.provider_secret_ref IS NOT NULL,
            'reference_scheme', CASE
                WHEN NEW.provider_secret_ref IS NULL THEN NULL
                ELSE split_part(NEW.provider_secret_ref, ':', 1)
            END
        ),
        NEW.updated_at
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_agent_runtime_configs_secret_reference_rotated
AFTER UPDATE OF provider_secret_ref ON agent_runtime_configs
FOR EACH ROW EXECUTE FUNCTION emit_agent_runtime_secret_reference_rotated_event();
