use super::mapping::*;
use super::*;

impl AuthPlatformRepository for PostgresPlatformRepository {
    fn health_check(&self) -> AppResult<()> {
        self.with_client(|client| client.simple_query("SELECT 1").map(|_| ()))
    }

    fn find_human_user_by_email(&self, email: &str) -> Option<HumanUser> {
        self.find_human_user_by_email_result(email).ok().flatten()
    }

    fn find_human_user_by_email_result(&self, email: &str) -> AppResult<Option<HumanUser>> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT id, email, display_name, created_at
                FROM human_users
                WHERE lower(email) = lower($1)
                "#,
                &[&email],
            )
        })
        .map(|row| row.map(map_human_user))
    }

    fn get_human_user(&self, user_id: Uuid) -> Option<HumanUser> {
        self.get_human_user_result(user_id).ok().flatten()
    }

    fn get_human_user_result(&self, user_id: Uuid) -> AppResult<Option<HumanUser>> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT id, email, display_name, created_at
                FROM human_users
                WHERE id = $1
                "#,
                &[&user_id],
            )
        })
        .map(|row| row.map(map_human_user))
    }

    fn list_human_users(&self) -> Vec<HumanUser> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT id, email, display_name, created_at
                FROM human_users
                ORDER BY created_at DESC
                "#,
                &[],
            )
        })
        .unwrap_or_default()
        .into_iter()
        .map(map_human_user)
        .collect()
    }

    fn insert_human_user(&self, user: HumanUser) -> AppResult<HumanUser> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO human_users (id, email, display_name, status, created_at, updated_at)
                VALUES ($1, $2, $3, 'active', $4, $4)
                "#,
                &[&user.id, &user.email, &user.display_name, &user.created_at],
            )?;
            Ok(user)
        })
    }

    fn human_user_exists(&self, user_id: Uuid) -> bool {
        self.human_user_exists_result(user_id).unwrap_or(false)
    }

    fn human_user_exists_result(&self, user_id: Uuid) -> AppResult<bool> {
        self.with_client(|client| {
            client
                .query_one(
                    "SELECT COUNT(1) FROM human_users WHERE id = $1",
                    &[&user_id],
                )
                .map(|row| row.get::<_, i64>(0) > 0)
        })
    }

    fn get_human_credential(&self, user_id: Uuid) -> Option<HumanCredential> {
        self.get_human_credential_result(user_id).ok().flatten()
    }

    fn get_human_credential_result(&self, user_id: Uuid) -> AppResult<Option<HumanCredential>> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT human_user_id, password_hash, created_at, updated_at
                FROM human_credentials
                WHERE human_user_id = $1
                "#,
                &[&user_id],
            )
        })
        .map(|row| row.map(map_human_credential))
    }

    fn insert_human_auth_bundle(
        &self,
        user: HumanUser,
        credential: HumanCredential,
    ) -> AppResult<HumanUser> {
        self.with_client(|client| {
            let mut tx = client.transaction()?;
            tx.execute(
                r#"
                INSERT INTO human_users (id, email, display_name, status, created_at, updated_at)
                VALUES ($1, $2, $3, 'active', $4, $4)
                "#,
                &[&user.id, &user.email, &user.display_name, &user.created_at],
            )?;
            tx.execute(
                r#"
                INSERT INTO human_credentials (
                    human_user_id, password_hash, created_at, updated_at
                )
                VALUES ($1, $2, $3, $4)
                "#,
                &[
                    &credential.human_user_id,
                    &credential.password_hash,
                    &credential.created_at,
                    &credential.updated_at,
                ],
            )?;
            tx.commit()?;
            Ok(user)
        })
    }

    fn get_human_harness_account(&self, human_user_id: Uuid) -> Option<HumanHarnessAccount> {
        self.get_human_harness_account_result(human_user_id)
            .ok()
            .flatten()
    }

    fn get_human_harness_account_result(
        &self,
        human_user_id: Uuid,
    ) -> AppResult<Option<HumanHarnessAccount>> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT human_user_id, provider_mode, harness_base_url, harness_uid,
                       harness_email, space_identifier, status, attempt_count,
                       last_error, last_attempt_at, provisioned_at, created_at, updated_at
                FROM human_harness_accounts
                WHERE human_user_id = $1
                "#,
                &[&human_user_id],
            )
        })
        .map(|row| row.map(map_human_harness_account))
    }

    fn upsert_human_harness_account(&self, account: HumanHarnessAccount) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO human_harness_accounts (
                    human_user_id, provider_mode, harness_base_url, harness_uid,
                    harness_email, space_identifier, status, attempt_count,
                    last_error, last_attempt_at, provisioned_at, created_at, updated_at
                ) VALUES (
                    $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13
                )
                ON CONFLICT (human_user_id) DO UPDATE SET
                    provider_mode = EXCLUDED.provider_mode,
                    harness_base_url = EXCLUDED.harness_base_url,
                    harness_uid = EXCLUDED.harness_uid,
                    harness_email = EXCLUDED.harness_email,
                    space_identifier = EXCLUDED.space_identifier,
                    status = EXCLUDED.status,
                    attempt_count = EXCLUDED.attempt_count,
                    last_error = EXCLUDED.last_error,
                    last_attempt_at = EXCLUDED.last_attempt_at,
                    provisioned_at = EXCLUDED.provisioned_at,
                    updated_at = EXCLUDED.updated_at
                "#,
                &[
                    &account.human_user_id,
                    &account.provider_mode,
                    &account.harness_base_url,
                    &account.harness_uid,
                    &account.harness_email,
                    &account.space_identifier,
                    &account.status,
                    &account.attempt_count,
                    &account.last_error,
                    &account.last_attempt_at,
                    &account.provisioned_at,
                    &account.created_at,
                    &account.updated_at,
                ],
            )?;
            Ok(())
        })
    }

    fn insert_human_session(&self, session: HumanSession) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO human_sessions (
                    id, human_user_id, token_prefix, token_hash, expires_at,
                    revoked_at, last_used_at, created_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                "#,
                &[
                    &session.id,
                    &session.human_user_id,
                    &session.token_prefix,
                    &session.token_hash,
                    &session.expires_at,
                    &session.revoked_at,
                    &session.last_used_at,
                    &session.created_at,
                ],
            )?;
            Ok(())
        })
    }

    fn find_human_session_by_token_hash(&self, token_hash: &str) -> Option<HumanSession> {
        self.find_human_session_by_token_hash_result(token_hash)
            .ok()
            .flatten()
    }

    fn find_human_session_by_token_hash_result(
        &self,
        token_hash: &str,
    ) -> AppResult<Option<HumanSession>> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT id, human_user_id, token_prefix, token_hash, expires_at,
                       revoked_at, last_used_at, created_at
                FROM human_sessions
                WHERE token_hash = $1
                "#,
                &[&token_hash],
            )
        })
        .map(|row| row.map(map_human_session))
    }

    fn touch_human_session(
        &self,
        session_id: Uuid,
        used_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                "UPDATE human_sessions SET last_used_at = $2 WHERE id = $1",
                &[&session_id, &used_at],
            )?;
            Ok(())
        })
    }

    fn revoke_human_session(
        &self,
        session_id: Uuid,
        revoked_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                UPDATE human_sessions
                SET revoked_at = COALESCE(revoked_at, $2)
                WHERE id = $1
                "#,
                &[&session_id, &revoked_at],
            )?;
            Ok(())
        })
    }

    fn get_human_session(&self, session_id: Uuid) -> Option<HumanSession> {
        self.get_human_session_result(session_id).ok().flatten()
    }

    fn get_human_session_result(&self, session_id: Uuid) -> AppResult<Option<HumanSession>> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT id, human_user_id, token_prefix, token_hash, expires_at,
                       revoked_at, last_used_at, created_at
                FROM human_sessions
                WHERE id = $1
                "#,
                &[&session_id],
            )
        })
        .map(|row| row.map(map_human_session))
    }

    fn list_human_sessions(&self, human_user_id: Uuid) -> Vec<HumanSession> {
        self.list_human_sessions_result(human_user_id)
            .unwrap_or_default()
    }

    fn list_human_sessions_result(&self, human_user_id: Uuid) -> AppResult<Vec<HumanSession>> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT id, human_user_id, token_prefix, token_hash, expires_at,
                       revoked_at, last_used_at, created_at
                FROM human_sessions
                WHERE human_user_id = $1
                ORDER BY created_at DESC
                LIMIT 100
                "#,
                &[&human_user_id],
            )
        })
        .map(|rows| rows.into_iter().map(map_human_session).collect())
    }

    fn revoke_human_sessions(
        &self,
        human_user_id: Uuid,
        except_session_id: Option<Uuid>,
        revoked_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<usize> {
        self.with_client(|client| {
            client
                .execute(
                    r#"
                    UPDATE human_sessions
                    SET revoked_at = $3
                    WHERE human_user_id = $1
                      AND revoked_at IS NULL
                      AND ($2::uuid IS NULL OR id <> $2)
                    "#,
                    &[&human_user_id, &except_session_id, &revoked_at],
                )
                .map(|count| count as usize)
        })
    }

    fn update_human_password_hash(
        &self,
        human_user_id: Uuid,
        password_hash: String,
        updated_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                UPDATE human_credentials
                SET password_hash = $2, updated_at = $3
                WHERE human_user_id = $1
                "#,
                &[&human_user_id, &password_hash, &updated_at],
            )?;
            Ok(())
        })
    }

    fn delete_expired_human_sessions(
        &self,
        now: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<usize> {
        self.with_client(|client| {
            client
                .execute(
                    r#"
                    DELETE FROM human_sessions
                    WHERE expires_at <= $1
                       OR (revoked_at IS NOT NULL AND revoked_at <= $1 - INTERVAL '30 days')
                    "#,
                    &[&now],
                )
                .map(|count| count as usize)
        })
    }

    fn insert_human_account_token(&self, token: HumanAccountToken) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO human_account_tokens (
                    id, human_user_id, purpose, token_prefix, token_hash,
                    expires_at, used_at, created_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                "#,
                &[
                    &token.id,
                    &token.human_user_id,
                    &token.purpose,
                    &token.token_prefix,
                    &token.token_hash,
                    &token.expires_at,
                    &token.used_at,
                    &token.created_at,
                ],
            )?;
            Ok(())
        })
    }

    fn find_human_account_token_by_hash(&self, token_hash: &str) -> Option<HumanAccountToken> {
        self.find_human_account_token_by_hash_result(token_hash)
            .ok()
            .flatten()
    }

    fn find_human_account_token_by_hash_result(
        &self,
        token_hash: &str,
    ) -> AppResult<Option<HumanAccountToken>> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT id, human_user_id, purpose, token_prefix, token_hash,
                       expires_at, used_at, created_at
                FROM human_account_tokens
                WHERE token_hash = $1
                "#,
                &[&token_hash],
            )
        })
        .map(|row| row.map(map_human_account_token))
    }

    fn mark_human_account_token_used(
        &self,
        token_id: Uuid,
        used_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        let updated = self.with_client(|client| {
            client.execute(
                r#"
                UPDATE human_account_tokens
                SET used_at = $2
                WHERE id = $1 AND used_at IS NULL
                "#,
                &[&token_id, &used_at],
            )
        })?;
        if updated == 0 {
            return Err(AppError::Conflict(
                "human account token was already used or not found".into(),
            ));
        }
        Ok(())
    }

    fn mark_human_email_verified(
        &self,
        human_user_id: Uuid,
        verified_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO human_email_verifications (human_user_id, verified_at)
                VALUES ($1, $2)
                ON CONFLICT (human_user_id)
                DO UPDATE SET verified_at = EXCLUDED.verified_at
                "#,
                &[&human_user_id, &verified_at],
            )?;
            Ok(())
        })
    }

    fn verify_human_email_atomic(
        &self,
        token_id: Uuid,
        human_user_id: Uuid,
        verified_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        self.with_transaction(|transaction| {
            let consumed = transaction
                .execute(
                    r#"
                    UPDATE human_account_tokens
                    SET used_at = $3
                    WHERE id = $1 AND human_user_id = $2 AND used_at IS NULL
                    "#,
                    &[&token_id, &human_user_id, &verified_at],
                )
                .map_err(map_postgres_error)?;
            if consumed == 0 {
                return Err(AppError::Conflict(
                    "human account token was already used or not found".into(),
                ));
            }
            transaction
                .execute(
                    r#"
                    INSERT INTO human_email_verifications (human_user_id, verified_at)
                    VALUES ($1, $2)
                    ON CONFLICT (human_user_id)
                    DO UPDATE SET verified_at = EXCLUDED.verified_at
                    "#,
                    &[&human_user_id, &verified_at],
                )
                .map_err(map_postgres_error)?;
            Ok(())
        })
    }

    fn reset_human_password_atomic(
        &self,
        token_id: Uuid,
        human_user_id: Uuid,
        password_hash: String,
        updated_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        self.with_transaction(|transaction| {
            let consumed = transaction
                .execute(
                    r#"
                    UPDATE human_account_tokens
                    SET used_at = $3
                    WHERE id = $1 AND human_user_id = $2 AND used_at IS NULL
                    "#,
                    &[&token_id, &human_user_id, &updated_at],
                )
                .map_err(map_postgres_error)?;
            if consumed == 0 {
                return Err(AppError::Conflict(
                    "human account token was already used or not found".into(),
                ));
            }
            let updated = transaction
                .execute(
                    r#"
                    UPDATE human_credentials
                    SET password_hash = $2, updated_at = $3
                    WHERE human_user_id = $1
                    "#,
                    &[&human_user_id, &password_hash, &updated_at],
                )
                .map_err(map_postgres_error)?;
            if updated == 0 {
                return Err(AppError::NotFound("human credential not found".into()));
            }
            transaction
                .execute(
                    r#"
                    UPDATE human_sessions
                    SET revoked_at = $2
                    WHERE human_user_id = $1 AND revoked_at IS NULL
                    "#,
                    &[&human_user_id, &updated_at],
                )
                .map_err(map_postgres_error)?;
            Ok(())
        })
    }

    fn is_human_email_verified(&self, human_user_id: Uuid) -> bool {
        self.is_human_email_verified_result(human_user_id)
            .unwrap_or(false)
    }

    fn is_human_email_verified_result(&self, human_user_id: Uuid) -> AppResult<bool> {
        self.with_client(|client| {
            client
                .query_one(
                    "SELECT COUNT(1) FROM human_email_verifications WHERE human_user_id = $1",
                    &[&human_user_id],
                )
                .map(|row| row.get::<_, i64>(0) > 0)
        })
    }

    fn delete_expired_human_account_tokens(
        &self,
        now: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<usize> {
        self.with_client(|client| {
            client
                .execute(
                    r#"
                    DELETE FROM human_account_tokens
                    WHERE expires_at <= $1
                       OR (used_at IS NOT NULL AND used_at <= $1 - INTERVAL '30 days')
                    "#,
                    &[&now],
                )
                .map(|count| count as usize)
        })
    }
}
