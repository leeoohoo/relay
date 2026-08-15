use super::*;

pub(super) fn next_eligible_agent_codex_trigger_at(
    repository: &PostgresPlatformRepository,
    now: chrono::DateTime<chrono::Utc>,
) -> AppResult<Option<chrono::DateTime<chrono::Utc>>> {
    repository
        .with_client(|client| {
            client.query_one(
                r#"
            SELECT MIN(
                CASE
                    WHEN config.lease_expires_at IS NOT NULL
                     AND config.lease_expires_at > $1
                        THEN GREATEST(config.next_run_at, config.lease_expires_at)
                    ELSE config.next_run_at
                END
            ) AS next_eligible_at
            FROM agent_codex_trigger_configs config
            WHERE config.status = 'active'
              AND EXISTS (
                  SELECT 1
                  FROM company_human_members human_member
                  INNER JOIN human_sessions human_session
                      ON human_session.human_user_id = human_member.human_user_id
                  WHERE human_member.company_id = config.company_id
                    AND human_member.status = 'active'
                    AND human_session.revoked_at IS NULL
                    AND human_session.expires_at > $1
                    AND COALESCE(human_session.last_used_at, human_session.created_at)
                        > $1 - INTERVAL '60 seconds'
              )
              AND NOT EXISTS (
                  SELECT 1
                  FROM agent_codex_trigger_runs active_run
                  WHERE active_run.agent_profile_id = config.agent_profile_id
                    AND active_run.status = 'running'
                    AND active_run.started_at
                        + make_interval(secs => config.max_run_seconds + 60) > $1
              )
            "#,
                &[&now],
            )
        })
        .map(|row| row.get("next_eligible_at"))
}
