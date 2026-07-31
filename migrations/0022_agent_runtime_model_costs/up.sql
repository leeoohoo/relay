ALTER TABLE agent_runtime_configs
    ADD COLUMN daily_model_cost_budget_microusd BIGINT NOT NULL DEFAULT 5000000
        CHECK (daily_model_cost_budget_microusd BETWEEN 1 AND 1000000000000),
    ADD COLUMN model_input_price_microusd_per_million_tokens BIGINT NOT NULL DEFAULT 0
        CHECK (model_input_price_microusd_per_million_tokens BETWEEN 0 AND 1000000000000),
    ADD COLUMN model_output_price_microusd_per_million_tokens BIGINT NOT NULL DEFAULT 0
        CHECK (model_output_price_microusd_per_million_tokens BETWEEN 0 AND 1000000000000),
    ADD CONSTRAINT agent_runtime_configs_model_prices_pair_check CHECK (
        (model_input_price_microusd_per_million_tokens = 0
            AND model_output_price_microusd_per_million_tokens = 0)
        OR
        (model_input_price_microusd_per_million_tokens > 0
            AND model_output_price_microusd_per_million_tokens > 0)
    );

ALTER TABLE agent_runtime_runs
    ADD COLUMN model_cost_microusd BIGINT NOT NULL DEFAULT 0
        CHECK (model_cost_microusd >= 0),
    ADD COLUMN model_input_price_microusd_per_million_tokens BIGINT NOT NULL DEFAULT 0
        CHECK (model_input_price_microusd_per_million_tokens >= 0),
    ADD COLUMN model_output_price_microusd_per_million_tokens BIGINT NOT NULL DEFAULT 0
        CHECK (model_output_price_microusd_per_million_tokens >= 0),
    ADD COLUMN model_pricing_status TEXT NOT NULL DEFAULT 'unpriced'
        CHECK (model_pricing_status IN ('priced', 'unpriced'));

CREATE TABLE company_model_budget_policies (
    company_id UUID PRIMARY KEY REFERENCES companies(id) ON DELETE CASCADE,
    daily_model_cost_budget_microusd BIGINT NOT NULL DEFAULT 100000000
        CHECK (daily_model_cost_budget_microusd BETWEEN 1 AND 1000000000000),
    created_by_human_user_id UUID REFERENCES human_users(id) ON DELETE SET NULL,
    updated_by_human_user_id UUID REFERENCES human_users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

UPDATE agent_runtime_templates
SET settings = settings || jsonb_build_object(
    'daily_model_cost_budget_microusd', 5000000,
    'model_input_price_microusd_per_million_tokens', 0,
    'model_output_price_microusd_per_million_tokens', 0
)
WHERE NOT settings ? 'daily_model_cost_budget_microusd'
   OR NOT settings ? 'model_input_price_microusd_per_million_tokens'
   OR NOT settings ? 'model_output_price_microusd_per_million_tokens';

CREATE FUNCTION emit_agent_runtime_cost_policy_realtime_event()
RETURNS TRIGGER AS $$
BEGIN
    IF OLD.daily_model_cost_budget_microusd IS NOT DISTINCT FROM NEW.daily_model_cost_budget_microusd
       AND OLD.model_input_price_microusd_per_million_tokens IS NOT DISTINCT FROM NEW.model_input_price_microusd_per_million_tokens
       AND OLD.model_output_price_microusd_per_million_tokens IS NOT DISTINCT FROM NEW.model_output_price_microusd_per_million_tokens THEN
        RETURN NEW;
    END IF;
    INSERT INTO realtime_events (
        company_id, event_type, aggregate_type, aggregate_id,
        actor_human_user_id, payload, created_at
    ) VALUES (
        NEW.company_id,
        'agent.runtime.cost_policy_updated',
        'agent_runtime_config',
        NEW.id,
        NEW.updated_by_human_user_id,
        jsonb_build_object(
            'runtime_config_id', NEW.id,
            'agent_profile_id', NEW.agent_profile_id,
            'daily_model_cost_budget_microusd', NEW.daily_model_cost_budget_microusd,
            'model_input_price_microusd_per_million_tokens', NEW.model_input_price_microusd_per_million_tokens,
            'model_output_price_microusd_per_million_tokens', NEW.model_output_price_microusd_per_million_tokens
        ),
        NEW.updated_at
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_agent_runtime_configs_cost_policy_realtime_update
AFTER UPDATE OF daily_model_cost_budget_microusd,
    model_input_price_microusd_per_million_tokens,
    model_output_price_microusd_per_million_tokens
ON agent_runtime_configs
FOR EACH ROW EXECUTE FUNCTION emit_agent_runtime_cost_policy_realtime_event();

CREATE FUNCTION emit_company_model_budget_policy_realtime_event()
RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO realtime_events (
        company_id, event_type, aggregate_type, aggregate_id,
        actor_human_user_id, payload, created_at
    ) VALUES (
        NEW.company_id,
        'company.model_budget.updated',
        'company_model_budget_policy',
        NEW.company_id,
        COALESCE(NEW.updated_by_human_user_id, NEW.created_by_human_user_id),
        jsonb_build_object(
            'company_id', NEW.company_id,
            'daily_model_cost_budget_microusd', NEW.daily_model_cost_budget_microusd
        ),
        NEW.updated_at
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_company_model_budget_policy_realtime_insert
AFTER INSERT ON company_model_budget_policies
FOR EACH ROW EXECUTE FUNCTION emit_company_model_budget_policy_realtime_event();

CREATE TRIGGER trg_company_model_budget_policy_realtime_update
AFTER UPDATE OF daily_model_cost_budget_microusd ON company_model_budget_policies
FOR EACH ROW EXECUTE FUNCTION emit_company_model_budget_policy_realtime_event();
