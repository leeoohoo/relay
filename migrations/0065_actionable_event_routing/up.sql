ALTER TABLE agent_event_inbox
    ADD COLUMN event_class TEXT NOT NULL DEFAULT 'actionable',
    ADD COLUMN requires_action BOOLEAN NOT NULL DEFAULT TRUE,
    ADD COLUMN wake_policy TEXT NOT NULL DEFAULT 'immediate',
    ADD COLUMN dedupe_key TEXT,
    ADD COLUMN coalesce_key TEXT,
    ADD COLUMN causation_id UUID,
    ADD COLUMN correlation_id UUID,
    ADD COLUMN expires_at TIMESTAMPTZ,
    ADD COLUMN handled_by_run_id UUID,
    ADD CONSTRAINT agent_event_inbox_event_class_check
        CHECK (event_class IN ('informational', 'digestible', 'actionable', 'blocking', 'execution_ready')),
    ADD CONSTRAINT agent_event_inbox_wake_policy_check
        CHECK (wake_policy IN ('never', 'deferred', 'immediate'));

CREATE INDEX idx_agent_event_inbox_actionable_pending
    ON agent_event_inbox(agent_profile_id, priority, created_at DESC)
    WHERE status = 'pending' AND requires_action = TRUE;

CREATE INDEX idx_agent_event_inbox_coalesce_pending
    ON agent_event_inbox(agent_profile_id, coalesce_key, created_at DESC)
    WHERE status = 'pending' AND coalesce_key IS NOT NULL;

CREATE UNIQUE INDEX idx_agent_event_inbox_active_dedupe
    ON agent_event_inbox(agent_profile_id, dedupe_key)
    WHERE dedupe_key IS NOT NULL AND status IN ('pending', 'processing');

UPDATE agent_event_inbox
SET event_class = CASE
        WHEN event_type = 'company.project.task_ready' THEN 'execution_ready'
        WHEN event_type LIKE '%approval%' OR event_type LIKE '%failed%' THEN 'blocking'
        WHEN event_type IN ('company.project.status_updated') THEN 'digestible'
        ELSE 'actionable'
    END,
    requires_action = CASE
        WHEN event_type = 'company.project.status_updated' THEN FALSE
        ELSE TRUE
    END,
    wake_policy = CASE
        WHEN event_type = 'company.project.status_updated' THEN 'never'
        ELSE 'immediate'
    END;
