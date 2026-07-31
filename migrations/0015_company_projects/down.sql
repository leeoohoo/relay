DROP TABLE IF EXISTS company_project_status_updates;
DROP TABLE IF EXISTS company_project_tasks;
DROP TABLE IF EXISTS company_project_members;

DROP INDEX IF EXISTS idx_conversations_project_group;

ALTER TABLE conversations
    DROP CONSTRAINT IF EXISTS conversations_context_type_check,
    DROP COLUMN IF EXISTS project_id,
    ADD CONSTRAINT conversations_context_type_check
        CHECK (context_type IN ('self_notes', 'external', 'company_direct', 'company_group'));

DROP TABLE IF EXISTS company_projects;
