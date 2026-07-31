DROP TRIGGER IF EXISTS trg_agent_runtime_configs_approval_policy_realtime_update
    ON agent_runtime_configs;
DROP FUNCTION IF EXISTS emit_agent_runtime_approval_policy_realtime_event();
DROP TRIGGER IF EXISTS trg_agent_tool_approvals_realtime_update
    ON agent_tool_approval_requests;
DROP TRIGGER IF EXISTS trg_agent_tool_approvals_realtime_insert
    ON agent_tool_approval_requests;
DROP FUNCTION IF EXISTS emit_agent_tool_approval_realtime_event();
DROP TABLE IF EXISTS agent_tool_approval_requests;

ALTER TABLE agent_runtime_runs
    DROP COLUMN IF EXISTS approval_request_count;

ALTER TABLE agent_runtime_configs
    DROP COLUMN IF EXISTS daily_approval_request_budget,
    DROP COLUMN IF EXISTS approval_request_ttl_minutes,
    DROP COLUMN IF EXISTS approval_required_model_actions;
