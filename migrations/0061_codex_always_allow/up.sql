CREATE INDEX idx_agent_tool_approvals_always_allow
    ON agent_tool_approval_requests (
        company_id,
        requested_by_agent_id,
        tool_name,
        (execution_result ->> 'approval_scope'),
        (execution_result ->> 'approval_target')
    )
    WHERE approval_source = 'codex'
      AND status IN ('approved', 'executed')
      AND execution_result ->> 'approval_mode' = 'always';
