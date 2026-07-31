DROP TRIGGER IF EXISTS trg_agent_runtime_configs_secret_reference_rotated
    ON agent_runtime_configs;
DROP FUNCTION IF EXISTS emit_agent_runtime_secret_reference_rotated_event();
