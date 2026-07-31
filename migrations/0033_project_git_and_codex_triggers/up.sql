CREATE TABLE company_project_git_configs (
    project_id UUID PRIMARY KEY REFERENCES company_projects(id) ON DELETE CASCADE,
    remote_url TEXT NOT NULL,
    default_branch TEXT NOT NULL DEFAULT 'main',
    git_host TEXT NOT NULL,
    host_local_path TEXT NOT NULL,
    auth_profile TEXT,
    allow_agent_push BOOLEAN NOT NULL DEFAULT FALSE,
    branch_prefix TEXT NOT NULL DEFAULT 'relay/',
    created_by_agent_id UUID REFERENCES agent_profiles(id) ON DELETE SET NULL,
    created_by_human_user_id UUID REFERENCES human_users(id) ON DELETE SET NULL,
    updated_by_agent_id UUID REFERENCES agent_profiles(id) ON DELETE SET NULL,
    updated_by_human_user_id UUID REFERENCES human_users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (char_length(remote_url) BETWEEN 1 AND 2048),
    CHECK (char_length(default_branch) BETWEEN 1 AND 255),
    CHECK (char_length(git_host) BETWEEN 1 AND 255),
    CHECK (char_length(host_local_path) BETWEEN 1 AND 4096),
    CHECK (auth_profile IS NULL OR char_length(auth_profile) BETWEEN 1 AND 64),
    CHECK (char_length(branch_prefix) BETWEEN 1 AND 80)
);

CREATE INDEX idx_company_project_git_configs_host
    ON company_project_git_configs(git_host, project_id);

CREATE TABLE agent_codex_trigger_configs (
    id UUID PRIMARY KEY,
    company_id UUID NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    agent_profile_id UUID NOT NULL UNIQUE REFERENCES agent_profiles(id) ON DELETE CASCADE,
    status TEXT NOT NULL DEFAULT 'active'
        CHECK (status IN ('active', 'paused', 'error')),
    interval_seconds INTEGER NOT NULL DEFAULT 30
        CHECK (interval_seconds BETWEEN 10 AND 86400),
    codex_profile TEXT NOT NULL,
    sandbox_mode TEXT NOT NULL DEFAULT 'read_only'
        CHECK (sandbox_mode IN ('read_only', 'workspace_write')),
    max_run_seconds INTEGER NOT NULL DEFAULT 1800
        CHECK (max_run_seconds BETWEEN 60 AND 7200),
    next_run_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    lease_owner TEXT,
    lease_expires_at TIMESTAMPTZ,
    manual_run_requested_at TIMESTAMPTZ,
    last_run_at TIMESTAMPTZ,
    last_success_at TIMESTAMPTZ,
    last_error TEXT,
    consecutive_failure_count INTEGER NOT NULL DEFAULT 0,
    created_by_human_user_id UUID NOT NULL REFERENCES human_users(id) ON DELETE RESTRICT,
    updated_by_human_user_id UUID REFERENCES human_users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_agent_codex_trigger_configs_due
    ON agent_codex_trigger_configs(status, next_run_at)
    WHERE status = 'active';

CREATE TABLE agent_codex_trigger_runs (
    id UUID PRIMARY KEY,
    trigger_config_id UUID NOT NULL REFERENCES agent_codex_trigger_configs(id) ON DELETE CASCADE,
    agent_profile_id UUID NOT NULL REFERENCES agent_profiles(id) ON DELETE CASCADE,
    project_id UUID REFERENCES company_projects(id) ON DELETE SET NULL,
    trigger_type TEXT NOT NULL
        CHECK (trigger_type IN ('scheduled', 'manual', 'message', 'task')),
    status TEXT NOT NULL
        CHECK (status IN ('running', 'succeeded', 'failed', 'timed_out', 'cancelled', 'lease_lost')),
    codex_thread_id TEXT,
    codex_version TEXT,
    exit_code INTEGER,
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    finished_at TIMESTAMPTZ,
    final_message_summary TEXT,
    error_message TEXT
);

CREATE INDEX idx_agent_codex_trigger_runs_agent_started
    ON agent_codex_trigger_runs(agent_profile_id, started_at DESC);

CREATE TABLE agent_codex_sessions (
    agent_profile_id UUID PRIMARY KEY REFERENCES agent_profiles(id) ON DELETE CASCADE,
    current_project_id UUID REFERENCES company_projects(id) ON DELETE SET NULL,
    codex_thread_id TEXT NOT NULL,
    worktree_key TEXT NOT NULL,
    last_used_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE agent_codex_run_tokens (
    id UUID PRIMARY KEY,
    run_id UUID NOT NULL REFERENCES agent_codex_trigger_runs(id) ON DELETE CASCADE,
    agent_profile_id UUID NOT NULL REFERENCES agent_profiles(id) ON DELETE CASCADE,
    token_hash TEXT NOT NULL UNIQUE,
    expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_agent_codex_run_tokens_active
    ON agent_codex_run_tokens(token_hash, expires_at)
    WHERE revoked_at IS NULL;
