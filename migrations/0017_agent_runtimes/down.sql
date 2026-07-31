DROP TRIGGER IF EXISTS trg_agent_runtime_runs_realtime_update ON agent_runtime_runs;
DROP TRIGGER IF EXISTS trg_agent_runtime_runs_realtime_insert ON agent_runtime_runs;
DROP FUNCTION IF EXISTS emit_agent_runtime_run_realtime_event();

DROP TRIGGER IF EXISTS trg_agent_runtime_configs_realtime_update ON agent_runtime_configs;
DROP TRIGGER IF EXISTS trg_agent_runtime_configs_realtime_insert ON agent_runtime_configs;
DROP FUNCTION IF EXISTS emit_agent_runtime_config_realtime_event();

DROP TABLE IF EXISTS agent_runtime_runs;
DROP TABLE IF EXISTS agent_runtime_configs;
