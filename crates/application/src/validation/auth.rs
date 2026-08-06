use super::*;

pub(crate) fn generate_code() -> String {
    let compact = Uuid::new_v4()
        .simple()
        .to_string()
        .chars()
        .take(6)
        .collect::<String>()
        .to_uppercase();

    format!("AI-SOCIAL-{compact}")
}

pub(crate) fn generate_agent_key() -> String {
    format!("agk_{}", Uuid::new_v4().simple())
}

pub(crate) fn agent_key_is_active_record(key: &AgentKeyRecord) -> bool {
    key.revoked_at.is_none()
        && key
            .expires_at
            .is_none_or(|expires_at| now_utc() < expires_at)
}

pub(crate) fn generate_human_session_token() -> String {
    format!("hus_{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

pub(crate) fn normalize_human_email(value: &str) -> AppResult<String> {
    let email = value.trim().to_lowercase();
    if email.is_empty() || email.len() > 320 || !email.contains('@') {
        return Err(AppError::Validation("a valid email is required".into()));
    }
    Ok(email)
}

pub(crate) fn normalize_human_display_name(value: &str) -> AppResult<String> {
    let display_name = value.trim().to_string();
    if display_name.is_empty() || display_name.chars().count() > 100 {
        return Err(AppError::Validation(
            "display_name must contain 1 to 100 characters".into(),
        ));
    }
    Ok(display_name)
}

pub(crate) fn validate_human_password(password: &str) -> AppResult<()> {
    if !(8..=128).contains(&password.chars().count()) {
        return Err(AppError::Validation(
            "password must contain 8 to 128 characters".into(),
        ));
    }
    Ok(())
}

pub(crate) fn hash_human_password(password: &str) -> AppResult<String> {
    let salt = SaltString::encode_b64(Uuid::new_v4().as_bytes()).map_err(|error| {
        AppError::Validation(format!("failed to create password salt: {error}"))
    })?;
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|error| AppError::Validation(format!("failed to hash password: {error}")))
}

pub(crate) fn invalid_human_credentials() -> AppError {
    AppError::Unauthorized("invalid email or password".into())
}

pub(crate) fn normalize_idempotency_key(value: &str) -> AppResult<String> {
    let value = value.trim();
    if value.is_empty() || value.len() > 160 {
        return Err(AppError::Validation(
            "idempotency_key must contain 1 to 160 bytes".into(),
        ));
    }
    Ok(value.to_string())
}

pub(crate) fn hash_json_payload(value: &serde_json::Value) -> AppResult<String> {
    serde_json::to_string(value)
        .map(|serialized| hash_secret(&serialized))
        .map_err(|error| AppError::Validation(format!("failed to hash request payload: {error}")))
}
