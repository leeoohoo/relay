ALTER TABLE agent_codex_trigger_runs
    ADD COLUMN process_instance_id TEXT,
    ADD COLUMN heartbeat_at TIMESTAMPTZ,
    ADD COLUMN state_reason TEXT,
    ADD COLUMN current_intent_id UUID REFERENCES agent_execution_intents(id) ON DELETE SET NULL,
    ADD COLUMN current_task_id UUID REFERENCES company_project_tasks(id) ON DELETE SET NULL,
    ADD COLUMN waiting_on_type TEXT,
    ADD COLUMN waiting_on_id UUID,
    ADD COLUMN session_kind TEXT NOT NULL DEFAULT 'control'
        CHECK (session_kind IN ('control', 'project')),
    ADD COLUMN resumes_run_id UUID REFERENCES agent_codex_trigger_runs(id) ON DELETE SET NULL;

UPDATE agent_codex_trigger_runs
SET heartbeat_at = COALESCE(last_activity_at, finished_at, started_at),
    state_reason = COALESCE(activity_summary, final_message_summary, error_message),
    session_kind = CASE WHEN project_id IS NULL THEN 'control' ELSE 'project' END;

CREATE INDEX idx_agent_codex_trigger_runs_running_heartbeat
    ON agent_codex_trigger_runs(heartbeat_at)
    WHERE status = 'running';

CREATE INDEX idx_agent_codex_trigger_runs_resumes
    ON agent_codex_trigger_runs(resumes_run_id)
    WHERE resumes_run_id IS NOT NULL;

DROP TRIGGER IF EXISTS trg_agent_codex_trigger_runs_realtime_update
    ON agent_codex_trigger_runs;

CREATE TRIGGER trg_agent_codex_trigger_runs_realtime_update
AFTER UPDATE OF status, activity_phase, activity_summary, last_activity_at,
    heartbeat_at, state_reason, current_intent_id, current_task_id,
    waiting_on_type, waiting_on_id, session_kind, finished_at,
    final_message_summary, error_message
ON agent_codex_trigger_runs
FOR EACH ROW EXECUTE FUNCTION emit_agent_codex_run_realtime_event();
