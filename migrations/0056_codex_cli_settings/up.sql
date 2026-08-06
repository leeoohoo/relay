ALTER TABLE company_codex_runner_profiles
    ADD COLUMN reasoning_summary TEXT
        CHECK (reasoning_summary IS NULL OR reasoning_summary IN ('auto', 'concise', 'detailed', 'none')),
    ADD COLUMN verbosity TEXT
        CHECK (verbosity IS NULL OR verbosity IN ('low', 'medium', 'high')),
    ADD COLUMN personality TEXT
        CHECK (personality IS NULL OR personality IN ('none', 'friendly', 'pragmatic')),
    ADD COLUMN service_tier TEXT
        CHECK (service_tier IS NULL OR service_tier IN ('fast')),
    ADD COLUMN network_access BOOLEAN,
    ADD COLUMN web_search TEXT
        CHECK (web_search IS NULL OR web_search IN ('disabled', 'cached', 'indexed', 'live')),
    ADD COLUMN feature_multi_agent BOOLEAN,
    ADD COLUMN feature_remote_plugin BOOLEAN,
    ADD COLUMN feature_hooks BOOLEAN,
    ADD COLUMN feature_goals BOOLEAN,
    ADD COLUMN feature_shell_tool BOOLEAN;

ALTER TABLE agent_codex_trigger_configs
    ADD COLUMN reasoning_summary TEXT
        CHECK (reasoning_summary IS NULL OR reasoning_summary IN ('auto', 'concise', 'detailed', 'none')),
    ADD COLUMN verbosity TEXT
        CHECK (verbosity IS NULL OR verbosity IN ('low', 'medium', 'high')),
    ADD COLUMN personality TEXT
        CHECK (personality IS NULL OR personality IN ('none', 'friendly', 'pragmatic')),
    ADD COLUMN service_tier TEXT
        CHECK (service_tier IS NULL OR service_tier IN ('fast')),
    ADD COLUMN network_access BOOLEAN,
    ADD COLUMN web_search TEXT
        CHECK (web_search IS NULL OR web_search IN ('disabled', 'cached', 'indexed', 'live')),
    ADD COLUMN feature_multi_agent BOOLEAN,
    ADD COLUMN feature_remote_plugin BOOLEAN,
    ADD COLUMN feature_hooks BOOLEAN,
    ADD COLUMN feature_goals BOOLEAN,
    ADD COLUMN feature_shell_tool BOOLEAN;

ALTER TABLE company_codex_runner_profiles
    DROP CONSTRAINT IF EXISTS company_codex_runner_profiles_sandbox_mode_check,
    DROP CONSTRAINT IF EXISTS company_codex_runner_profiles_approval_policy_check,
    ADD CONSTRAINT company_codex_runner_profiles_sandbox_mode_check
        CHECK (sandbox_mode IN ('inherit', 'read_only', 'workspace_write')),
    ADD CONSTRAINT company_codex_runner_profiles_approval_policy_check
        CHECK (approval_policy IN ('inherit', 'never', 'on-request'));

ALTER TABLE agent_codex_trigger_configs
    DROP CONSTRAINT IF EXISTS agent_codex_trigger_configs_sandbox_mode_check,
    DROP CONSTRAINT IF EXISTS agent_codex_trigger_configs_approval_policy_check,
    ADD CONSTRAINT agent_codex_trigger_configs_sandbox_mode_check
        CHECK (sandbox_mode IN ('inherit', 'read_only', 'workspace_write')),
    ADD CONSTRAINT agent_codex_trigger_configs_approval_policy_check
        CHECK (approval_policy IN ('inherit', 'never', 'on-request'));
