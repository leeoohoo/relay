ALTER TABLE agent_runtime_configs
    DROP CONSTRAINT agent_runtime_configs_executor_kind_check,
    ADD CONSTRAINT agent_runtime_configs_executor_kind_check
        CHECK (executor_kind IN ('rules_v1', 'model_v1')),
    ADD COLUMN model_provider TEXT NOT NULL DEFAULT 'openai_responses'
        CHECK (model_provider IN ('openai_responses')),
    ADD COLUMN allowed_model_actions JSONB NOT NULL DEFAULT '["company.chat.message.send", "company.project.task.start_assigned", "company.project.status.update"]'::jsonb
        CHECK (jsonb_typeof(allowed_model_actions) = 'array'),
    ADD COLUMN model_max_output_tokens INTEGER NOT NULL DEFAULT 1200
        CHECK (model_max_output_tokens BETWEEN 64 AND 8192),
    ADD COLUMN model_timeout_seconds INTEGER NOT NULL DEFAULT 30
        CHECK (model_timeout_seconds BETWEEN 1 AND 120),
    ADD COLUMN model_max_retries INTEGER NOT NULL DEFAULT 1
        CHECK (model_max_retries BETWEEN 0 AND 3),
    ADD COLUMN fallback_to_rules_v1 BOOLEAN NOT NULL DEFAULT TRUE,
    ADD COLUMN daily_model_input_token_budget BIGINT NOT NULL DEFAULT 2000000
        CHECK (daily_model_input_token_budget BETWEEN 1000 AND 1000000000),
    ADD COLUMN daily_model_output_token_budget BIGINT NOT NULL DEFAULT 500000
        CHECK (daily_model_output_token_budget BETWEEN 1000 AND 1000000000);

ALTER TABLE agent_runtime_runs
    ADD COLUMN executor_kind TEXT NOT NULL DEFAULT 'rules_v1'
        CHECK (executor_kind IN ('rules_v1', 'model_v1')),
    ADD COLUMN model_request_count INTEGER NOT NULL DEFAULT 0
        CHECK (model_request_count >= 0),
    ADD COLUMN model_input_tokens BIGINT NOT NULL DEFAULT 0
        CHECK (model_input_tokens >= 0),
    ADD COLUMN model_output_tokens BIGINT NOT NULL DEFAULT 0
        CHECK (model_output_tokens >= 0);

CREATE FUNCTION emit_agent_runtime_model_policy_realtime_event()
RETURNS TRIGGER AS $$
BEGIN
    IF OLD.model_provider IS NOT DISTINCT FROM NEW.model_provider
       AND OLD.allowed_model_actions IS NOT DISTINCT FROM NEW.allowed_model_actions
       AND OLD.model_max_output_tokens IS NOT DISTINCT FROM NEW.model_max_output_tokens
       AND OLD.model_timeout_seconds IS NOT DISTINCT FROM NEW.model_timeout_seconds
       AND OLD.model_max_retries IS NOT DISTINCT FROM NEW.model_max_retries
       AND OLD.fallback_to_rules_v1 IS NOT DISTINCT FROM NEW.fallback_to_rules_v1
       AND OLD.daily_model_input_token_budget IS NOT DISTINCT FROM NEW.daily_model_input_token_budget
       AND OLD.daily_model_output_token_budget IS NOT DISTINCT FROM NEW.daily_model_output_token_budget THEN
        RETURN NEW;
    END IF;

    INSERT INTO realtime_events (
        company_id, event_type, aggregate_type, aggregate_id,
        actor_human_user_id, payload, created_at
    ) VALUES (
        NEW.company_id,
        'agent.runtime.model_policy_updated',
        'agent_runtime_config',
        NEW.id,
        NEW.updated_by_human_user_id,
        jsonb_build_object(
            'runtime_config_id', NEW.id,
            'agent_profile_id', NEW.agent_profile_id,
            'model_provider', NEW.model_provider,
            'allowed_model_actions', NEW.allowed_model_actions,
            'model_max_output_tokens', NEW.model_max_output_tokens,
            'model_timeout_seconds', NEW.model_timeout_seconds,
            'model_max_retries', NEW.model_max_retries,
            'fallback_to_rules_v1', NEW.fallback_to_rules_v1,
            'daily_model_input_token_budget', NEW.daily_model_input_token_budget,
            'daily_model_output_token_budget', NEW.daily_model_output_token_budget
        ),
        NEW.updated_at
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_agent_runtime_configs_model_policy_realtime_update
AFTER UPDATE OF model_provider, allowed_model_actions,
    model_max_output_tokens, model_timeout_seconds, model_max_retries,
    fallback_to_rules_v1, daily_model_input_token_budget,
    daily_model_output_token_budget
ON agent_runtime_configs
FOR EACH ROW EXECUTE FUNCTION emit_agent_runtime_model_policy_realtime_event();
