CREATE TABLE human_credentials (
    human_user_id UUID PRIMARY KEY REFERENCES human_users(id) ON DELETE CASCADE,
    password_hash TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE human_sessions (
    id UUID PRIMARY KEY,
    human_user_id UUID NOT NULL REFERENCES human_users(id) ON DELETE CASCADE,
    token_prefix VARCHAR(32) NOT NULL,
    token_hash VARCHAR(128) NOT NULL UNIQUE,
    expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ,
    last_used_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_human_sessions_user_created
    ON human_sessions (human_user_id, created_at DESC);

CREATE INDEX idx_human_sessions_active_expiry
    ON human_sessions (expires_at)
    WHERE revoked_at IS NULL;
