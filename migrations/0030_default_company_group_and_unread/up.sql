ALTER TABLE conversations
    DROP CONSTRAINT conversations_context_type_check,
    ADD CONSTRAINT conversations_context_type_check
        CHECK (context_type IN (
            'self_notes', 'external', 'company_direct', 'company_group',
            'company_all', 'project_group'
        ));

CREATE UNIQUE INDEX IF NOT EXISTS idx_conversations_company_all
    ON conversations(company_id)
    WHERE context_type = 'company_all' AND company_id IS NOT NULL;

INSERT INTO conversations (
    id, conversation_type, title, created_by_agent_id, status,
    company_id, project_id, context_type, visibility,
    last_message_at, created_at, updated_at
)
SELECT
    gen_random_uuid(),
    'group',
    company.name || ' 全员群',
    NULL,
    'active',
    company.id,
    NULL,
    'company_all',
    'members',
    NULL,
    company.created_at,
    company.updated_at
FROM companies company
WHERE NOT EXISTS (
    SELECT 1
    FROM conversations conversation
    WHERE conversation.company_id = company.id
      AND conversation.context_type = 'company_all'
);

INSERT INTO conversation_members (
    id, conversation_id, agent_profile_id, member_role, joined_at
)
SELECT
    gen_random_uuid(),
    conversation.id,
    membership.agent_profile_id,
    CASE WHEN membership.role_key = 'company_manager' THEN 'admin' ELSE 'member' END,
    membership.joined_at
FROM conversations conversation
INNER JOIN company_agent_memberships membership
    ON membership.company_id = conversation.company_id
WHERE conversation.context_type = 'company_all'
  AND membership.employment_status = 'active'
ON CONFLICT (conversation_id, agent_profile_id) DO NOTHING;
