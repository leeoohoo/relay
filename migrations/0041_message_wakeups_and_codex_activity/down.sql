DROP INDEX IF EXISTS idx_agent_codex_trigger_configs_wake_requested;

ALTER TABLE agent_codex_trigger_runs
    DROP CONSTRAINT IF EXISTS agent_codex_trigger_runs_activity_log_array_check,
    DROP COLUMN IF EXISTS activity_log,
    DROP COLUMN IF EXISTS last_activity_at,
    DROP COLUMN IF EXISTS activity_summary,
    DROP COLUMN IF EXISTS activity_phase;

UPDATE company_codex_runner_profiles
SET interval_seconds = LEAST(interval_seconds, 86400);

ALTER TABLE company_codex_runner_profiles
    DROP CONSTRAINT company_codex_runner_profiles_interval_seconds_check,
    ADD CONSTRAINT company_codex_runner_profiles_interval_seconds_check
        CHECK (interval_seconds BETWEEN 10 AND 86400);

UPDATE agent_codex_trigger_configs
SET interval_seconds = LEAST(interval_seconds, 86400);

ALTER TABLE agent_codex_trigger_configs
    DROP CONSTRAINT IF EXISTS agent_codex_trigger_configs_wake_reason_check,
    DROP CONSTRAINT agent_codex_trigger_configs_interval_seconds_check,
    ADD CONSTRAINT agent_codex_trigger_configs_interval_seconds_check
        CHECK (interval_seconds BETWEEN 10 AND 86400),
    DROP COLUMN IF EXISTS wake_reason,
    DROP COLUMN IF EXISTS wake_requested_at;
