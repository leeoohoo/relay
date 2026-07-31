CREATE TABLE company_project_rules (
    project_id UUID PRIMARY KEY REFERENCES company_projects(id) ON DELETE CASCADE,
    content TEXT NOT NULL DEFAULT '',
    updated_by_agent_id UUID REFERENCES agent_profiles(id) ON DELETE SET NULL,
    updated_by_human_user_id UUID REFERENCES human_users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (char_length(content) <= 50000)
);

CREATE TABLE company_project_assets (
    id UUID PRIMARY KEY,
    project_id UUID NOT NULL REFERENCES company_projects(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    asset_type TEXT NOT NULL,
    locator TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'active'
        CHECK (status IN ('active', 'missing', 'deprecated', 'unknown')),
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    updated_by_agent_id UUID REFERENCES agent_profiles(id) ON DELETE SET NULL,
    updated_by_human_user_id UUID REFERENCES human_users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (project_id, locator),
    CHECK (char_length(name) BETWEEN 1 AND 200),
    CHECK (char_length(asset_type) BETWEEN 1 AND 80),
    CHECK (char_length(locator) BETWEEN 1 AND 4096),
    CHECK (char_length(description) <= 4000)
);

CREATE INDEX idx_company_project_assets_project_type
    ON company_project_assets(project_id, asset_type, name);

CREATE TABLE company_project_asset_refresh_configs (
    project_id UUID PRIMARY KEY REFERENCES company_projects(id) ON DELETE CASCADE,
    maintainer_agent_id UUID NOT NULL REFERENCES agent_profiles(id) ON DELETE RESTRICT,
    interval_minutes INTEGER NOT NULL DEFAULT 1440
        CHECK (interval_minutes BETWEEN 5 AND 10080),
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    next_refresh_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_requested_at TIMESTAMPTZ,
    last_completed_at TIMESTAMPTZ,
    created_by_human_user_id UUID NOT NULL REFERENCES human_users(id) ON DELETE RESTRICT,
    updated_by_human_user_id UUID REFERENCES human_users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_company_project_asset_refresh_due
    ON company_project_asset_refresh_configs(maintainer_agent_id, next_refresh_at)
    WHERE enabled = TRUE;

ALTER TABLE agent_codex_trigger_runs
    DROP CONSTRAINT agent_codex_trigger_runs_trigger_type_check,
    ADD CONSTRAINT agent_codex_trigger_runs_trigger_type_check
        CHECK (trigger_type IN ('scheduled', 'manual', 'message', 'task', 'asset_refresh'));
