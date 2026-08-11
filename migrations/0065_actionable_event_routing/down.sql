DROP INDEX IF EXISTS idx_agent_event_inbox_active_dedupe;
DROP INDEX IF EXISTS idx_agent_event_inbox_coalesce_pending;
DROP INDEX IF EXISTS idx_agent_event_inbox_actionable_pending;

ALTER TABLE agent_event_inbox
    DROP CONSTRAINT IF EXISTS agent_event_inbox_wake_policy_check,
    DROP CONSTRAINT IF EXISTS agent_event_inbox_event_class_check,
    DROP COLUMN IF EXISTS handled_by_run_id,
    DROP COLUMN IF EXISTS expires_at,
    DROP COLUMN IF EXISTS correlation_id,
    DROP COLUMN IF EXISTS causation_id,
    DROP COLUMN IF EXISTS coalesce_key,
    DROP COLUMN IF EXISTS dedupe_key,
    DROP COLUMN IF EXISTS wake_policy,
    DROP COLUMN IF EXISTS requires_action,
    DROP COLUMN IF EXISTS event_class;
