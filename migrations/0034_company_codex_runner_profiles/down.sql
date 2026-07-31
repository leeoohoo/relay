DROP TABLE IF EXISTS agent_codex_runner_profile_assignments;
DROP TABLE IF EXISTS company_codex_runner_profiles;
ALTER TABLE agent_codex_trigger_configs DROP CONSTRAINT IF EXISTS chk_agent_codex_trigger_model;
ALTER TABLE agent_codex_trigger_configs DROP COLUMN IF EXISTS model;
