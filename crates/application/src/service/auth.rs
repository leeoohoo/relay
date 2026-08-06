use super::*;

pub trait AuthPlatformRepository: Send + Sync {
    fn health_check(&self) -> AppResult<()> {
        Ok(())
    }
    fn find_human_user_by_email(&self, email: &str) -> Option<HumanUser>;
    fn find_human_user_by_email_result(&self, email: &str) -> AppResult<Option<HumanUser>> {
        Ok(self.find_human_user_by_email(email))
    }
    fn get_human_user(&self, user_id: Uuid) -> Option<HumanUser>;
    fn get_human_user_result(&self, user_id: Uuid) -> AppResult<Option<HumanUser>> {
        Ok(self.get_human_user(user_id))
    }
    fn list_human_users(&self) -> Vec<HumanUser>;
    fn insert_human_user(&self, user: HumanUser) -> AppResult<HumanUser>;
    fn human_user_exists(&self, user_id: Uuid) -> bool;
    fn human_user_exists_result(&self, user_id: Uuid) -> AppResult<bool> {
        Ok(self.human_user_exists(user_id))
    }
    fn get_human_credential(&self, _user_id: Uuid) -> Option<HumanCredential> {
        None
    }
    fn get_human_credential_result(&self, user_id: Uuid) -> AppResult<Option<HumanCredential>> {
        Ok(self.get_human_credential(user_id))
    }
    fn insert_human_auth_bundle(
        &self,
        _user: HumanUser,
        _credential: HumanCredential,
    ) -> AppResult<HumanUser> {
        Err(AppError::Validation(
            "human authentication is not supported by this repository".into(),
        ))
    }
    fn get_human_harness_account(
        &self,
        _human_user_id: Uuid,
    ) -> Option<ai_chat_domain::agent_identity::HumanHarnessAccount> {
        None
    }
    fn get_human_harness_account_result(
        &self,
        human_user_id: Uuid,
    ) -> AppResult<Option<ai_chat_domain::agent_identity::HumanHarnessAccount>> {
        Ok(self.get_human_harness_account(human_user_id))
    }
    fn upsert_human_harness_account(
        &self,
        _account: ai_chat_domain::agent_identity::HumanHarnessAccount,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "human Harness accounts are not supported by this repository".into(),
        ))
    }
    fn insert_human_session(&self, _session: HumanSession) -> AppResult<()> {
        Err(AppError::Validation(
            "human sessions are not supported by this repository".into(),
        ))
    }
    fn find_human_session_by_token_hash(&self, _token_hash: &str) -> Option<HumanSession> {
        None
    }
    fn find_human_session_by_token_hash_result(
        &self,
        token_hash: &str,
    ) -> AppResult<Option<HumanSession>> {
        Ok(self.find_human_session_by_token_hash(token_hash))
    }
    fn touch_human_session(
        &self,
        _session_id: Uuid,
        _used_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "human sessions are not supported by this repository".into(),
        ))
    }
    fn revoke_human_session(
        &self,
        _session_id: Uuid,
        _revoked_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "human sessions are not supported by this repository".into(),
        ))
    }
    fn get_human_session(&self, _session_id: Uuid) -> Option<HumanSession> {
        None
    }
    fn get_human_session_result(&self, session_id: Uuid) -> AppResult<Option<HumanSession>> {
        Ok(self.get_human_session(session_id))
    }
    fn list_human_sessions(&self, _human_user_id: Uuid) -> Vec<HumanSession> {
        Vec::new()
    }
    fn list_human_sessions_result(&self, human_user_id: Uuid) -> AppResult<Vec<HumanSession>> {
        Ok(self.list_human_sessions(human_user_id))
    }
    fn revoke_human_sessions(
        &self,
        _human_user_id: Uuid,
        _except_session_id: Option<Uuid>,
        _revoked_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<usize> {
        Ok(0)
    }
    fn update_human_password_hash(
        &self,
        _human_user_id: Uuid,
        _password_hash: String,
        _updated_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "password updates are not supported by this repository".into(),
        ))
    }
    fn delete_expired_human_sessions(
        &self,
        _now: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<usize> {
        Ok(0)
    }
    fn insert_human_account_token(&self, _token: HumanAccountToken) -> AppResult<()> {
        Err(AppError::Validation(
            "human account tokens are not supported by this repository".into(),
        ))
    }
    fn find_human_account_token_by_hash(&self, _token_hash: &str) -> Option<HumanAccountToken> {
        None
    }
    fn find_human_account_token_by_hash_result(
        &self,
        token_hash: &str,
    ) -> AppResult<Option<HumanAccountToken>> {
        Ok(self.find_human_account_token_by_hash(token_hash))
    }
    fn mark_human_account_token_used(
        &self,
        _token_id: Uuid,
        _used_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "human account tokens are not supported by this repository".into(),
        ))
    }
    fn mark_human_email_verified(
        &self,
        _human_user_id: Uuid,
        _verified_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "human email verification is not supported by this repository".into(),
        ))
    }
    fn verify_human_email_atomic(
        &self,
        token_id: Uuid,
        human_user_id: Uuid,
        verified_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        self.mark_human_email_verified(human_user_id, verified_at)?;
        self.mark_human_account_token_used(token_id, verified_at)
    }
    fn reset_human_password_atomic(
        &self,
        token_id: Uuid,
        human_user_id: Uuid,
        password_hash: String,
        updated_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        self.update_human_password_hash(human_user_id, password_hash, updated_at)?;
        self.revoke_human_sessions(human_user_id, None, updated_at)?;
        self.mark_human_account_token_used(token_id, updated_at)
    }
    fn is_human_email_verified(&self, _human_user_id: Uuid) -> bool {
        false
    }
    fn is_human_email_verified_result(&self, human_user_id: Uuid) -> AppResult<bool> {
        Ok(self.is_human_email_verified(human_user_id))
    }
    fn delete_expired_human_account_tokens(
        &self,
        _now: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<usize> {
        Ok(0)
    }
}
