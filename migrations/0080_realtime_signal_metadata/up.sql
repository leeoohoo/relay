CREATE OR REPLACE FUNCTION notify_realtime_event_insert()
RETURNS TRIGGER AS $$
BEGIN
    PERFORM pg_notify(
        'ai_chat_realtime_events',
        json_build_object(
            'sequence_id', NEW.sequence_id,
            'company_id', NEW.company_id,
            'event_type', NEW.event_type,
            'aggregate_type', NEW.aggregate_type,
            'aggregate_id', NEW.aggregate_id
        )::text
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
