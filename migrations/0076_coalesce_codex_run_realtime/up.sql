CREATE TABLE IF NOT EXISTS codex_run_realtime_emit_state (
    run_id UUID PRIMARY KEY REFERENCES agent_codex_trigger_runs(id) ON DELETE CASCADE,
    last_progress_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE OR REPLACE FUNCTION emit_agent_codex_run_realtime_event()
RETURNS TRIGGER AS $$
DECLARE
    run_company_id UUID;
    accepted_progress_at TIMESTAMPTZ;
    terminal_transition BOOLEAN;
BEGIN
    SELECT company_id INTO run_company_id
    FROM agent_codex_trigger_configs
    WHERE id = NEW.trigger_config_id;

    IF run_company_id IS NULL THEN
        RETURN NEW;
    END IF;

    IF TG_OP = 'INSERT' THEN
        terminal_transition := TRUE;
    ELSE
        terminal_transition := NEW.status IS DISTINCT FROM OLD.status
            OR NEW.finished_at IS DISTINCT FROM OLD.finished_at
            OR NEW.error_message IS DISTINCT FROM OLD.error_message
            OR NEW.final_message_summary IS DISTINCT FROM OLD.final_message_summary;
    END IF;

    IF NOT terminal_transition THEN
        -- An atomic upsert is the concurrency gate. Progress callbacks for the
        -- same run can be committed by different transactions, but only one of
        -- them may advance this row during each one-second activity window.
        INSERT INTO codex_run_realtime_emit_state (
            run_id, last_progress_at, updated_at
        ) VALUES (
            NEW.id,
            COALESCE(NEW.last_activity_at, NEW.heartbeat_at, clock_timestamp()),
            clock_timestamp()
        )
        ON CONFLICT (run_id) DO UPDATE
        SET last_progress_at = EXCLUDED.last_progress_at,
            updated_at = clock_timestamp()
        WHERE EXCLUDED.last_progress_at
            > codex_run_realtime_emit_state.last_progress_at + INTERVAL '1 second'
        RETURNING last_progress_at INTO accepted_progress_at;

        IF accepted_progress_at IS NULL THEN
            RETURN NEW;
        END IF;
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
            'project_id', NEW.project_id,
            'activity_phase', NEW.activity_phase,
            'activity_summary', NEW.activity_summary,
            'last_activity_at', NEW.last_activity_at,
            'finished_at', NEW.finished_at,
            'heartbeat_at', NEW.heartbeat_at,
            'state_reason', NEW.state_reason,
            'current_intent_id', NEW.current_intent_id,
            'current_task_id', NEW.current_task_id,
            'waiting_on_type', NEW.waiting_on_type,
            'waiting_on_id', NEW.waiting_on_id,
            'session_kind', NEW.session_kind
        ),
        COALESCE(NEW.last_activity_at, NEW.finished_at, NEW.started_at)
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
