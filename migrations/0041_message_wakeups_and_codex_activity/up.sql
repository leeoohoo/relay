ALTER TABLE agent_codex_trigger_configs
    ADD COLUMN wake_requested_at TIMESTAMPTZ,
    ADD COLUMN wake_reason TEXT,
    DROP CONSTRAINT agent_codex_trigger_configs_interval_seconds_check,
    ADD CONSTRAINT agent_codex_trigger_configs_interval_seconds_check
        CHECK (interval_seconds BETWEEN 10 AND 604800),
    ADD CONSTRAINT agent_codex_trigger_configs_wake_reason_check
        CHECK (wake_reason IS NULL OR wake_reason IN ('message'));

ALTER TABLE company_codex_runner_profiles
    DROP CONSTRAINT company_codex_runner_profiles_interval_seconds_check,
    ADD CONSTRAINT company_codex_runner_profiles_interval_seconds_check
        CHECK (interval_seconds BETWEEN 10 AND 604800);

ALTER TABLE agent_codex_trigger_runs
    ADD COLUMN activity_phase TEXT NOT NULL DEFAULT 'preparing',
    ADD COLUMN activity_summary TEXT,
    ADD COLUMN last_activity_at TIMESTAMPTZ,
    ADD COLUMN activity_log JSONB NOT NULL DEFAULT '[]'::jsonb,
    ADD CONSTRAINT agent_codex_trigger_runs_activity_log_array_check
        CHECK (jsonb_typeof(activity_log) = 'array');

CREATE INDEX idx_agent_codex_trigger_configs_wake_requested
    ON agent_codex_trigger_configs(wake_requested_at)
    WHERE status = 'active' AND wake_requested_at IS NOT NULL;
