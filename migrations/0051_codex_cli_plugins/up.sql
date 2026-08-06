CREATE TABLE codex_plugin_catalog_snapshots (
    runner_id TEXT PRIMARY KEY,
    hostname TEXT NOT NULL,
    codex_version TEXT,
    fingerprint TEXT NOT NULL,
    installed JSONB NOT NULL DEFAULT '[]'::JSONB,
    available JSONB NOT NULL DEFAULT '[]'::JSONB,
    marketplaces JSONB NOT NULL DEFAULT '[]'::JSONB,
    discovered_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE codex_plugin_operations (
    id UUID PRIMARY KEY,
    company_id UUID NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    target_runner_id TEXT NOT NULL,
    operation TEXT NOT NULL CHECK (operation IN ('install', 'remove', 'refresh')),
    plugin_id TEXT,
    status TEXT NOT NULL CHECK (status IN ('queued', 'running', 'succeeded', 'failed')),
    requested_by_human_user_id UUID NOT NULL REFERENCES human_users(id),
    lease_owner TEXT,
    lease_expires_at TIMESTAMPTZ,
    attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
    error_message TEXT,
    result JSONB NOT NULL DEFAULT '{}'::JSONB,
    requested_at TIMESTAMPTZ NOT NULL,
    started_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ NOT NULL,
    CHECK (
        (operation = 'refresh' AND plugin_id IS NULL)
        OR (operation IN ('install', 'remove') AND plugin_id IS NOT NULL)
    )
);

CREATE INDEX idx_codex_plugin_operations_runner_queue
    ON codex_plugin_operations (target_runner_id, status, requested_at);
CREATE INDEX idx_codex_plugin_operations_company_recent
    ON codex_plugin_operations (company_id, requested_at DESC);

CREATE UNIQUE INDEX uq_codex_plugin_operations_active_action
    ON codex_plugin_operations (target_runner_id, operation, COALESCE(plugin_id, ''))
    WHERE status IN ('queued', 'running');

CREATE FUNCTION emit_codex_plugin_catalog_realtime_event()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'UPDATE' AND NEW.fingerprint = OLD.fingerprint THEN
        RETURN NEW;
    END IF;

    INSERT INTO realtime_events (
        company_id, event_type, aggregate_type, aggregate_id,
        payload, created_at
    )
    SELECT
        company.id,
        'codex.plugin.catalog.updated',
        'codex_plugin_catalog',
        NULL,
        jsonb_build_object(
            'runner_id', NEW.runner_id,
            'hostname', NEW.hostname,
            'codex_version', NEW.codex_version,
            'fingerprint', NEW.fingerprint,
            'discovered_at', NEW.discovered_at
        ),
        NEW.updated_at
    FROM companies company
    WHERE company.status = 'active';
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_codex_plugin_catalog_realtime
AFTER INSERT OR UPDATE OF fingerprint ON codex_plugin_catalog_snapshots
FOR EACH ROW EXECUTE FUNCTION emit_codex_plugin_catalog_realtime_event();

CREATE FUNCTION emit_codex_plugin_operation_realtime_event()
RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO realtime_events (
        company_id, event_type, aggregate_type, aggregate_id,
        actor_human_user_id, payload, created_at
    ) VALUES (
        NEW.company_id,
        'codex.plugin.operation.updated',
        'codex_plugin_operation',
        NEW.id,
        NEW.requested_by_human_user_id,
        jsonb_build_object(
            'operation_id', NEW.id,
            'runner_id', NEW.target_runner_id,
            'operation', NEW.operation,
            'plugin_id', NEW.plugin_id,
            'status', NEW.status,
            'attempt_count', NEW.attempt_count,
            'error_message', NEW.error_message
        ),
        NEW.updated_at
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_codex_plugin_operations_realtime
AFTER INSERT OR UPDATE OF status, attempt_count, error_message, result
ON codex_plugin_operations
FOR EACH ROW EXECUTE FUNCTION emit_codex_plugin_operation_realtime_event();
