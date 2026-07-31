ALTER TABLE agent_codex_trigger_runs
    DROP CONSTRAINT agent_codex_trigger_runs_trigger_type_check,
    ADD CONSTRAINT agent_codex_trigger_runs_trigger_type_check
        CHECK (trigger_type IN ('scheduled', 'manual', 'message', 'task'));

DROP TABLE IF EXISTS company_project_asset_refresh_configs;
DROP TABLE IF EXISTS company_project_assets;
DROP TABLE IF EXISTS company_project_rules;
