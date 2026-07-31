CREATE TABLE company_codex_runner_profiles (
    id UUID PRIMARY KEY,
    company_id UUID NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    interval_seconds INTEGER NOT NULL DEFAULT 30
        CHECK (interval_seconds BETWEEN 10 AND 86400),
    codex_profile TEXT NOT NULL DEFAULT 'default',
    model TEXT,
    sandbox_mode TEXT NOT NULL DEFAULT 'workspace_write'
        CHECK (sandbox_mode IN ('read_only', 'workspace_write')),
    max_run_seconds INTEGER NOT NULL DEFAULT 1800
        CHECK (max_run_seconds BETWEEN 60 AND 7200),
    is_default BOOLEAN NOT NULL DEFAULT FALSE,
    created_by_human_user_id UUID NOT NULL REFERENCES human_users(id) ON DELETE RESTRICT,
    updated_by_human_user_id UUID REFERENCES human_users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (char_length(name) BETWEEN 1 AND 80),
    CHECK (char_length(codex_profile) BETWEEN 1 AND 64),
    CHECK (model IS NULL OR char_length(model) BETWEEN 1 AND 128),
    UNIQUE (company_id, name)
);

CREATE INDEX idx_company_codex_runner_profiles_company
    ON company_codex_runner_profiles(company_id, created_at);

CREATE UNIQUE INDEX idx_company_codex_runner_profiles_one_default
    ON company_codex_runner_profiles(company_id)
    WHERE is_default;

CREATE TABLE agent_codex_runner_profile_assignments (
    agent_profile_id UUID PRIMARY KEY REFERENCES agent_profiles(id) ON DELETE CASCADE,
    runner_profile_id UUID NOT NULL REFERENCES company_codex_runner_profiles(id) ON DELETE RESTRICT,
    assigned_by_human_user_id UUID NOT NULL REFERENCES human_users(id) ON DELETE RESTRICT,
    assigned_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_agent_codex_runner_profile_assignments_profile
    ON agent_codex_runner_profile_assignments(runner_profile_id, agent_profile_id);

ALTER TABLE agent_codex_trigger_configs
    ADD COLUMN model TEXT;

ALTER TABLE agent_codex_trigger_configs
    ADD CONSTRAINT chk_agent_codex_trigger_model
    CHECK (model IS NULL OR char_length(model) BETWEEN 1 AND 128);
