UPDATE agent_keys
SET key_name = CASE
    WHEN key_name LIKE 'plaintext:%' THEN 'legacy'
    ELSE key_name
END;

UPDATE agent_keys
SET expires_at = created_at + INTERVAL '180 days'
WHERE expires_at IS NULL
  AND revoked_at IS NULL;

CREATE INDEX idx_agent_keys_active_expiry
    ON agent_keys (expires_at)
    WHERE revoked_at IS NULL;
