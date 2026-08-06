use ai_chat_shared::{AppError, AppResult};

use super::*;

pub(super) fn validate_profile_name(value: &str) -> AppResult<()> {
    if value.trim() != value
        || value.is_empty()
        || value.chars().count() > 80
        || value.chars().any(char::is_control)
    {
        return Err(AppError::Validation(
            "Codex authentication profile name must contain 1 to 80 safe characters".into(),
        ));
    }
    Ok(())
}

pub(super) fn validate_api_key(value: &str) -> AppResult<()> {
    if value.trim() != value
        || value.chars().count() < 20
        || value.chars().count() > 1_024
        || value.chars().any(char::is_whitespace)
        || value.chars().any(char::is_control)
    {
        return Err(AppError::Validation("OpenAI API key is invalid".into()));
    }
    Ok(())
}

pub(super) fn validate_company_cli_settings(settings: &CompanyCodexCliSettings) -> AppResult<()> {
    if settings.model.as_ref().is_some_and(|value| {
        value.trim() != value
            || value.is_empty()
            || value.chars().count() > 128
            || value.chars().any(char::is_control)
    }) {
        return Err(AppError::Validation(
            "Codex CLI model must contain at most 128 safe characters".into(),
        ));
    }
    if settings.reasoning_effort.as_deref().is_some_and(|value| {
        !matches!(
            value,
            "none" | "minimal" | "low" | "medium" | "high" | "xhigh" | "max" | "ultra"
        )
    }) {
        return Err(AppError::Validation(
            "unsupported Codex reasoning effort".into(),
        ));
    }
    if !matches!(
        settings.reasoning_summary.as_str(),
        "auto" | "concise" | "detailed" | "none"
    ) {
        return Err(AppError::Validation(
            "unsupported Codex reasoning summary".into(),
        ));
    }
    if settings
        .verbosity
        .as_deref()
        .is_some_and(|value| !matches!(value, "low" | "medium" | "high"))
    {
        return Err(AppError::Validation(
            "unsupported Codex model verbosity".into(),
        ));
    }
    if settings
        .personality
        .as_deref()
        .is_some_and(|value| !matches!(value, "none" | "friendly" | "pragmatic"))
    {
        return Err(AppError::Validation("unsupported Codex personality".into()));
    }
    if settings
        .service_tier
        .as_deref()
        .is_some_and(|value| value != "fast")
    {
        return Err(AppError::Validation(
            "unsupported Codex service tier".into(),
        ));
    }
    if !matches!(settings.approval_policy.as_str(), "never" | "on-request") {
        return Err(AppError::Validation(
            "unsupported Codex approval policy".into(),
        ));
    }
    if !matches!(
        settings.sandbox_mode.as_str(),
        "read_only" | "workspace_write"
    ) {
        return Err(AppError::Validation(
            "unsupported Codex sandbox mode".into(),
        ));
    }
    if !matches!(
        settings.web_search.as_str(),
        "disabled" | "cached" | "indexed" | "live"
    ) {
        return Err(AppError::Validation(
            "unsupported Codex web search mode".into(),
        ));
    }
    Ok(())
}

pub(super) fn validate_agent_trigger_batch_size(value: usize) -> AppResult<usize> {
    if !(AGENT_TRIGGER_BATCH_SIZE_MIN..=AGENT_TRIGGER_BATCH_SIZE_MAX).contains(&value) {
        return Err(AppError::Validation(format!(
            "Agent Trigger batch size must be between {AGENT_TRIGGER_BATCH_SIZE_MIN} and {AGENT_TRIGGER_BATCH_SIZE_MAX}"
        )));
    }
    Ok(value)
}

pub(super) fn normalize_codex_base_url(value: Option<&str>) -> AppResult<String> {
    let value = value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(CODEX_DEFAULT_OPENAI_BASE_URL);
    if value.len() > 2_048 || value.chars().any(char::is_control) {
        return Err(AppError::Validation("Codex Base URL is invalid".into()));
    }
    let parsed = reqwest::Url::parse(value)
        .map_err(|_| AppError::Validation("Codex Base URL is invalid".into()))?;
    if !matches!(parsed.scheme(), "http" | "https")
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err(AppError::Validation("Codex Base URL is invalid".into()));
    }
    Ok(value.trim_end_matches('/').to_string())
}

pub(super) fn versions_show_update(installed: Option<&str>, latest: Option<&str>) -> bool {
    match (
        installed.and_then(version_tuple),
        latest.and_then(version_tuple),
    ) {
        (Some(installed), Some(latest)) => latest > installed,
        _ => false,
    }
}

pub(super) fn version_tuple(value: &str) -> Option<(u64, u64, u64)> {
    let value = value
        .split_whitespace()
        .last()
        .unwrap_or(value)
        .trim_start_matches('v');
    let core = value.split(['-', '+']).next()?;
    let mut parts = core.split('.');
    Some((
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
    ))
}
