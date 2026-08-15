use super::*;

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
