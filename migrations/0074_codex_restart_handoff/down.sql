UPDATE agent_codex_trigger_runs
SET status = 'lease_lost',
    activity_phase = 'lease_lost',
    activity_summary = 'Trigger 进程中断，本轮已停止',
    error_message = COALESCE(
        error_message,
        'Codex trigger process stopped before the run completed'
    )
WHERE status = 'restarted';

ALTER TABLE agent_codex_trigger_runs
    DROP CONSTRAINT agent_codex_trigger_runs_status_check;

ALTER TABLE agent_codex_trigger_runs
    ADD CONSTRAINT agent_codex_trigger_runs_status_check
    CHECK (status IN (
        'running',
        'succeeded',
        'failed',
        'timed_out',
        'cancelled',
        'lease_lost'
    ));
