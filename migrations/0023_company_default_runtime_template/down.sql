DROP TRIGGER IF EXISTS trg_agent_runtime_configs_default_template_applied_realtime
    ON agent_runtime_configs;
DROP FUNCTION IF EXISTS emit_default_runtime_template_applied_realtime_event();

DROP TRIGGER IF EXISTS trg_agent_runtime_templates_clear_company_default
    ON agent_runtime_templates;
DROP FUNCTION IF EXISTS clear_archived_company_default_runtime_template();

DROP TRIGGER IF EXISTS trg_company_runtime_policies_realtime_update
    ON company_runtime_policies;
DROP TRIGGER IF EXISTS trg_company_runtime_policies_realtime_insert
    ON company_runtime_policies;
DROP FUNCTION IF EXISTS emit_company_runtime_policy_realtime_event();

DROP TABLE IF EXISTS company_runtime_policies;
