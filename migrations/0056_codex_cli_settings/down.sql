UPDATE company_codex_runner_profiles
SET sandbox_mode = 'workspace_write'
WHERE sandbox_mode = 'inherit';

UPDATE company_codex_runner_profiles
SET approval_policy = 'never'
WHERE approval_policy = 'inherit';

UPDATE agent_codex_trigger_configs
SET sandbox_mode = 'workspace_write'
WHERE sandbox_mode = 'inherit';

UPDATE agent_codex_trigger_configs
SET approval_policy = 'never'
WHERE approval_policy = 'inherit';

ALTER TABLE company_codex_runner_profiles
    DROP CONSTRAINT IF EXISTS company_codex_runner_profiles_sandbox_mode_check,
    DROP CONSTRAINT IF EXISTS company_codex_runner_profiles_approval_policy_check,
    ADD CONSTRAINT company_codex_runner_profiles_sandbox_mode_check
        CHECK (sandbox_mode IN ('read_only', 'workspace_write')),
    ADD CONSTRAINT company_codex_runner_profiles_approval_policy_check
        CHECK (approval_policy IN ('never', 'on-request')),
    DROP COLUMN IF EXISTS feature_shell_tool,
    DROP COLUMN IF EXISTS feature_goals,
    DROP COLUMN IF EXISTS feature_hooks,
    DROP COLUMN IF EXISTS feature_remote_plugin,
    DROP COLUMN IF EXISTS feature_multi_agent,
    DROP COLUMN IF EXISTS web_search,
    DROP COLUMN IF EXISTS network_access,
    DROP COLUMN IF EXISTS service_tier,
    DROP COLUMN IF EXISTS personality,
    DROP COLUMN IF EXISTS verbosity,
    DROP COLUMN IF EXISTS reasoning_summary;

ALTER TABLE agent_codex_trigger_configs
    DROP CONSTRAINT IF EXISTS agent_codex_trigger_configs_sandbox_mode_check,
    DROP CONSTRAINT IF EXISTS agent_codex_trigger_configs_approval_policy_check,
    ADD CONSTRAINT agent_codex_trigger_configs_sandbox_mode_check
        CHECK (sandbox_mode IN ('read_only', 'workspace_write')),
    ADD CONSTRAINT agent_codex_trigger_configs_approval_policy_check
        CHECK (approval_policy IN ('never', 'on-request')),
    DROP COLUMN IF EXISTS feature_shell_tool,
    DROP COLUMN IF EXISTS feature_goals,
    DROP COLUMN IF EXISTS feature_hooks,
    DROP COLUMN IF EXISTS feature_remote_plugin,
    DROP COLUMN IF EXISTS feature_multi_agent,
    DROP COLUMN IF EXISTS web_search,
    DROP COLUMN IF EXISTS network_access,
    DROP COLUMN IF EXISTS service_tier,
    DROP COLUMN IF EXISTS personality,
    DROP COLUMN IF EXISTS verbosity,
    DROP COLUMN IF EXISTS reasoning_summary;
