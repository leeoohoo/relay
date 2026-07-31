ALTER TABLE conversations
    ADD COLUMN company_id UUID REFERENCES companies(id) ON DELETE CASCADE,
    ADD COLUMN context_type TEXT NOT NULL DEFAULT 'external'
        CHECK (context_type IN ('self_notes', 'external', 'company_direct', 'company_group')),
    ADD COLUMN visibility TEXT NOT NULL DEFAULT 'members'
        CHECK (visibility IN ('private', 'members'));

UPDATE conversations c
SET company_id = cam.company_id,
    context_type = 'self_notes',
    visibility = 'private'
FROM conversation_members cm
INNER JOIN company_agent_memberships cam
    ON cam.agent_profile_id = cm.agent_profile_id
WHERE cm.conversation_id = c.id
  AND c.conversation_type = 'direct'
  AND c.title = 'Self Notes'
  AND NOT EXISTS (
      SELECT 1
      FROM conversation_members another
      WHERE another.conversation_id = c.id
        AND another.agent_profile_id <> cm.agent_profile_id
        AND another.left_at IS NULL
  );

CREATE INDEX idx_conversations_company_context_updated
    ON conversations(company_id, context_type, updated_at DESC)
    WHERE company_id IS NOT NULL;

CREATE TABLE company_direct_conversations (
    company_id UUID NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    left_agent_id UUID NOT NULL REFERENCES agent_profiles(id) ON DELETE RESTRICT,
    right_agent_id UUID NOT NULL REFERENCES agent_profiles(id) ON DELETE RESTRICT,
    conversation_id UUID NOT NULL UNIQUE REFERENCES conversations(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (company_id, left_agent_id, right_agent_id),
    CHECK (left_agent_id <> right_agent_id),
    CHECK (left_agent_id::text < right_agent_id::text)
);

CREATE INDEX idx_company_direct_conversations_agents
    ON company_direct_conversations(company_id, left_agent_id, right_agent_id);
