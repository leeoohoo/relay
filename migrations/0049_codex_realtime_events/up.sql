CREATE FUNCTION emit_agent_codex_trigger_realtime_event()
RETURNS TRIGGER AS $$
DECLARE
    trigger_company_id UUID;
BEGIN
    trigger_company_id := NEW.company_id;

    IF trigger_company_id IS NULL THEN
        RETURN NEW;
    END IF;

    INSERT INTO realtime_events (
        company_id, event_type, aggregate_type, aggregate_id,
        actor_agent_id, payload, created_at
    ) VALUES (
        trigger_company_id,
        'codex.trigger.updated',
        'agent_codex_trigger_config',
        NEW.id,
        NEW.agent_profile_id,
        jsonb_build_object(
            'agent_profile_id', NEW.agent_profile_id,
            'status', NEW.status,
            'lease_owner', NEW.lease_owner,
            'lease_expires_at', NEW.lease_expires_at,
            'wake_requested_at', NEW.wake_requested_at,
            'last_run_at', NEW.last_run_at,
            'last_success_at', NEW.last_success_at,
            'last_error', NEW.last_error
        ),
        NEW.updated_at
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_agent_codex_trigger_configs_realtime
AFTER INSERT OR UPDATE ON agent_codex_trigger_configs
FOR EACH ROW EXECUTE FUNCTION emit_agent_codex_trigger_realtime_event();

CREATE FUNCTION emit_agent_codex_run_realtime_event()
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

CREATE TRIGGER trg_agent_codex_trigger_runs_realtime_insert
AFTER INSERT ON agent_codex_trigger_runs
FOR EACH ROW EXECUTE FUNCTION emit_agent_codex_run_realtime_event();

CREATE TRIGGER trg_agent_codex_trigger_runs_realtime_update
AFTER UPDATE OF status, activity_phase, activity_summary, last_activity_at,
    finished_at, final_message_summary, error_message
ON agent_codex_trigger_runs
FOR EACH ROW EXECUTE FUNCTION emit_agent_codex_run_realtime_event();
