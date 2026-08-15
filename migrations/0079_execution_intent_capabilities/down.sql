ALTER TABLE agent_execution_intents
    DROP CONSTRAINT IF EXISTS agent_execution_intents_required_capabilities_array_check;

ALTER TABLE agent_execution_intents
    DROP COLUMN IF EXISTS required_capabilities;
