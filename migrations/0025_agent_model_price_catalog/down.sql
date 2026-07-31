DROP TRIGGER IF EXISTS trg_agent_runtime_configs_model_price_catalog_update
    ON agent_runtime_configs;
DROP TRIGGER IF EXISTS trg_agent_runtime_configs_model_price_catalog_insert
    ON agent_runtime_configs;
DROP FUNCTION IF EXISTS emit_agent_runtime_model_price_catalog_applied_event();

DROP TRIGGER IF EXISTS trg_agent_model_price_catalog_realtime_update
    ON agent_model_price_catalog_entries;
DROP TRIGGER IF EXISTS trg_agent_model_price_catalog_realtime_insert
    ON agent_model_price_catalog_entries;
DROP FUNCTION IF EXISTS emit_agent_model_price_catalog_realtime_event();

UPDATE agent_runtime_templates
SET settings = settings - 'model_price_catalog_entry_id';

DROP INDEX IF EXISTS idx_agent_runtime_configs_model_price_catalog;
ALTER TABLE agent_runtime_configs
    DROP COLUMN IF EXISTS model_price_catalog_entry_id;

DROP TABLE IF EXISTS agent_model_price_catalog_entries;
