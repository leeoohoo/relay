CREATE TABLE agent_model_price_catalog_entries (
    id UUID PRIMARY KEY,
    company_id UUID NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    model_provider TEXT NOT NULL CHECK (char_length(model_provider) BETWEEN 1 AND 80),
    model_name TEXT NOT NULL CHECK (char_length(model_name) BETWEEN 1 AND 120),
    version INTEGER NOT NULL CHECK (version >= 1),
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'archived')),
    input_price_microusd_per_million_tokens BIGINT NOT NULL
        CHECK (input_price_microusd_per_million_tokens BETWEEN 1 AND 1000000000000),
    output_price_microusd_per_million_tokens BIGINT NOT NULL
        CHECK (output_price_microusd_per_million_tokens BETWEEN 1 AND 1000000000000),
    notes TEXT NOT NULL DEFAULT '' CHECK (char_length(notes) <= 1000),
    source_agent_profile_id UUID REFERENCES agent_profiles(id) ON DELETE SET NULL,
    created_by_human_user_id UUID NOT NULL REFERENCES human_users(id) ON DELETE RESTRICT,
    updated_by_human_user_id UUID REFERENCES human_users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (company_id, model_provider, model_name, version)
);

CREATE UNIQUE INDEX uq_agent_model_price_catalog_active_model
    ON agent_model_price_catalog_entries(company_id, model_provider, model_name)
    WHERE status = 'active';

CREATE INDEX idx_agent_model_price_catalog_company_model
    ON agent_model_price_catalog_entries(
        company_id, model_provider, model_name, version DESC
    );

ALTER TABLE agent_runtime_configs
    ADD COLUMN model_price_catalog_entry_id UUID
        REFERENCES agent_model_price_catalog_entries(id) ON DELETE SET NULL;

CREATE INDEX idx_agent_runtime_configs_model_price_catalog
    ON agent_runtime_configs(model_price_catalog_entry_id)
    WHERE model_price_catalog_entry_id IS NOT NULL;

UPDATE agent_runtime_templates
SET settings = settings || jsonb_build_object('model_price_catalog_entry_id', NULL)
WHERE NOT settings ? 'model_price_catalog_entry_id';

CREATE FUNCTION emit_agent_model_price_catalog_realtime_event()
RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO realtime_events (
        company_id, event_type, aggregate_type, aggregate_id,
        actor_human_user_id, payload, created_at
    ) VALUES (
        NEW.company_id,
        CASE
            WHEN TG_OP = 'INSERT' THEN 'company.model_price.published'
            WHEN NEW.status = 'archived' AND OLD.status IS DISTINCT FROM NEW.status
                THEN 'company.model_price.archived'
            ELSE 'company.model_price.updated'
        END,
        'agent_model_price_catalog_entry',
        NEW.id,
        COALESCE(NEW.updated_by_human_user_id, NEW.created_by_human_user_id),
        jsonb_build_object(
            'catalog_entry_id', NEW.id,
            'model_provider', NEW.model_provider,
            'model_name', NEW.model_name,
            'version', NEW.version,
            'status', NEW.status,
            'input_price_microusd_per_million_tokens',
                NEW.input_price_microusd_per_million_tokens,
            'output_price_microusd_per_million_tokens',
                NEW.output_price_microusd_per_million_tokens
        ),
        NEW.updated_at
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_agent_model_price_catalog_realtime_insert
AFTER INSERT ON agent_model_price_catalog_entries
FOR EACH ROW EXECUTE FUNCTION emit_agent_model_price_catalog_realtime_event();

CREATE TRIGGER trg_agent_model_price_catalog_realtime_update
AFTER UPDATE OF status, notes ON agent_model_price_catalog_entries
FOR EACH ROW EXECUTE FUNCTION emit_agent_model_price_catalog_realtime_event();

CREATE FUNCTION emit_agent_runtime_model_price_catalog_applied_event()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'UPDATE'
       AND OLD.model_price_catalog_entry_id IS NOT DISTINCT FROM NEW.model_price_catalog_entry_id THEN
        RETURN NEW;
    END IF;
    IF NEW.model_price_catalog_entry_id IS NULL THEN
        RETURN NEW;
    END IF;
    INSERT INTO realtime_events (
        company_id, event_type, aggregate_type, aggregate_id,
        actor_human_user_id, payload, created_at
    ) VALUES (
        NEW.company_id,
        'agent.runtime.model_price_applied',
        'agent_runtime_config',
        NEW.id,
        NEW.updated_by_human_user_id,
        jsonb_build_object(
            'runtime_config_id', NEW.id,
            'agent_profile_id', NEW.agent_profile_id,
            'catalog_entry_id', NEW.model_price_catalog_entry_id,
            'input_price_microusd_per_million_tokens',
                NEW.model_input_price_microusd_per_million_tokens,
            'output_price_microusd_per_million_tokens',
                NEW.model_output_price_microusd_per_million_tokens
        ),
        NEW.updated_at
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_agent_runtime_configs_model_price_catalog_insert
AFTER INSERT ON agent_runtime_configs
FOR EACH ROW EXECUTE FUNCTION emit_agent_runtime_model_price_catalog_applied_event();

CREATE TRIGGER trg_agent_runtime_configs_model_price_catalog_update
AFTER UPDATE OF model_price_catalog_entry_id ON agent_runtime_configs
FOR EACH ROW EXECUTE FUNCTION emit_agent_runtime_model_price_catalog_applied_event();
