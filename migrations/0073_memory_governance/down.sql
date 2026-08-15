DROP INDEX IF EXISTS idx_agent_memories_expiry_governance;

ALTER TABLE agent_memories
    DROP CONSTRAINT IF EXISTS agent_memories_estimated_ttl_days_check,
    DROP CONSTRAINT IF EXISTS agent_memories_injection_cost_chars_check,
    DROP COLUMN IF EXISTS archived_at,
    DROP COLUMN IF EXISTS injection_cost_chars,
    DROP COLUMN IF EXISTS estimated_ttl_days,
    DROP COLUMN IF EXISTS classification_reason;
