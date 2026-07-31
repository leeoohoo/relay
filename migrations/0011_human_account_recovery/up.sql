CREATE TABLE human_email_verifications (
    human_user_id UUID PRIMARY KEY REFERENCES human_users(id) ON DELETE CASCADE,
    verified_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE human_account_tokens (
    id UUID PRIMARY KEY,
    human_user_id UUID NOT NULL REFERENCES human_users(id) ON DELETE CASCADE,
    purpose VARCHAR(64) NOT NULL,
    token_prefix VARCHAR(32) NOT NULL,
    token_hash VARCHAR(128) NOT NULL UNIQUE,
    expires_at TIMESTAMPTZ NOT NULL,
    used_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_human_account_tokens_user_purpose
    ON human_account_tokens (human_user_id, purpose, created_at DESC);

CREATE INDEX idx_human_account_tokens_expiry
    ON human_account_tokens (expires_at)
    WHERE used_at IS NULL;
