use super::*;

pub(super) async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        service: "ai-chat-server",
    })
}

pub(super) async fn readiness(
    State(state): State<AppState>,
) -> Result<Json<ReadinessResponse>, ApiError> {
    state.platform.health_check()?;
    Ok(Json(ReadinessResponse {
        status: "ready",
        repository: "ok",
    }))
}

pub(super) fn build_cors_layer(origins: &[String]) -> anyhow::Result<CorsLayer> {
    let origins = origins
        .iter()
        .map(|origin| origin.parse::<HeaderValue>())
        .collect::<Result<Vec<_>, _>>()?;
    Ok(CorsLayer::new()
        .allow_origin(AllowOrigin::list(origins))
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            ACCEPT,
            AUTHORIZATION,
            CACHE_CONTROL,
            CONTENT_TYPE,
            HeaderName::from_static("x-agent-key"),
            HeaderName::from_static("x-agent-run-token"),
            HeaderName::from_static("x-relay-session-kind"),
            HeaderName::from_static("idempotency-key"),
            HeaderName::from_static("last-event-id"),
            HeaderName::from_static("mcp-protocol-version"),
        ]))
}

pub(super) async fn runtime_config(State(state): State<AppState>) -> Json<RuntimeConfigResponse> {
    Json(RuntimeConfigResponse {
        dev_endpoints_enabled: state.enable_dev_endpoints,
        admin_token_configured: !state.admin_credentials.is_empty(),
        email_verification_required: state.require_email_verification,
        harness_mode: state.harness_provisioner.mode_key(),
        project_types: company_project_type_catalog(),
    })
}

pub(super) async fn register_human(
    State(state): State<AppState>,
    Json(input): Json<RegisterHumanInput>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state
        .login_limiter
        .check(format!("register:{}", input.email.trim().to_lowercase()))?;
    let auth = state.platform.register_human(input)?;
    let harness = ensure_harness_account(&state, &auth.user).await;
    let verification = state
        .platform
        .issue_human_email_verification(auth.user.id)?;
    let email_verification_sent = deliver_account_token(
        &state,
        &auth.user.email,
        "email_verification",
        &verification.token,
        verification.expires_at,
    )
    .await;
    Ok(Json(serde_json::json!({
        "user": auth.user,
        "session_token": auth.session_token,
        "expires_at": auth.expires_at,
        "email_verified": false,
        "email_verification_sent": email_verification_sent,
        "harness": harness
    })))
}

pub(super) async fn login_human(
    State(state): State<AppState>,
    Json(input): Json<LoginHumanInput>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state
        .login_limiter
        .check(format!("login:{}", input.email.trim().to_lowercase()))?;
    let auth = state.platform.login_human(input)?;
    let harness = ensure_harness_account(&state, &auth.user).await;
    let email_verified = state.platform.is_human_email_verified(auth.user.id)?;
    Ok(Json(serde_json::json!({
        "user": auth.user,
        "session_token": auth.session_token,
        "expires_at": auth.expires_at,
        "email_verified": email_verified,
        "harness": harness
    })))
}

pub(super) async fn get_authenticated_human(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let user = authenticate_human_session_request(&state, &headers)?;
    let email_verified = state.platform.is_human_email_verified(user.id)?;
    let harness = state.harness_provisioner.account(user.id)?;
    Ok(Json(serde_json::json!({
        "user": user,
        "email_verified": email_verified,
        "harness": harness
    })))
}

pub(super) async fn ensure_harness_account(
    state: &AppState,
    user: &HumanUser,
) -> Option<HumanHarnessAccount> {
    if !state.harness_provisioner.is_enabled() {
        return None;
    }
    match state.harness_provisioner.ensure_account(user).await {
        Ok(account) => account,
        Err(error) => {
            tracing::warn!(
                human_user_id = %user.id,
                error = %error,
                "Harness provisioning failed; Human authentication remains available and login will retry"
            );
            state.harness_provisioner.account(user.id).ok().flatten()
        }
    }
}

pub(super) async fn logout_human(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let token = bearer_token(&headers)?;
    state
        .owner_api_limiter
        .check(format!("owner:{}", hash_secret(token)))?;
    state.platform.logout_human_session(token)?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub(super) async fn list_authenticated_human_sessions(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_session_request(&state, &headers)?;
    let token = bearer_token(&headers)?;
    let sessions = state.platform.list_human_sessions(human.id, token)?;
    Ok(Json(serde_json::json!({ "sessions": sessions })))
}

pub(super) async fn revoke_authenticated_human_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_session_request(&state, &headers)?;
    state
        .platform
        .revoke_owned_human_session(human.id, session_id)?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub(super) async fn revoke_other_authenticated_human_sessions(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_session_request(&state, &headers)?;
    let token = bearer_token(&headers)?;
    let revoked_count = state
        .platform
        .revoke_other_human_sessions(human.id, token)?;
    Ok(Json(serde_json::json!({ "revoked_count": revoked_count })))
}

pub(super) async fn change_human_password(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<ChangeHumanPasswordInput>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_session_request(&state, &headers)?;
    let token = bearer_token(&headers)?;
    let revoked_count = state
        .platform
        .change_human_password(human.id, token, input)?;
    Ok(Json(serde_json::json!({
        "ok": true,
        "revoked_other_sessions": revoked_count
    })))
}

pub(super) async fn request_human_email_verification(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_session_request(&state, &headers)?;
    let verification = state.platform.issue_human_email_verification(human.id)?;
    let delivered = deliver_account_token(
        &state,
        &human.email,
        "email_verification",
        &verification.token,
        verification.expires_at,
    )
    .await;
    let mut response = serde_json::json!({
        "ok": true,
        "delivered": delivered,
        "expires_at": verification.expires_at
    });
    if state.enable_dev_endpoints {
        response["development_token"] = serde_json::Value::String(verification.token);
    }
    Ok(Json(response))
}

pub(super) async fn verify_human_email(
    State(state): State<AppState>,
    Json(input): Json<HumanAccountTokenInput>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let user = state.platform.verify_human_email(&input.token)?;
    Ok(Json(serde_json::json!({
        "ok": true,
        "user": user,
        "email_verified": true
    })))
}

pub(super) async fn request_human_password_reset(
    State(state): State<AppState>,
    Json(input): Json<HumanEmailInput>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state.login_limiter.check(format!(
        "password-reset:{}",
        input.email.trim().to_lowercase()
    ))?;
    let issued = state.platform.issue_human_password_reset(&input.email)?;
    let mut response = serde_json::json!({
        "ok": true,
        "message": "If the account exists, password reset instructions have been sent."
    });
    if let Some((user, reset)) = issued {
        let delivered = deliver_account_token(
            &state,
            &user.email,
            "password_reset",
            &reset.token,
            reset.expires_at,
        )
        .await;
        response["delivered"] = serde_json::Value::Bool(delivered);
        if state.enable_dev_endpoints {
            response["development_token"] = serde_json::Value::String(reset.token);
        }
    }
    Ok(Json(response))
}

pub(super) async fn confirm_human_password_reset(
    State(state): State<AppState>,
    Json(input): Json<ResetHumanPasswordInput>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state.login_limiter.check(format!(
        "password-reset-confirm:{}",
        hash_secret(&input.token)
    ))?;
    state.platform.reset_human_password(input)?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub(super) async fn deliver_account_token(
    state: &AppState,
    email: &str,
    template: &'static str,
    token: &str,
    expires_at: chrono::DateTime<chrono::Utc>,
) -> bool {
    let action_path = match template {
        "email_verification" => "/?verify_email_token=",
        "password_reset" => "/?password_reset_token=",
        _ => "/?account_token=",
    };
    let action_url = format!(
        "{}{}{}",
        state.public_base_url.trim_end_matches('/'),
        action_path,
        token
    );

    let Some(webhook_url) = state.email_delivery_webhook_url.as_deref() else {
        if state.enable_dev_endpoints {
            tracing::warn!(
                email,
                template,
                action_url,
                "development account email token"
            );
        }
        return false;
    };
    let payload = EmailDeliveryPayload {
        to: email.to_string(),
        template,
        action_url,
        expires_at,
    };
    match state
        .http_client
        .post(webhook_url)
        .json(&payload)
        .send()
        .await
    {
        Ok(response) if response.status().is_success() => true,
        Ok(response) => {
            tracing::error!(status = %response.status(), template, "email delivery webhook rejected request");
            false
        }
        Err(error) => {
            tracing::error!(%error, template, "email delivery webhook failed");
            false
        }
    }
}

pub(super) fn resolve_web_dist_dir() -> PathBuf {
    if let Ok(value) = std::env::var("WEB_DIST_DIR") {
        return PathBuf::from(value);
    }

    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../web/dist")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../web/dist"))
}
