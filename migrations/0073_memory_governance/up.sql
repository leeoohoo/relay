ALTER TABLE agent_memories
    ADD COLUMN IF NOT EXISTS classification_reason TEXT NOT NULL DEFAULT 'legacy memory; classification pending',
    ADD COLUMN IF NOT EXISTS estimated_ttl_days INTEGER NULL,
    ADD COLUMN IF NOT EXISTS injection_cost_chars INTEGER NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS archived_at TIMESTAMPTZ NULL;

ALTER TABLE agent_memories
    DROP CONSTRAINT IF EXISTS agent_memories_estimated_ttl_days_check,
    DROP CONSTRAINT IF EXISTS agent_memories_injection_cost_chars_check;

ALTER TABLE agent_memories
    ADD CONSTRAINT agent_memories_estimated_ttl_days_check
        CHECK (estimated_ttl_days IS NULL OR estimated_ttl_days BETWEEN 1 AND 3650),
    ADD CONSTRAINT agent_memories_injection_cost_chars_check
        CHECK (injection_cost_chars >= 0);

UPDATE agent_memories
SET injection_cost_chars = char_length(title) + char_length(topic_key) + char_length(summary)
        + char_length(when_to_use) + 96,
    estimated_ttl_days = CASE WHEN memory_tier = 'short_term' THEN 30 ELSE NULL END,
    classification_reason = CASE
        WHEN memory_tier = 'short_term' THEN 'legacy short-term memory; retained as on-demand context'
        ELSE 'legacy long-term memory; retained pending the next verified update'
    END
WHERE injection_cost_chars = 0;

CREATE INDEX IF NOT EXISTS idx_agent_memories_expiry_governance
    ON agent_memories(company_id, status, expires_at)
    WHERE expires_at IS NOT NULL AND status = 'active';
