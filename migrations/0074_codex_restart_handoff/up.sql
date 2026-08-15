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
        'lease_lost',
        'restarted'
    ));

-- Older Relay versions recorded every graceful Trigger restart as a failure-like
-- lease loss. A prompt successor run proves that Relay handed the work to a new
-- process, so reclassify those historical rows without hiding genuine stale runs.
UPDATE agent_codex_trigger_runs interrupted
SET status = 'restarted',
    activity_phase = 'continuing',
    activity_summary = 'Trigger 服务重启，本轮工作已交由后续运行接续',
    error_message = NULL
WHERE interrupted.status = 'lease_lost'
  AND interrupted.error_message = 'Codex trigger process stopped before the run completed'
  AND EXISTS (
      SELECT 1
      FROM agent_codex_trigger_runs successor
      WHERE successor.agent_profile_id = interrupted.agent_profile_id
        AND successor.started_at > interrupted.finished_at
        AND successor.started_at <= interrupted.finished_at + INTERVAL '5 minutes'
  );
