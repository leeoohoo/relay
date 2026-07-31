DROP TRIGGER IF EXISTS trg_agent_runtime_configs_template_applied_realtime
    ON agent_runtime_configs;
DROP FUNCTION IF EXISTS emit_agent_runtime_template_applied_realtime_event();

DROP TRIGGER IF EXISTS trg_agent_runtime_templates_realtime_update
    ON agent_runtime_templates;
DROP TRIGGER IF EXISTS trg_agent_runtime_templates_realtime_insert
    ON agent_runtime_templates;
DROP FUNCTION IF EXISTS emit_agent_runtime_template_realtime_event();

DROP INDEX IF EXISTS idx_agent_runtime_configs_template;
ALTER TABLE agent_runtime_configs
    DROP COLUMN IF EXISTS runtime_template_id;

DROP TABLE IF EXISTS agent_runtime_templates;
