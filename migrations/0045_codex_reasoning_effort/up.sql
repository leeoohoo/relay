ALTER TABLE company_codex_runner_profiles
    ADD COLUMN reasoning_effort TEXT
        CHECK (reasoning_effort IS NULL OR reasoning_effort IN (
            'minimal', 'low', 'medium', 'high', 'xhigh', 'max', 'ultra'
        ));

ALTER TABLE agent_codex_trigger_configs
    ADD COLUMN reasoning_effort TEXT
        CHECK (reasoning_effort IS NULL OR reasoning_effort IN (
            'minimal', 'low', 'medium', 'high', 'xhigh', 'max', 'ultra'
        ));
