DROP TRIGGER IF EXISTS trg_agent_runtime_configs_model_policy_realtime_update
    ON agent_runtime_configs;
DROP FUNCTION IF EXISTS emit_agent_runtime_model_policy_realtime_event();

ALTER TABLE agent_runtime_runs
    DROP COLUMN IF EXISTS model_output_tokens,
    DROP COLUMN IF EXISTS model_input_tokens,
    DROP COLUMN IF EXISTS model_request_count,
    DROP COLUMN IF EXISTS executor_kind;

ALTER TABLE agent_runtime_configs
    DROP COLUMN IF EXISTS daily_model_output_token_budget,
    DROP COLUMN IF EXISTS daily_model_input_token_budget,
    DROP COLUMN IF EXISTS fallback_to_rules_v1,
    DROP COLUMN IF EXISTS model_max_retries,
    DROP COLUMN IF EXISTS model_timeout_seconds,
    DROP COLUMN IF EXISTS model_max_output_tokens,
    DROP COLUMN IF EXISTS allowed_model_actions,
    DROP COLUMN IF EXISTS model_provider,
    DROP CONSTRAINT agent_runtime_configs_executor_kind_check,
    ADD CONSTRAINT agent_runtime_configs_executor_kind_check
        CHECK (executor_kind IN ('rules_v1'));
