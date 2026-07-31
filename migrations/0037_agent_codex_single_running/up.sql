WITH duplicate_running AS (
    SELECT id,
           ROW_NUMBER() OVER (
               PARTITION BY agent_profile_id
               ORDER BY started_at DESC, id DESC
           ) AS position
    FROM agent_codex_trigger_runs
    WHERE status = 'running'
)
UPDATE agent_codex_trigger_runs run
SET status = 'lease_lost',
    finished_at = COALESCE(run.finished_at, NOW()),
    error_message = COALESCE(
        run.error_message,
        'Superseded while enforcing one running Codex cycle per Agent'
    )
FROM duplicate_running duplicate
WHERE run.id = duplicate.id
  AND duplicate.position > 1;

CREATE UNIQUE INDEX uq_agent_codex_trigger_runs_one_running_per_agent
    ON agent_codex_trigger_runs(agent_profile_id)
    WHERE status = 'running';
