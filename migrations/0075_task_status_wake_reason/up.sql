ALTER TABLE agent_codex_trigger_configs
    DROP CONSTRAINT IF EXISTS agent_codex_trigger_configs_wake_reason_check;

ALTER TABLE agent_codex_trigger_configs
    ADD CONSTRAINT agent_codex_trigger_configs_wake_reason_check
        CHECK (
            wake_reason IS NULL
            OR wake_reason IN (
                'message',
                'task_ready',
                'task_status_changed',
                'project_resumed',
                'intent_recovery'
            )
        );
