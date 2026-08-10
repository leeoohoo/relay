UPDATE agent_codex_trigger_configs
SET wake_reason = NULL
WHERE wake_reason IS NOT NULL
  AND wake_reason <> 'message';

ALTER TABLE agent_codex_trigger_configs
    DROP CONSTRAINT IF EXISTS agent_codex_trigger_configs_wake_reason_check;

ALTER TABLE agent_codex_trigger_configs
    ADD CONSTRAINT agent_codex_trigger_configs_wake_reason_check
        CHECK (wake_reason IS NULL OR wake_reason IN ('message'));
