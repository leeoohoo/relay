DROP TRIGGER IF EXISTS trg_company_model_budget_policy_realtime_update
    ON company_model_budget_policies;
DROP TRIGGER IF EXISTS trg_company_model_budget_policy_realtime_insert
    ON company_model_budget_policies;
DROP FUNCTION IF EXISTS emit_company_model_budget_policy_realtime_event();

DROP TRIGGER IF EXISTS trg_agent_runtime_configs_cost_policy_realtime_update
    ON agent_runtime_configs;
DROP FUNCTION IF EXISTS emit_agent_runtime_cost_policy_realtime_event();

UPDATE agent_runtime_templates
SET settings = settings
    - 'daily_model_cost_budget_microusd'
    - 'model_input_price_microusd_per_million_tokens'
    - 'model_output_price_microusd_per_million_tokens';

DROP TABLE IF EXISTS company_model_budget_policies;

ALTER TABLE agent_runtime_runs
    DROP COLUMN IF EXISTS model_pricing_status,
    DROP COLUMN IF EXISTS model_output_price_microusd_per_million_tokens,
    DROP COLUMN IF EXISTS model_input_price_microusd_per_million_tokens,
    DROP COLUMN IF EXISTS model_cost_microusd;

ALTER TABLE agent_runtime_configs
    DROP CONSTRAINT IF EXISTS agent_runtime_configs_model_prices_pair_check,
    DROP COLUMN IF EXISTS model_output_price_microusd_per_million_tokens,
    DROP COLUMN IF EXISTS model_input_price_microusd_per_million_tokens,
    DROP COLUMN IF EXISTS daily_model_cost_budget_microusd;
