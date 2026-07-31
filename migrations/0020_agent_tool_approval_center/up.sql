ALTER TABLE agent_runtime_configs
    ADD COLUMN approval_required_model_actions JSONB NOT NULL DEFAULT '[]'::jsonb
        CHECK (jsonb_typeof(approval_required_model_actions) = 'array'),
    ADD COLUMN approval_request_ttl_minutes INTEGER NOT NULL DEFAULT 1440
        CHECK (approval_request_ttl_minutes BETWEEN 5 AND 10080),
    ADD COLUMN daily_approval_request_budget INTEGER NOT NULL DEFAULT 20
        CHECK (daily_approval_request_budget BETWEEN 1 AND 1000);

ALTER TABLE agent_runtime_runs
    ADD COLUMN approval_request_count INTEGER NOT NULL DEFAULT 0
        CHECK (approval_request_count >= 0);

CREATE TABLE agent_tool_approval_requests (
    id UUID PRIMARY KEY,
    company_id UUID NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    runtime_config_id UUID REFERENCES agent_runtime_configs(id) ON DELETE SET NULL,
    runtime_run_id UUID REFERENCES agent_runtime_runs(id) ON DELETE SET NULL,
    requested_by_agent_id UUID NOT NULL REFERENCES agent_profiles(id) ON DELETE RESTRICT,
    tool_name TEXT NOT NULL CHECK (tool_name IN (
        'agent.staff.hire',
        'agent.staff.suspend',
        'agent.staff.terminate',
        'company.project.task.reassign'
    )),
    risk_level TEXT NOT NULL DEFAULT 'high' CHECK (risk_level IN ('high')),
    reason TEXT NOT NULL DEFAULT '',
    arguments JSONB NOT NULL DEFAULT '{}'::jsonb CHECK (jsonb_typeof(arguments) = 'object'),
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN (
        'pending', 'executing', 'executed', 'rejected', 'expired', 'failed'
    )),
    expires_at TIMESTAMPTZ NOT NULL,
    reviewed_by_human_user_id UUID REFERENCES human_users(id) ON DELETE SET NULL,
    review_note TEXT NOT NULL DEFAULT '',
    reviewed_at TIMESTAMPTZ,
    execution_result JSONB NOT NULL DEFAULT '{}'::jsonb,
    error_message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_agent_tool_approvals_company_status
    ON agent_tool_approval_requests(company_id, status, created_at DESC);

CREATE INDEX idx_agent_tool_approvals_expiry
    ON agent_tool_approval_requests(status, expires_at)
    WHERE status = 'pending';

CREATE FUNCTION emit_agent_tool_approval_realtime_event()
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
            'runtime_config_id', NEW.runtime_config_id,
            'runtime_run_id', NEW.runtime_run_id,
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

CREATE TRIGGER trg_agent_tool_approvals_realtime_insert
AFTER INSERT ON agent_tool_approval_requests
FOR EACH ROW EXECUTE FUNCTION emit_agent_tool_approval_realtime_event();

CREATE TRIGGER trg_agent_tool_approvals_realtime_update
AFTER UPDATE OF status, reviewed_by_human_user_id, review_note,
    reviewed_at, execution_result, error_message
ON agent_tool_approval_requests
FOR EACH ROW EXECUTE FUNCTION emit_agent_tool_approval_realtime_event();

CREATE FUNCTION emit_agent_runtime_approval_policy_realtime_event()
RETURNS TRIGGER AS $$
BEGIN
    IF OLD.approval_required_model_actions IS NOT DISTINCT FROM NEW.approval_required_model_actions
       AND OLD.approval_request_ttl_minutes IS NOT DISTINCT FROM NEW.approval_request_ttl_minutes
       AND OLD.daily_approval_request_budget IS NOT DISTINCT FROM NEW.daily_approval_request_budget THEN
        RETURN NEW;
    END IF;
    INSERT INTO realtime_events (
        company_id, event_type, aggregate_type, aggregate_id,
        actor_human_user_id, payload, created_at
    ) VALUES (
        NEW.company_id,
        'agent.runtime.approval_policy_updated',
        'agent_runtime_config',
        NEW.id,
        NEW.updated_by_human_user_id,
        jsonb_build_object(
            'runtime_config_id', NEW.id,
            'agent_profile_id', NEW.agent_profile_id,
            'approval_required_model_actions', NEW.approval_required_model_actions,
            'approval_request_ttl_minutes', NEW.approval_request_ttl_minutes,
            'daily_approval_request_budget', NEW.daily_approval_request_budget
        ),
        NEW.updated_at
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_agent_runtime_configs_approval_policy_realtime_update
AFTER UPDATE OF approval_required_model_actions, approval_request_ttl_minutes,
    daily_approval_request_budget
ON agent_runtime_configs
FOR EACH ROW EXECUTE FUNCTION emit_agent_runtime_approval_policy_realtime_event();
