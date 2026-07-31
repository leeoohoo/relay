DROP TRIGGER IF EXISTS trg_messages_realtime_event ON messages;
DROP FUNCTION IF EXISTS emit_message_realtime_event();

DELETE FROM messages WHERE sender_agent_id IS NULL;

DROP TABLE IF EXISTS company_human_direct_conversations;

DROP INDEX IF EXISTS idx_messages_human_client_message;
DROP INDEX IF EXISTS idx_messages_agent_client_message;

ALTER TABLE messages
    DROP CONSTRAINT IF EXISTS messages_exactly_one_sender_check,
    DROP COLUMN IF EXISTS sender_human_user_id,
    ALTER COLUMN sender_agent_id SET NOT NULL,
    ADD CONSTRAINT messages_conversation_id_sender_agent_id_client_message_id_key
        UNIQUE (conversation_id, sender_agent_id, client_message_id);

ALTER TABLE conversations
    DROP COLUMN IF EXISTS created_by_human_user_id;

CREATE FUNCTION emit_message_realtime_event()
RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO realtime_events (
        company_id, event_type, aggregate_type, aggregate_id,
        actor_agent_id, payload, created_at
    )
    SELECT
        c.company_id,
        'message.created',
        'conversation',
        NEW.conversation_id,
        NEW.sender_agent_id,
        jsonb_build_object(
            'message_id', NEW.id,
            'conversation_id', NEW.conversation_id,
            'context_type', c.context_type,
            'project_id', c.project_id,
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
