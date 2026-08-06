use super::*;

pub(crate) fn validate_codex_plugin_id(raw: Option<&str>) -> AppResult<String> {
    let value = raw.unwrap_or_default().trim();
    let Some((name, marketplace)) = value.split_once('@') else {
        return Err(AppError::Validation(
            "Codex plugin_id must use name@marketplace format".into(),
        ));
    };
    let valid_part = |part: &str| {
        !part.is_empty()
            && part.chars().count() <= 100
            && part.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
            })
    };
    if value != raw.unwrap_or_default()
        || value.chars().count() > 201
        || !valid_part(name)
        || !valid_part(marketplace)
    {
        return Err(AppError::Validation(
            "Codex plugin_id contains unsupported characters".into(),
        ));
    }
    Ok(value.to_string())
}

pub(crate) fn codex_plugin_catalog_contains(catalog: &Value, plugin_id: &str) -> bool {
    catalog.as_array().is_some_and(|plugins| {
        plugins.iter().any(|plugin| {
            plugin
                .get("pluginId")
                .or_else(|| plugin.get("plugin_id"))
                .and_then(Value::as_str)
                == Some(plugin_id)
        })
    })
}

pub(crate) fn validate_codex_profile_name(raw: &str) -> AppResult<String> {
    let value = raw.trim();
    if value.is_empty()
        || value.chars().count() > 80
        || value != raw
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err(AppError::Validation(
            "codex_profile must contain only letters, numbers, hyphens, or underscores".into(),
        ));
    }
    Ok(value.to_string())
}

pub(crate) fn validate_optional_codex_reasoning_effort(
    raw: Option<String>,
    field_name: &str,
) -> AppResult<Option<String>> {
    let Some(value) = raw.map(|value| value.trim().to_ascii_lowercase()) else {
        return Ok(None);
    };
    if value.is_empty() {
        return Ok(None);
    }
    if !matches!(
        value.as_str(),
        "minimal" | "low" | "medium" | "high" | "xhigh" | "max" | "ultra"
    ) {
        return Err(AppError::Validation(format!(
            "{field_name} must be minimal, low, medium, high, xhigh, max, or ultra"
        )));
    }
    Ok(Some(value))
}

pub(crate) fn validate_optional_codex_choice(
    raw: Option<String>,
    allowed: &[&str],
    field_name: &str,
) -> AppResult<Option<String>> {
    let Some(value) = raw.map(|value| value.trim().to_ascii_lowercase()) else {
        return Ok(None);
    };
    if value.is_empty() {
        return Ok(None);
    }
    if !allowed.contains(&value.as_str()) {
        return Err(AppError::Validation(format!(
            "{field_name} must be one of: {}",
            allowed.join(", ")
        )));
    }
    Ok(Some(value))
}

pub(crate) fn company_project_git_view(config: CompanyProjectGitConfig) -> CompanyProjectGitView {
    CompanyProjectGitView {
        remote_url: config.remote_url,
        default_branch: config.default_branch,
        git_host: config.git_host,
        push_enabled: config.allow_agent_push,
        branch_prefix: config.branch_prefix,
        auth_configured: config.auth_profile.is_some(),
    }
}

pub(crate) fn company_project_git_admin_view(
    config: CompanyProjectGitConfig,
) -> CompanyProjectGitAdminView {
    CompanyProjectGitAdminView {
        remote_url: config.remote_url,
        default_branch: config.default_branch,
        git_host: config.git_host,
        host_local_path: config.host_local_path,
        auth_profile: config.auth_profile,
        allow_agent_push: config.allow_agent_push,
        branch_prefix: config.branch_prefix,
        created_at: config.created_at,
        updated_at: config.updated_at,
    }
}

pub(crate) fn is_agent_tool_approval_status(status: &str) -> bool {
    matches!(
        status,
        AGENT_TOOL_APPROVAL_STATUS_PENDING
            | AGENT_TOOL_APPROVAL_STATUS_APPROVED
            | AGENT_TOOL_APPROVAL_STATUS_EXECUTING
            | AGENT_TOOL_APPROVAL_STATUS_EXECUTED
            | AGENT_TOOL_APPROVAL_STATUS_REJECTED
            | AGENT_TOOL_APPROVAL_STATUS_EXPIRED
            | AGENT_TOOL_APPROVAL_STATUS_FAILED
    )
}

pub(crate) fn normalize_approval_review_note(note: Option<String>) -> AppResult<String> {
    let note = note.unwrap_or_default().trim().to_string();
    if note.chars().count() > 1_000 {
        return Err(AppError::Validation(
            "approval review_note cannot exceed 1000 characters".into(),
        ));
    }
    Ok(note)
}
