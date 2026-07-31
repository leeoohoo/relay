ALTER TABLE company_codex_runner_profiles
    ADD COLUMN approval_policy TEXT NOT NULL DEFAULT 'never'
        CHECK (approval_policy IN ('never', 'on-request'));

ALTER TABLE agent_codex_trigger_configs
    ADD COLUMN approval_policy TEXT NOT NULL DEFAULT 'never'
        CHECK (approval_policy IN ('never', 'on-request'));

ALTER TABLE agent_tool_approval_requests
    DROP CONSTRAINT agent_tool_approval_requests_tool_name_check,
    DROP CONSTRAINT agent_tool_approval_requests_status_check,
    DROP CONSTRAINT agent_tool_approval_requests_risk_level_check,
    ADD COLUMN approval_source TEXT NOT NULL DEFAULT 'runtime_model'
        CHECK (approval_source IN ('runtime_model', 'codex')),
    ADD COLUMN codex_trigger_run_id UUID
        REFERENCES agent_codex_trigger_runs(id) ON DELETE SET NULL,
    ADD CONSTRAINT agent_tool_approval_requests_tool_name_check CHECK (tool_name IN (
        'agent.staff.hire',
        'agent.staff.suspend',
        'agent.staff.terminate',
        'company.project.task.reassign',
        'codex.command_execution',
        'codex.file_change',
        'codex.permissions'
    )),
    ADD CONSTRAINT agent_tool_approval_requests_status_check CHECK (status IN (
        'pending', 'approved', 'executing', 'executed', 'rejected', 'expired', 'failed'
    )),
    ADD CONSTRAINT agent_tool_approval_requests_risk_level_check CHECK (risk_level IN (
        'low', 'medium', 'high'
    ));

CREATE INDEX idx_agent_tool_approvals_codex_run
    ON agent_tool_approval_requests(codex_trigger_run_id, created_at DESC)
    WHERE codex_trigger_run_id IS NOT NULL;

CREATE OR REPLACE FUNCTION emit_agent_tool_approval_realtime_event()
RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO realtime_events (
        company_id, event_type, aggregate_type, aggregate_id,
        actor_agent_id, actor_human_user_id, payload, created_at
    ) VALUES (
        NEW.company_id,
        CASE WHEN TG_OP = 'INSERT'
            THEN 'agent.runtime.approval_requested'
            ELSE 'agent.runtime.approval_updated'
        END,
        'agent_tool_approval_request',
        NEW.id,
        NEW.requested_by_agent_id,
        NEW.reviewed_by_human_user_id,
        jsonb_build_object(
            'approval_request_id', NEW.id,
            'approval_source', NEW.approval_source,
            'runtime_config_id', NEW.runtime_config_id,
            'runtime_run_id', NEW.runtime_run_id,
            'codex_trigger_run_id', NEW.codex_trigger_run_id,
            'requested_by_agent_id', NEW.requested_by_agent_id,
            'tool_name', NEW.tool_name,
            'risk_level', NEW.risk_level,
            'status', NEW.status,
            'expires_at', NEW.expires_at,
            'error_message', NEW.error_message
        ),
        NEW.updated_at
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
