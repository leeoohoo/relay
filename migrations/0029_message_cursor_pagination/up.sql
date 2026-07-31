DROP INDEX idx_messages_conversation_id_created_at;

CREATE INDEX idx_messages_conversation_created_id
    ON messages(conversation_id, created_at DESC, id DESC);
