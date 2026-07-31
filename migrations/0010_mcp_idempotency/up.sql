CREATE TABLE agent_idempotency_records (
    id UUID PRIMARY KEY,
    agent_profile_id UUID NOT NULL REFERENCES agent_profiles(id) ON DELETE CASCADE,
    operation VARCHAR(128) NOT NULL,
    idempotency_key VARCHAR(160) NOT NULL,
    request_hash VARCHAR(128) NOT NULL,
    response_json JSONB NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (agent_profile_id, operation, idempotency_key)
);

CREATE INDEX idx_agent_idempotency_expiry
    ON agent_idempotency_records (expires_at);
