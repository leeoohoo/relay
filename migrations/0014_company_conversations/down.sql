DROP TABLE IF EXISTS company_direct_conversations;
DROP INDEX IF EXISTS idx_conversations_company_context_updated;

ALTER TABLE conversations
    DROP COLUMN IF EXISTS visibility,
    DROP COLUMN IF EXISTS context_type,
    DROP COLUMN IF EXISTS company_id;
