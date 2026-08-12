DROP TRIGGER IF EXISTS trg_agent_codex_trigger_runs_realtime_update
    ON agent_codex_trigger_runs;

ALTER TABLE agent_codex_trigger_runs
    DROP COLUMN IF EXISTS resumes_run_id,
    DROP COLUMN IF EXISTS session_kind,
    DROP COLUMN IF EXISTS waiting_on_id,
    DROP COLUMN IF EXISTS waiting_on_type,
    DROP COLUMN IF EXISTS current_task_id,
    DROP COLUMN IF EXISTS current_intent_id,
    DROP COLUMN IF EXISTS state_reason,
    DROP COLUMN IF EXISTS heartbeat_at,
    DROP COLUMN IF EXISTS process_instance_id;

CREATE TRIGGER trg_agent_codex_trigger_runs_realtime_update
AFTER UPDATE OF status, activity_phase, activity_summary, last_activity_at,
    finished_at, final_message_summary, error_message
ON agent_codex_trigger_runs
FOR EACH ROW EXECUTE FUNCTION emit_agent_codex_run_realtime_event();
