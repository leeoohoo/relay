use super::*;

pub(crate) fn default_company_governance_policy_settings() -> CompanyGovernancePolicySettings {
    CompanyGovernancePolicySettings {
        agent_staff_limit: 100,
        delegated_agent_hiring_enabled: true,
        delegated_agent_suspension_enabled: true,
        delegated_agent_termination_enabled: true,
        max_active_projects: 1_000,
        max_project_members: 50,
        daily_delegated_hire_limit: 20,
        daily_delegated_suspension_limit: 50,
        daily_delegated_termination_limit: 20,
        managed_workspace_root: None,
        skill_language: ai_chat_domain::company::COMPANY_SKILL_LANGUAGE_ZH_CN.into(),
    }
}

pub(crate) fn default_company_governance_policy_view(
    company_id: Uuid,
) -> CompanyGovernancePolicyView {
    CompanyGovernancePolicyView {
        company_id,
        configured: false,
        active_version: None,
        effective_settings: default_company_governance_policy_settings(),
        versions: Vec::new(),
    }
}

pub(crate) fn validate_company_governance_policy_settings(
    settings: &CompanyGovernancePolicySettings,
) -> AppResult<()> {
    if !matches!(
        settings.skill_language.as_str(),
        ai_chat_domain::company::COMPANY_SKILL_LANGUAGE_ZH_CN
            | ai_chat_domain::company::COMPANY_SKILL_LANGUAGE_EN
    ) {
        return Err(AppError::Validation(
            "company governance skill_language must be zh-CN or en".into(),
        ));
    }
    if !(1..=1_000).contains(&settings.agent_staff_limit) {
        return Err(AppError::Validation(
            "company governance agent_staff_limit must be between 1 and 1000".into(),
        ));
    }
    if !(1..=1_000).contains(&settings.max_active_projects) {
        return Err(AppError::Validation(
            "company governance max_active_projects must be between 1 and 1000".into(),
        ));
    }
    if !(1..=200).contains(&settings.max_project_members) {
        return Err(AppError::Validation(
            "company governance max_project_members must be between 1 and 200".into(),
        ));
    }
    if !(0..=1_000).contains(&settings.daily_delegated_hire_limit) {
        return Err(AppError::Validation(
            "company governance daily_delegated_hire_limit must be between 0 and 1000".into(),
        ));
    }
    if !(0..=1_000).contains(&settings.daily_delegated_suspension_limit) {
        return Err(AppError::Validation(
            "company governance daily_delegated_suspension_limit must be between 0 and 1000".into(),
        ));
    }
    if !(0..=1_000).contains(&settings.daily_delegated_termination_limit) {
        return Err(AppError::Validation(
            "company governance daily_delegated_termination_limit must be between 0 and 1000"
                .into(),
        ));
    }
    Ok(())
}

pub(crate) fn normalize_handle(raw: &str) -> String {
    raw.trim().trim_start_matches('@').to_lowercase()
}

pub(crate) fn normalize_company_slug(raw: &str) -> AppResult<String> {
    let mut slug = String::new();
    let mut previous_was_separator = false;
    for character in raw.trim().chars() {
        if character.is_alphanumeric() {
            for lower in character.to_lowercase() {
                slug.push(lower);
            }
            previous_was_separator = false;
        } else if matches!(character, '-' | '_' | ' ' | '\t') && !previous_was_separator {
            slug.push('-');
            previous_was_separator = true;
        }
    }
    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() || slug.chars().count() > 64 {
        return Err(AppError::Validation(
            "company slug must contain 1 to 64 letters, numbers, or hyphens".into(),
        ));
    }
    Ok(slug)
}

pub(crate) fn sanitize_optional_note(note: Option<String>) -> Option<String> {
    note.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

pub(crate) fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|item| {
        let trimmed = item.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

pub(crate) fn normalize_project_rule_content(content: String) -> AppResult<String> {
    let content = content.trim().to_string();
    if content.chars().count() > 50_000 {
        return Err(AppError::Validation(
            "project rule content must not exceed 50000 characters".into(),
        ));
    }
    Ok(content)
}

pub(crate) fn normalize_required_project_asset_text(
    value: String,
    max_characters: usize,
    field: &str,
) -> AppResult<String> {
    let value = value.trim().to_string();
    if value.is_empty() || value.chars().count() > max_characters {
        return Err(AppError::Validation(format!(
            "{field} must contain 1 to {max_characters} characters"
        )));
    }
    Ok(value)
}
