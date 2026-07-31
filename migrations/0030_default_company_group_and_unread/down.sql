-- Remove the default company-wide groups before restoring the older context constraint.
DELETE FROM conversations
WHERE context_type = 'company_all';

DROP INDEX IF EXISTS idx_conversations_company_all;

ALTER TABLE conversations
    DROP CONSTRAINT IF EXISTS conversations_context_type_check,
    ADD CONSTRAINT conversations_context_type_check
        CHECK (context_type IN (
            'self_notes', 'external', 'company_direct', 'company_group', 'project_group'
        ));
