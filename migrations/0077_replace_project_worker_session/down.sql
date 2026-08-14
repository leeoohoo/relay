UPDATE agent_execution_intents
SET action_type = 'execute'
WHERE action_type = 'replace_session';

ALTER TABLE agent_execution_intents
    DROP CONSTRAINT IF EXISTS agent_execution_intents_action_type_check;

ALTER TABLE agent_execution_intents
    ADD CONSTRAINT agent_execution_intents_action_type_check
    CHECK (action_type IN ('execute', 'coordinate', 'reply', 'no_action'));
