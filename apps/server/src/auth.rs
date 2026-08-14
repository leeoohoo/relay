use super::*;

pub(super) fn bearer_token(headers: &HeaderMap) -> Result<&str, ApiError> {
    let value = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| AppError::Unauthorized("missing bearer token".into()))?;
    let scheme_end = value
        .find(char::is_whitespace)
        .ok_or_else(|| ApiError(AppError::Unauthorized("invalid bearer token".into())))?;
    let (scheme, token) = value.split_at(scheme_end);
    let token = token.trim();
    if !scheme.eq_ignore_ascii_case("bearer") || token.is_empty() {
        return Err(ApiError(AppError::Unauthorized(
            "invalid bearer token".into(),
        )));
    }
    Ok(token)
}

pub(super) fn authenticate_human_session_request(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<HumanUser, ApiError> {
    let token = bearer_token(headers)?;
    state
        .owner_api_limiter
        .check(format!("owner:{}", hash_secret(token)))?;
    state
        .platform
        .authenticate_human_session(token)
        .map_err(ApiError::from)
}

pub(super) fn authenticate_human_request(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<HumanUser, ApiError> {
    let human = authenticate_human_session_request(state, headers)?;
    if state.require_email_verification
        && !state
            .platform
            .is_human_email_verified(human.id)
            .map_err(ApiError::from)?
    {
        return Err(ApiError(AppError::Unauthorized(
            "email verification is required".into(),
        )));
    }
    Ok(human)
}

pub(super) fn require_same_human(
    authenticated_id: Uuid,
    requested_id: Uuid,
) -> Result<(), ApiError> {
    if authenticated_id != requested_id {
        return Err(ApiError(AppError::Unauthorized(
            "cannot access another human user's resources".into(),
        )));
    }
    Ok(())
}

pub(super) fn build_admin_credentials(
    legacy_root_token: Option<&str>,
    credentials_json: Option<&str>,
) -> anyhow::Result<Vec<AdminCredential>> {
    let mut credentials = Vec::new();
    if let Some(token) = legacy_root_token.filter(|value| !value.trim().is_empty()) {
        credentials.push(AdminCredential {
            name: "legacy-root".into(),
            token_hash: hash_secret(token),
            scopes: HashSet::from(["*".to_string()]),
        });
    }

    if let Some(raw_json) = credentials_json.filter(|value| !value.trim().is_empty()) {
        let configured: Vec<AdminCredentialEnv> = serde_json::from_str(raw_json)
            .map_err(|error| anyhow::anyhow!("invalid ADMIN_API_TOKENS_JSON: {error}"))?;
        for item in configured {
            let name = item.name.trim();
            let token = item.token.trim();
            if name.is_empty() || token.len() < 24 {
                anyhow::bail!(
                    "each ADMIN_API_TOKENS_JSON entry needs a name and a token of at least 24 characters"
                );
            }
            let scopes = item
                .scopes
                .into_iter()
                .map(|scope| scope.trim().to_ascii_lowercase())
                .filter(|scope| !scope.is_empty())
                .collect::<HashSet<_>>();
            if scopes.is_empty()
                || scopes.iter().any(|scope| {
                    !matches!(scope.as_str(), "*" | ADMIN_SCOPE_READ | ADMIN_SCOPE_AGENTS)
                })
            {
                anyhow::bail!(
                    "admin credential {name} has no scopes or contains an unsupported scope"
                );
            }
            credentials.push(AdminCredential {
                name: name.to_string(),
                token_hash: hash_secret(token),
                scopes,
            });
        }
    }

    let mut token_hashes = HashSet::new();
    if credentials
        .iter()
        .any(|credential| !token_hashes.insert(credential.token_hash.clone()))
    {
        anyhow::bail!("admin tokens must be unique");
    }
    Ok(credentials)
}

pub(super) fn require_admin(
    state: &AppState,
    headers: &HeaderMap,
    required_scope: &str,
) -> Result<(), ApiError> {
    state.admin_api_limiter.check("admin")?;
    if state.admin_credentials.is_empty() {
        return Err(ApiError(AppError::Unauthorized(
            "admin API token is not configured".into(),
        )));
    }
    let actual_hash = hash_secret(bearer_token(headers)?);
    let credential = state
        .admin_credentials
        .iter()
        .find(|credential| credential.token_hash == actual_hash)
        .ok_or_else(|| ApiError(AppError::Unauthorized("invalid admin API token".into())))?;
    if !credential.allows(required_scope) {
        tracing::warn!(
            admin_credential = %credential.name,
            %required_scope,
            "admin credential denied by scope"
        );
        return Err(ApiError(AppError::Unauthorized(format!(
            "admin token lacks required scope {required_scope}"
        ))));
    }
    Ok(())
}

pub(super) fn require_dev_endpoints(state: &AppState) -> Result<(), ApiError> {
    if !state.enable_dev_endpoints {
        return Err(ApiError(AppError::NotFound(
            "development endpoints are disabled".into(),
        )));
    }
    Ok(())
}

#[derive(Debug)]
pub(super) struct ApiError(pub(super) AppError);

impl From<AppError> for ApiError {
    fn from(value: AppError) -> Self {
        Self(value)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let status = match &self.0 {
            AppError::Validation(_) => StatusCode::BAD_REQUEST,
            AppError::NotFound(_) => StatusCode::NOT_FOUND,
            AppError::Conflict(_) => StatusCode::CONFLICT,
            AppError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            AppError::RateLimited(_) => StatusCode::TOO_MANY_REQUESTS,
            AppError::Internal(message) => {
                tracing::error!(error = %message, "internal API error");
                StatusCode::INTERNAL_SERVER_ERROR
            }
        };
        if !matches!(self.0, AppError::Internal(_)) {
            tracing::warn!(
                status = %status,
                code = %self.0.code(),
                error = %self.0,
                "API request rejected"
            );
        }

        let body = Json(ApiErrorResponse {
            code: self.0.code().to_string(),
            message: if matches!(self.0, AppError::Internal(_)) {
                "internal server error".into()
            } else {
                self.0.to_string()
            },
        });

        (status, body).into_response()
    }
}
