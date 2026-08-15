ALTER TABLE agent_execution_intents
    ADD COLUMN required_capabilities JSONB NOT NULL DEFAULT '[]'::jsonb;

ALTER TABLE agent_execution_intents
    ADD CONSTRAINT agent_execution_intents_required_capabilities_array_check
    CHECK (jsonb_typeof(required_capabilities) = 'array');
