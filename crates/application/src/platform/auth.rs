use super::*;

impl<R: PlatformRepository, V: OwnershipProofVerifier> PlatformApp<R, V> {
    pub fn dev_login(&self, input: DevLoginInput) -> AppResult<HumanUser> {
        let email = input.email.trim().to_lowercase();
        let display_name = input.display_name.trim().to_string();

        if email.is_empty() || display_name.is_empty() {
            return Err(AppError::Validation(
                "email and display_name are required".into(),
            ));
        }

        if let Some(existing) = self.repo.find_human_user_by_email_result(&email)? {
            return Ok(existing);
        }

        let user = HumanUser {
            id: Uuid::new_v4(),
            email,
            display_name,
            created_at: now_utc(),
        };

        self.repo.insert_human_user(user)
    }

    pub fn register_human(&self, input: RegisterHumanInput) -> AppResult<HumanAuthResult> {
        let email = normalize_human_email(&input.email)?;
        let display_name = normalize_human_display_name(&input.display_name)?;
        validate_human_password(&input.password)?;

        if self.repo.find_human_user_by_email_result(&email)?.is_some() {
            return Err(AppError::Conflict("email is already registered".into()));
        }

        let created_at = now_utc();
        let user = HumanUser {
            id: Uuid::new_v4(),
            email,
            display_name,
            created_at,
        };
        let credential = HumanCredential {
            human_user_id: user.id,
            password_hash: hash_human_password(&input.password)?,
            created_at,
            updated_at: created_at,
        };
        let user = self.repo.insert_human_auth_bundle(user, credential)?;
        self.issue_human_session(user.id)
    }

    pub fn login_human(&self, input: LoginHumanInput) -> AppResult<HumanAuthResult> {
        let email = normalize_human_email(&input.email)?;
        let user = self
            .repo
            .find_human_user_by_email_result(&email)?
            .ok_or_else(invalid_human_credentials)?;
        let credential = self
            .repo
            .get_human_credential_result(user.id)?
            .ok_or_else(invalid_human_credentials)?;
        let parsed_hash = PasswordHash::new(&credential.password_hash)
            .map_err(|_| invalid_human_credentials())?;

        Argon2::default()
            .verify_password(input.password.as_bytes(), &parsed_hash)
            .map_err(|_| invalid_human_credentials())?;

        self.issue_human_session(user.id)
    }

    pub fn issue_human_session(&self, human_user_id: Uuid) -> AppResult<HumanAuthResult> {
        let user = self
            .repo
            .get_human_user_result(human_user_id)?
            .ok_or_else(|| AppError::NotFound("human user not found".into()))?;
        let session_token = generate_human_session_token();
        let created_at = now_utc();
        let expires_at = created_at + Duration::days(30);
        let session = HumanSession {
            id: Uuid::new_v4(),
            human_user_id,
            token_prefix: session_token.chars().take(12).collect(),
            token_hash: hash_secret(&session_token),
            expires_at,
            revoked_at: None,
            last_used_at: None,
            created_at,
        };
        self.repo.insert_human_session(session)?;

        Ok(HumanAuthResult {
            user,
            session_token,
            expires_at,
        })
    }

    pub fn authenticate_human_session(&self, session_token: &str) -> AppResult<HumanUser> {
        if !session_token.starts_with("hus_") {
            return Err(AppError::Unauthorized("invalid human session".into()));
        }

        let session = self
            .repo
            .find_human_session_by_token_hash_result(&hash_secret(session_token))?
            .ok_or_else(|| AppError::Unauthorized("invalid human session".into()))?;
        if session.revoked_at.is_some() {
            return Err(AppError::Unauthorized(
                "human session has been revoked".into(),
            ));
        }
        if now_utc() >= session.expires_at {
            return Err(AppError::Unauthorized("human session has expired".into()));
        }

        let user = self
            .repo
            .get_human_user_result(session.human_user_id)?
            .ok_or_else(|| AppError::Unauthorized("human session user not found".into()))?;
        self.repo.touch_human_session(session.id, now_utc())?;
        Ok(user)
    }

    pub fn logout_human_session(&self, session_token: &str) -> AppResult<()> {
        let session = self
            .repo
            .find_human_session_by_token_hash_result(&hash_secret(session_token))?
            .ok_or_else(|| AppError::Unauthorized("invalid human session".into()))?;
        self.repo.revoke_human_session(session.id, now_utc())
    }

    pub fn list_human_sessions(
        &self,
        human_user_id: Uuid,
        current_session_token: &str,
    ) -> AppResult<Vec<HumanSessionView>> {
        if !self.repo.human_user_exists_result(human_user_id)? {
            return Err(AppError::NotFound("human user not found".into()));
        }
        let current_hash = hash_secret(current_session_token);
        let now = now_utc();
        Ok(self
            .repo
            .list_human_sessions_result(human_user_id)?
            .into_iter()
            .filter(|session| session.revoked_at.is_none() && session.expires_at > now)
            .map(|session| HumanSessionView {
                id: session.id,
                token_prefix: session.token_prefix,
                expires_at: session.expires_at,
                last_used_at: session.last_used_at,
                created_at: session.created_at,
                is_current: session.token_hash == current_hash,
            })
            .collect())
    }

    pub fn revoke_owned_human_session(
        &self,
        human_user_id: Uuid,
        session_id: Uuid,
    ) -> AppResult<()> {
        let session = self
            .repo
            .get_human_session_result(session_id)?
            .ok_or_else(|| AppError::NotFound("human session not found".into()))?;
        if session.human_user_id != human_user_id {
            return Err(AppError::Unauthorized(
                "cannot revoke another human user's session".into(),
            ));
        }
        self.repo.revoke_human_session(session_id, now_utc())
    }

    pub fn revoke_other_human_sessions(
        &self,
        human_user_id: Uuid,
        current_session_token: &str,
    ) -> AppResult<usize> {
        let current_session = self
            .repo
            .find_human_session_by_token_hash_result(&hash_secret(current_session_token))?
            .ok_or_else(|| AppError::Unauthorized("invalid human session".into()))?;
        if current_session.human_user_id != human_user_id {
            return Err(AppError::Unauthorized(
                "human session belongs to another user".into(),
            ));
        }
        self.repo
            .revoke_human_sessions(human_user_id, Some(current_session.id), now_utc())
    }

    pub fn change_human_password(
        &self,
        human_user_id: Uuid,
        current_session_token: &str,
        input: ChangeHumanPasswordInput,
    ) -> AppResult<usize> {
        validate_human_password(&input.new_password)?;
        if input.current_password == input.new_password {
            return Err(AppError::Validation(
                "new password must differ from current password".into(),
            ));
        }
        let credential = self
            .repo
            .get_human_credential_result(human_user_id)?
            .ok_or_else(invalid_human_credentials)?;
        let parsed_hash = PasswordHash::new(&credential.password_hash)
            .map_err(|_| invalid_human_credentials())?;
        Argon2::default()
            .verify_password(input.current_password.as_bytes(), &parsed_hash)
            .map_err(|_| invalid_human_credentials())?;

        let current_session = self
            .repo
            .find_human_session_by_token_hash_result(&hash_secret(current_session_token))?
            .ok_or_else(|| AppError::Unauthorized("invalid human session".into()))?;
        if current_session.human_user_id != human_user_id {
            return Err(AppError::Unauthorized(
                "human session belongs to another user".into(),
            ));
        }

        let now = now_utc();
        self.repo.update_human_password_hash(
            human_user_id,
            hash_human_password(&input.new_password)?,
            now,
        )?;
        self.repo
            .revoke_human_sessions(human_user_id, Some(current_session.id), now)
    }

    pub fn issue_human_email_verification(
        &self,
        human_user_id: Uuid,
    ) -> AppResult<HumanAccountTokenResult> {
        if self.repo.is_human_email_verified_result(human_user_id)? {
            return Err(AppError::Conflict("human email is already verified".into()));
        }
        self.issue_human_account_token(human_user_id, "email_verification", 24)
    }

    pub fn verify_human_email(&self, plaintext_token: &str) -> AppResult<HumanUser> {
        let token = self.load_active_human_account_token(plaintext_token, "email_verification")?;
        let now = now_utc();
        self.repo
            .verify_human_email_atomic(token.id, token.human_user_id, now)?;
        self.repo
            .get_human_user_result(token.human_user_id)?
            .ok_or_else(|| AppError::NotFound("human user not found".into()))
    }

    pub fn issue_human_password_reset(
        &self,
        email: &str,
    ) -> AppResult<Option<(HumanUser, HumanAccountTokenResult)>> {
        let email = normalize_human_email(email)?;
        let Some(user) = self.repo.find_human_user_by_email_result(&email)? else {
            return Ok(None);
        };
        let token = self.issue_human_account_token(user.id, "password_reset", 1)?;
        Ok(Some((user, token)))
    }

    pub fn reset_human_password(&self, input: ResetHumanPasswordInput) -> AppResult<()> {
        validate_human_password(&input.new_password)?;
        let token = self.load_active_human_account_token(&input.token, "password_reset")?;
        let now = now_utc();
        self.repo.reset_human_password_atomic(
            token.id,
            token.human_user_id,
            hash_human_password(&input.new_password)?,
            now,
        )
    }

    pub fn is_human_email_verified(&self, human_user_id: Uuid) -> AppResult<bool> {
        if !self.repo.human_user_exists_result(human_user_id)? {
            return Err(AppError::NotFound("human user not found".into()));
        }
        self.repo.is_human_email_verified_result(human_user_id)
    }

    pub(super) fn issue_human_account_token(
        &self,
        human_user_id: Uuid,
        purpose: &str,
        ttl_hours: i64,
    ) -> AppResult<HumanAccountTokenResult> {
        if !self.repo.human_user_exists_result(human_user_id)? {
            return Err(AppError::NotFound("human user not found".into()));
        }
        let prefix = match purpose {
            "email_verification" => "hev_",
            "password_reset" => "hpr_",
            _ => "hat_",
        };
        let plaintext = format!(
            "{}{}{}",
            prefix,
            Uuid::new_v4().simple(),
            Uuid::new_v4().simple()
        );
        let now = now_utc();
        let expires_at = now + Duration::hours(ttl_hours);
        self.repo.insert_human_account_token(HumanAccountToken {
            id: Uuid::new_v4(),
            human_user_id,
            purpose: purpose.into(),
            token_prefix: plaintext.chars().take(12).collect(),
            token_hash: hash_secret(&plaintext),
            expires_at,
            used_at: None,
            created_at: now,
        })?;
        Ok(HumanAccountTokenResult {
            token: plaintext,
            expires_at,
        })
    }

    pub(super) fn load_active_human_account_token(
        &self,
        plaintext_token: &str,
        expected_purpose: &str,
    ) -> AppResult<HumanAccountToken> {
        let token = self
            .repo
            .find_human_account_token_by_hash_result(&hash_secret(plaintext_token))?
            .ok_or_else(|| AppError::Unauthorized("invalid or expired account token".into()))?;
        if token.purpose != expected_purpose
            || token.used_at.is_some()
            || token.expires_at <= now_utc()
        {
            return Err(AppError::Unauthorized(
                "invalid or expired account token".into(),
            ));
        }
        Ok(token)
    }
}
