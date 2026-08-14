ALTER TABLE agent_execution_intents
    DROP CONSTRAINT IF EXISTS agent_execution_intents_action_type_check;

ALTER TABLE agent_execution_intents
    ADD CONSTRAINT agent_execution_intents_action_type_check
    CHECK (action_type IN ('execute', 'replace_session', 'coordinate', 'reply', 'no_action'));
