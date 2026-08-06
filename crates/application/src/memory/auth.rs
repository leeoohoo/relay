use super::*;

impl AuthPlatformRepository for MemoryPlatformRepository {
    fn health_check(&self) -> AppResult<()> {
        self.inner.health_check()
    }

    fn find_human_user_by_email(&self, email: &str) -> Option<HumanUser> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard
            .human_users
            .values()
            .find(|user| user.email.eq_ignore_ascii_case(email))
            .cloned()
    }

    fn get_human_user(&self, user_id: Uuid) -> Option<HumanUser> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard.human_users.get(&user_id).cloned()
    }

    fn list_human_users(&self) -> Vec<HumanUser> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut users = guard.human_users.values().cloned().collect::<Vec<_>>();
        users.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        users
    }

    fn insert_human_user(&self, user: HumanUser) -> AppResult<HumanUser> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        guard.human_users.insert(user.id, user.clone());
        Ok(user)
    }

    fn human_user_exists(&self, user_id: Uuid) -> bool {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard.human_users.contains_key(&user_id)
    }

    fn get_human_credential(&self, user_id: Uuid) -> Option<HumanCredential> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard.human_credentials.get(&user_id).cloned()
    }

    fn insert_human_auth_bundle(
        &self,
        user: HumanUser,
        credential: HumanCredential,
    ) -> AppResult<HumanUser> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if guard
            .human_users
            .values()
            .any(|existing| existing.email.eq_ignore_ascii_case(&user.email))
        {
            return Err(ai_chat_shared::AppError::Conflict(
                "email is already registered".into(),
            ));
        }
        guard.human_credentials.insert(user.id, credential);
        guard.human_users.insert(user.id, user.clone());
        Ok(user)
    }

    fn get_human_harness_account(&self, human_user_id: Uuid) -> Option<HumanHarnessAccount> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard.human_harness_accounts.get(&human_user_id).cloned()
    }

    fn upsert_human_harness_account(&self, account: HumanHarnessAccount) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        guard
            .human_harness_accounts
            .insert(account.human_user_id, account);
        Ok(())
    }

    fn insert_human_session(&self, session: HumanSession) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        guard.human_sessions.insert(session.id, session);
        Ok(())
    }

    fn find_human_session_by_token_hash(&self, token_hash: &str) -> Option<HumanSession> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard
            .human_sessions
            .values()
            .find(|session| session.token_hash == token_hash)
            .cloned()
    }

    fn touch_human_session(
        &self,
        session_id: Uuid,
        used_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let session = guard
            .human_sessions
            .get_mut(&session_id)
            .ok_or_else(|| ai_chat_shared::AppError::NotFound("human session not found".into()))?;
        session.last_used_at = Some(used_at);
        Ok(())
    }

    fn revoke_human_session(
        &self,
        session_id: Uuid,
        revoked_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let session = guard
            .human_sessions
            .get_mut(&session_id)
            .ok_or_else(|| ai_chat_shared::AppError::NotFound("human session not found".into()))?;
        session.revoked_at = Some(revoked_at);
        Ok(())
    }

    fn get_human_session(&self, session_id: Uuid) -> Option<HumanSession> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard.human_sessions.get(&session_id).cloned()
    }

    fn list_human_sessions(&self, human_user_id: Uuid) -> Vec<HumanSession> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut sessions = guard
            .human_sessions
            .values()
            .filter(|session| session.human_user_id == human_user_id)
            .cloned()
            .collect::<Vec<_>>();
        sessions.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        sessions
    }

    fn revoke_human_sessions(
        &self,
        human_user_id: Uuid,
        except_session_id: Option<Uuid>,
        revoked_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<usize> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let mut count = 0;
        for session in guard.human_sessions.values_mut() {
            if session.human_user_id == human_user_id
                && session.revoked_at.is_none()
                && Some(session.id) != except_session_id
            {
                session.revoked_at = Some(revoked_at);
                count += 1;
            }
        }
        Ok(count)
    }

    fn update_human_password_hash(
        &self,
        human_user_id: Uuid,
        password_hash: String,
        updated_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let credential = guard
            .human_credentials
            .get_mut(&human_user_id)
            .ok_or_else(|| {
                ai_chat_shared::AppError::NotFound("human credential not found".into())
            })?;
        credential.password_hash = password_hash;
        credential.updated_at = updated_at;
        Ok(())
    }

    fn delete_expired_human_sessions(
        &self,
        now: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<usize> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let before = guard.human_sessions.len();
        guard.human_sessions.retain(|_, session| {
            session.expires_at > now
                || session
                    .revoked_at
                    .is_some_and(|revoked_at| revoked_at > now - chrono::Duration::days(30))
        });
        Ok(before - guard.human_sessions.len())
    }

    fn insert_human_account_token(&self, token: HumanAccountToken) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        guard.human_account_tokens.insert(token.id, token);
        Ok(())
    }

    fn find_human_account_token_by_hash(&self, token_hash: &str) -> Option<HumanAccountToken> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard
            .human_account_tokens
            .values()
            .find(|token| token.token_hash == token_hash)
            .cloned()
    }

    fn mark_human_account_token_used(
        &self,
        token_id: Uuid,
        used_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let token = guard
            .human_account_tokens
            .get_mut(&token_id)
            .ok_or_else(|| {
                ai_chat_shared::AppError::NotFound("human account token not found".into())
            })?;
        if token.used_at.is_some() {
            return Err(ai_chat_shared::AppError::Conflict(
                "human account token was already used".into(),
            ));
        }
        token.used_at = Some(used_at);
        Ok(())
    }

    fn mark_human_email_verified(
        &self,
        human_user_id: Uuid,
        verified_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        guard
            .human_email_verifications
            .insert(human_user_id, verified_at);
        Ok(())
    }

    fn verify_human_email_atomic(
        &self,
        token_id: Uuid,
        human_user_id: Uuid,
        verified_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let token = guard
            .human_account_tokens
            .get_mut(&token_id)
            .filter(|token| token.human_user_id == human_user_id)
            .ok_or_else(|| {
                ai_chat_shared::AppError::NotFound("human account token not found".into())
            })?;
        if token.used_at.is_some() {
            return Err(ai_chat_shared::AppError::Conflict(
                "human account token was already used".into(),
            ));
        }
        token.used_at = Some(verified_at);
        guard
            .human_email_verifications
            .insert(human_user_id, verified_at);
        Ok(())
    }

    fn reset_human_password_atomic(
        &self,
        token_id: Uuid,
        human_user_id: Uuid,
        password_hash: String,
        updated_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let token = guard
            .human_account_tokens
            .get(&token_id)
            .filter(|token| token.human_user_id == human_user_id)
            .ok_or_else(|| {
                ai_chat_shared::AppError::NotFound("human account token not found".into())
            })?;
        if token.used_at.is_some() {
            return Err(ai_chat_shared::AppError::Conflict(
                "human account token was already used".into(),
            ));
        }
        let credential = guard
            .human_credentials
            .get_mut(&human_user_id)
            .ok_or_else(|| {
                ai_chat_shared::AppError::NotFound("human credential not found".into())
            })?;
        credential.password_hash = password_hash;
        credential.updated_at = updated_at;
        for session in guard.human_sessions.values_mut().filter(|session| {
            session.human_user_id == human_user_id && session.revoked_at.is_none()
        }) {
            session.revoked_at = Some(updated_at);
        }
        guard
            .human_account_tokens
            .get_mut(&token_id)
            .expect("validated account token must still exist")
            .used_at = Some(updated_at);
        Ok(())
    }

    fn is_human_email_verified(&self, human_user_id: Uuid) -> bool {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard.human_email_verifications.contains_key(&human_user_id)
    }

    fn delete_expired_human_account_tokens(
        &self,
        now: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<usize> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let before = guard.human_account_tokens.len();
        guard.human_account_tokens.retain(|_, token| {
            token.expires_at > now
                || token
                    .used_at
                    .is_some_and(|used_at| used_at > now - chrono::Duration::days(30))
        });
        Ok(before - guard.human_account_tokens.len())
    }
}
