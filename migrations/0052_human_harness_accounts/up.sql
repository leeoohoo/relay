CREATE TABLE human_harness_accounts (
    human_user_id UUID PRIMARY KEY REFERENCES human_users(id) ON DELETE CASCADE,
    provider_mode TEXT NOT NULL CHECK (provider_mode IN ('official', 'self_hosted')),
    harness_base_url TEXT NOT NULL,
    harness_uid TEXT NOT NULL,
    harness_email TEXT NOT NULL,
    space_identifier TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending', 'provisioning', 'active', 'failed')),
    attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
    last_error TEXT,
    last_attempt_at TIMESTAMPTZ,
    provisioned_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (harness_base_url, harness_uid),
    UNIQUE (harness_base_url, harness_email),
    UNIQUE (harness_base_url, space_identifier)
);

CREATE INDEX idx_human_harness_accounts_retry
    ON human_harness_accounts (status, last_attempt_at)
    WHERE status IN ('pending', 'failed');
