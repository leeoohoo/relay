ALTER TABLE conversations
    ADD COLUMN created_by_human_user_id UUID REFERENCES human_users(id) ON DELETE SET NULL;

ALTER TABLE messages
    DROP CONSTRAINT IF EXISTS messages_conversation_id_sender_agent_id_client_message_id_key,
    ALTER COLUMN sender_agent_id DROP NOT NULL,
    ADD COLUMN sender_human_user_id UUID REFERENCES human_users(id) ON DELETE CASCADE,
    ADD CONSTRAINT messages_exactly_one_sender_check CHECK (
        (sender_agent_id IS NOT NULL AND sender_human_user_id IS NULL)
        OR (sender_agent_id IS NULL AND sender_human_user_id IS NOT NULL)
    );

CREATE UNIQUE INDEX idx_messages_agent_client_message
    ON messages(conversation_id, sender_agent_id, client_message_id)
    WHERE sender_agent_id IS NOT NULL AND client_message_id IS NOT NULL;

CREATE UNIQUE INDEX idx_messages_human_client_message
    ON messages(conversation_id, sender_human_user_id, client_message_id)
    WHERE sender_human_user_id IS NOT NULL AND client_message_id IS NOT NULL;

CREATE TABLE company_human_direct_conversations (
    company_id UUID NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    human_user_id UUID NOT NULL REFERENCES human_users(id) ON DELETE CASCADE,
    target_agent_id UUID NOT NULL REFERENCES agent_profiles(id) ON DELETE CASCADE,
    conversation_id UUID NOT NULL UNIQUE REFERENCES conversations(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (company_id, human_user_id, target_agent_id)
);

DROP TRIGGER IF EXISTS trg_messages_realtime_event ON messages;
DROP FUNCTION IF EXISTS emit_message_realtime_event();

CREATE FUNCTION emit_message_realtime_event()
RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO realtime_events (
        company_id, event_type, aggregate_type, aggregate_id,
        actor_agent_id, actor_human_user_id, payload, created_at
    )
    SELECT
        c.company_id,
        'message.created',
        'conversation',
        NEW.conversation_id,
        NEW.sender_agent_id,
        NEW.sender_human_user_id,
        jsonb_build_object(
            'message_id', NEW.id,
            'conversation_id', NEW.conversation_id,
            'context_type', c.context_type,
            'project_id', c.project_id,
            'sender_agent_id', NEW.sender_agent_id,
            'sender_human_user_id', NEW.sender_human_user_id,
            'content_preview', left(NEW.content_text, 240)
        ),
        NEW.created_at
    FROM conversations c
    WHERE c.id = NEW.conversation_id
      AND c.company_id IS NOT NULL;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_messages_realtime_event
AFTER INSERT ON messages
FOR EACH ROW EXECUTE FUNCTION emit_message_realtime_event();
