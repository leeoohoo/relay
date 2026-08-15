DROP TABLE IF EXISTS codex_run_realtime_emit_state;

CREATE OR REPLACE FUNCTION emit_agent_codex_run_realtime_event()
RETURNS TRIGGER AS $$
DECLARE
    run_company_id UUID;
BEGIN
    SELECT company_id INTO run_company_id
    FROM agent_codex_trigger_configs
    WHERE id = NEW.trigger_config_id;

    IF run_company_id IS NULL THEN
        RETURN NEW;
    END IF;

    INSERT INTO realtime_events (
        company_id, event_type, aggregate_type, aggregate_id,
        actor_agent_id, payload, created_at
    ) VALUES (
        run_company_id,
        'codex.run.updated',
        'agent_codex_trigger_run',
        NEW.id,
        NEW.agent_profile_id,
        jsonb_build_object(
            'agent_profile_id', NEW.agent_profile_id,
            'run_id', NEW.id,
            'status', NEW.status,
            'activity_phase', NEW.activity_phase,
            'activity_summary', NEW.activity_summary,
            'last_activity_at', NEW.last_activity_at,
            'finished_at', NEW.finished_at
        ),
        COALESCE(NEW.last_activity_at, NEW.finished_at, NEW.started_at)
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
