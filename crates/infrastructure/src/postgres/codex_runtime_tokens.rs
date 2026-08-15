use super::mapping::map_agent_codex_run_token;
use super::*;

pub(super) fn insert_agent_codex_run_token(
    repository: &PostgresPlatformRepository,
    token: AgentCodexRunToken,
) -> AppResult<()> {
    let row = repository.with_client(|client| {
        client.query_one(
            r#"
            WITH agent AS (
                SELECT id, status
                FROM agent_profiles
                WHERE id = $3
            ), inserted AS (
                INSERT INTO agent_codex_run_tokens (
                    id, run_id, agent_profile_id, token_hash,
                    expires_at, revoked_at, created_at
                )
                SELECT $1, $2, agent.id, $4, $5, $6, $7
                FROM agent
                WHERE agent.status <> 'frozen'
                RETURNING id
            )
            SELECT
                (SELECT status FROM agent) AS agent_status,
                EXISTS (SELECT 1 FROM inserted) AS inserted
            "#,
            &[
                &token.id,
                &token.run_id,
                &token.agent_profile_id,
                &token.token_hash,
                &token.expires_at,
                &token.revoked_at,
                &token.created_at,
            ],
        )
    })?;
    let status = row.get::<_, Option<String>>("agent_status");
    if status.is_none() {
        return Err(AppError::NotFound("agent not found".into()));
    }
    if !row.get::<_, bool>("inserted") {
        return Err(AppError::Conflict("agent is frozen by owner".into()));
    }
    Ok(())
}

pub(super) fn find_agent_codex_run_token_by_hash(
    repository: &PostgresPlatformRepository,
    token_hash: &str,
) -> Option<AgentCodexRunToken> {
    repository
        .with_client(|client| {
            client.query_opt(
                r#"
                SELECT id, run_id, agent_profile_id, token_hash,
                       expires_at, revoked_at, created_at
                FROM agent_codex_run_tokens
                WHERE token_hash = $1
                "#,
                &[&token_hash],
            )
        })
        .ok()
        .flatten()
        .map(map_agent_codex_run_token)
}

pub(super) fn revoke_agent_codex_run_tokens(
    repository: &PostgresPlatformRepository,
    run_id: Uuid,
    revoked_at: chrono::DateTime<chrono::Utc>,
) -> AppResult<()> {
    repository.with_client(|client| {
        client.execute(
            r#"
            UPDATE agent_codex_run_tokens
            SET revoked_at = $2
            WHERE run_id = $1 AND revoked_at IS NULL
            "#,
            &[&run_id, &revoked_at],
        )?;
        Ok(())
    })
}

pub(super) fn delete_expired_agent_codex_run_tokens(
    repository: &PostgresPlatformRepository,
    now: chrono::DateTime<chrono::Utc>,
) -> AppResult<usize> {
    repository.with_client(|client| {
        client
            .execute(
                r#"
                DELETE FROM agent_codex_run_tokens
                WHERE expires_at <= $1 OR revoked_at IS NOT NULL
                "#,
                &[&now],
            )
            .map(|count| count as usize)
    })
}
