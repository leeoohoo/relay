ALTER TABLE agent_codex_trigger_configs
    DROP COLUMN IF EXISTS reasoning_effort;

ALTER TABLE company_codex_runner_profiles
    DROP COLUMN IF EXISTS reasoning_effort;
