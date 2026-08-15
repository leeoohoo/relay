CREATE OR REPLACE FUNCTION notify_realtime_event_insert()
RETURNS TRIGGER AS $$
BEGIN
    PERFORM pg_notify(
        'ai_chat_realtime_events',
        json_build_object(
            'sequence_id', NEW.sequence_id,
            'company_id', NEW.company_id
        )::text
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
