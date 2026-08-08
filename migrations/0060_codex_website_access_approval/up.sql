ALTER TABLE agent_tool_approval_requests
    DROP CONSTRAINT agent_tool_approval_requests_tool_name_check,
    ADD CONSTRAINT agent_tool_approval_requests_tool_name_check CHECK (tool_name IN (
        'agent.staff.hire',
        'agent.staff.suspend',
        'agent.staff.terminate',
        'company.project.task.reassign',
        'codex.command_execution',
        'codex.file_change',
        'codex.permissions',
        'codex.website_access'
    ));
