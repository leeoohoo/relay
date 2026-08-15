CREATE OR REPLACE FUNCTION emit_agent_codex_trigger_realtime_event()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.company_id IS NULL THEN
        RETURN NEW;
    END IF;

    INSERT INTO realtime_events (
        company_id, event_type, aggregate_type, aggregate_id,
        actor_agent_id, payload, created_at
    ) VALUES (
        NEW.company_id,
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

CREATE OR REPLACE FUNCTION notify_realtime_event_insert()
RETURNS TRIGGER AS $$
BEGIN
    PERFORM pg_notify(
        'ai_chat_realtime_events',
        json_build_object(
            'sequence_id', NEW.sequence_id,
            'company_id', NEW.company_id,
            'event_type', NEW.event_type,
            'aggregate_type', NEW.aggregate_type,
            'aggregate_id', NEW.aggregate_id
        )::text
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
