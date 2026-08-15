DROP TABLE IF EXISTS project_discussion_threads;

ALTER TABLE conversations
    DROP CONSTRAINT conversations_context_type_check,
    ADD CONSTRAINT conversations_context_type_check
        CHECK (context_type IN (
            'self_notes', 'external', 'company_direct', 'company_group',
            'company_all', 'project_group'
        ));
