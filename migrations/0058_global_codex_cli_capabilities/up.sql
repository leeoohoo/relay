UPDATE company_codex_runner_profiles
SET service_tier = NULL,
    network_access = NULL,
    web_search = NULL,
    feature_multi_agent = NULL,
    feature_remote_plugin = NULL,
    feature_hooks = NULL,
    feature_goals = NULL,
    feature_shell_tool = NULL
WHERE service_tier IS NOT NULL
   OR network_access IS NOT NULL
   OR web_search IS NOT NULL
   OR feature_multi_agent IS NOT NULL
   OR feature_remote_plugin IS NOT NULL
   OR feature_hooks IS NOT NULL
   OR feature_goals IS NOT NULL
   OR feature_shell_tool IS NOT NULL;

UPDATE agent_codex_trigger_configs
SET service_tier = NULL,
    network_access = NULL,
    web_search = NULL,
    feature_multi_agent = NULL,
    feature_remote_plugin = NULL,
    feature_hooks = NULL,
    feature_goals = NULL,
    feature_shell_tool = NULL
WHERE service_tier IS NOT NULL
   OR network_access IS NOT NULL
   OR web_search IS NOT NULL
   OR feature_multi_agent IS NOT NULL
   OR feature_remote_plugin IS NOT NULL
   OR feature_hooks IS NOT NULL
   OR feature_goals IS NOT NULL
   OR feature_shell_tool IS NOT NULL;
