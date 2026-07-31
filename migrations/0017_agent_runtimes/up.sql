CREATE TABLE agent_runtime_configs (
    id UUID PRIMARY KEY,
    company_id UUID NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    agent_profile_id UUID NOT NULL UNIQUE REFERENCES agent_profiles(id) ON DELETE CASCADE,
    executor_kind TEXT NOT NULL DEFAULT 'rules_v1'
        CHECK (executor_kind IN ('rules_v1')),
    status TEXT NOT NULL DEFAULT 'active'
        CHECK (status IN ('active', 'paused', 'error')),
    interval_seconds INTEGER NOT NULL DEFAULT 30
        CHECK (interval_seconds BETWEEN 5 AND 86400),
    max_events INTEGER NOT NULL DEFAULT 5
        CHECK (max_events BETWEEN 1 AND 20),
    daily_run_budget INTEGER NOT NULL DEFAULT 500
        CHECK (daily_run_budget BETWEEN 1 AND 10000),
    daily_action_budget INTEGER NOT NULL DEFAULT 1000
        CHECK (daily_action_budget BETWEEN 1 AND 100000),
    system_prompt TEXT NOT NULL DEFAULT '',
    model_name TEXT,
    provider_secret_ref TEXT,
    next_run_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_run_at TIMESTAMPTZ,
    last_success_at TIMESTAMPTZ,
    last_error_at TIMESTAMPTZ,
    last_error TEXT,
    created_by_human_user_id UUID NOT NULL REFERENCES human_users(id) ON DELETE RESTRICT,
    updated_by_human_user_id UUID REFERENCES human_users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_agent_runtime_configs_company
    ON agent_runtime_configs(company_id, status);

CREATE INDEX idx_agent_runtime_configs_due
    ON agent_runtime_configs(status, next_run_at)
    WHERE status = 'active';

CREATE TABLE agent_runtime_runs (
    id UUID PRIMARY KEY,
    runtime_config_id UUID NOT NULL REFERENCES agent_runtime_configs(id) ON DELETE CASCADE,
    company_id UUID NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    agent_profile_id UUID NOT NULL REFERENCES agent_profiles(id) ON DELETE CASCADE,
    trigger_type TEXT NOT NULL CHECK (trigger_type IN ('scheduled', 'manual')),
    status TEXT NOT NULL CHECK (status IN ('running', 'succeeded', 'failed', 'skipped_budget')),
    input_event_count INTEGER NOT NULL DEFAULT 0 CHECK (input_event_count >= 0),
    processed_event_count INTEGER NOT NULL DEFAULT 0 CHECK (processed_event_count >= 0),
    action_count INTEGER NOT NULL DEFAULT 0 CHECK (action_count >= 0),
    remaining_pending_count INTEGER NOT NULL DEFAULT 0 CHECK (remaining_pending_count >= 0),
    input_payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    output_payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    error_message TEXT,
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    finished_at TIMESTAMPTZ
);

CREATE INDEX idx_agent_runtime_runs_runtime_started
    ON agent_runtime_runs(runtime_config_id, started_at DESC);

CREATE INDEX idx_agent_runtime_runs_agent_started
    ON agent_runtime_runs(agent_profile_id, started_at DESC);

CREATE FUNCTION emit_agent_runtime_config_realtime_event()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'UPDATE' THEN
        IF OLD.executor_kind IS NOT DISTINCT FROM NEW.executor_kind
           AND OLD.status IS NOT DISTINCT FROM NEW.status
           AND OLD.interval_seconds IS NOT DISTINCT FROM NEW.interval_seconds
           AND OLD.max_events IS NOT DISTINCT FROM NEW.max_events
           AND OLD.daily_run_budget IS NOT DISTINCT FROM NEW.daily_run_budget
           AND OLD.daily_action_budget IS NOT DISTINCT FROM NEW.daily_action_budget
           AND OLD.system_prompt IS NOT DISTINCT FROM NEW.system_prompt
           AND OLD.model_name IS NOT DISTINCT FROM NEW.model_name
           AND OLD.provider_secret_ref IS NOT DISTINCT FROM NEW.provider_secret_ref THEN
            RETURN NEW;
        END IF;
    END IF;

    INSERT INTO realtime_events (
        company_id, event_type, aggregate_type, aggregate_id,
        actor_human_user_id, payload, created_at
    ) VALUES (
        NEW.company_id,
        CASE WHEN TG_OP = 'INSERT' THEN 'agent.runtime.configured' ELSE 'agent.runtime.updated' END,
        'agent_runtime_config',
        NEW.id,
        CASE
            WHEN TG_OP = 'INSERT' THEN NEW.created_by_human_user_id
            ELSE NEW.updated_by_human_user_id
        END,
        jsonb_build_object(
            'runtime_config_id', NEW.id,
            'agent_profile_id', NEW.agent_profile_id,
            'executor_kind', NEW.executor_kind,
            'status', NEW.status,
            'interval_seconds', NEW.interval_seconds,
            'max_events', NEW.max_events,
            'daily_run_budget', NEW.daily_run_budget,
            'daily_action_budget', NEW.daily_action_budget
        ),
        NEW.updated_at
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_agent_runtime_configs_realtime_insert
AFTER INSERT ON agent_runtime_configs
FOR EACH ROW EXECUTE FUNCTION emit_agent_runtime_config_realtime_event();

CREATE TRIGGER trg_agent_runtime_configs_realtime_update
AFTER UPDATE OF executor_kind, status, interval_seconds, max_events,
    daily_run_budget, daily_action_budget, system_prompt, model_name, provider_secret_ref
ON agent_runtime_configs
FOR EACH ROW EXECUTE FUNCTION emit_agent_runtime_config_realtime_event();

CREATE FUNCTION emit_agent_runtime_run_realtime_event()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        IF NEW.status = 'running' THEN
            RETURN NEW;
        END IF;
    ELSIF OLD.status IS NOT DISTINCT FROM NEW.status THEN
        RETURN NEW;
    END IF;

    INSERT INTO realtime_events (
        company_id, event_type, aggregate_type, aggregate_id,
        actor_agent_id, payload, created_at
    ) VALUES (
        NEW.company_id,
        CASE NEW.status
            WHEN 'succeeded' THEN 'agent.runtime.run_succeeded'
            WHEN 'failed' THEN 'agent.runtime.run_failed'
            WHEN 'skipped_budget' THEN 'agent.runtime.run_skipped_budget'
            ELSE 'agent.runtime.run_updated'
        END,
        'agent_runtime_run',
        NEW.id,
        NEW.agent_profile_id,
        jsonb_build_object(
            'runtime_run_id', NEW.id,
            'runtime_config_id', NEW.runtime_config_id,
            'agent_profile_id', NEW.agent_profile_id,
            'trigger_type', NEW.trigger_type,
            'status', NEW.status,
            'processed_event_count', NEW.processed_event_count,
            'action_count', NEW.action_count,
            'remaining_pending_count', NEW.remaining_pending_count,
            'error_message', NEW.error_message
        ),
        COALESCE(NEW.finished_at, NEW.started_at)
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_agent_runtime_runs_realtime_insert
AFTER INSERT ON agent_runtime_runs
FOR EACH ROW EXECUTE FUNCTION emit_agent_runtime_run_realtime_event();

CREATE TRIGGER trg_agent_runtime_runs_realtime_update
AFTER UPDATE OF status ON agent_runtime_runs
FOR EACH ROW EXECUTE FUNCTION emit_agent_runtime_run_realtime_event();
