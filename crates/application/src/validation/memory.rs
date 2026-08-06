use super::*;

pub(crate) fn normalize_agent_memory_tier(value: &str) -> AppResult<String> {
    let value = value.trim().to_lowercase();
    if matches!(
        value.as_str(),
        AGENT_MEMORY_TIER_SHORT_TERM | AGENT_MEMORY_TIER_LONG_TERM
    ) {
        Ok(value)
    } else {
        Err(AppError::Validation(
            "memory_tier must be short_term or long_term".into(),
        ))
    }
}

pub(crate) fn normalize_agent_memory_type(value: &str) -> AppResult<String> {
    let value = value.trim().to_lowercase();
    if matches!(
        value.as_str(),
        "fact" | "decision" | "lesson" | "preference" | "procedure" | "relationship" | "handoff"
    ) {
        Ok(value)
    } else {
        Err(AppError::Validation(
            "memory_type must be fact, decision, lesson, preference, procedure, relationship, or handoff"
                .into(),
        ))
    }
}

pub(crate) fn normalize_agent_memory_status(value: &str) -> AppResult<String> {
    let value = value.trim().to_lowercase();
    if matches!(
        value.as_str(),
        AGENT_MEMORY_STATUS_DRAFT
            | AGENT_MEMORY_STATUS_ACTIVE
            | AGENT_MEMORY_STATUS_ARCHIVED
            | AGENT_MEMORY_STATUS_SUPERSEDED
    ) {
        Ok(value)
    } else {
        Err(AppError::Validation(
            "memory status must be draft, active, archived, or superseded".into(),
        ))
    }
}

pub(crate) fn normalize_agent_memory_topic_key(value: String) -> AppResult<String> {
    let mut normalized = String::new();
    let mut previous_separator = false;
    for character in value.trim().to_lowercase().chars() {
        if character.is_ascii_alphanumeric() {
            normalized.push(character);
            previous_separator = false;
        } else if matches!(character, '-' | '_' | '.' | ':' | ' ' | '\t') && !previous_separator {
            normalized.push('-');
            previous_separator = true;
        }
    }
    let normalized = normalized.trim_matches('-').to_string();
    if !(3..=120).contains(&normalized.chars().count()) {
        return Err(AppError::Validation(
            "memory topic_key must contain 3 to 120 ASCII letters, numbers, or separators".into(),
        ));
    }
    Ok(normalized)
}

pub(crate) fn normalize_agent_memory_text(
    value: String,
    max_characters: usize,
    field: &str,
    min_characters: usize,
) -> AppResult<String> {
    let value = value.trim().to_string();
    if !(min_characters..=max_characters).contains(&value.chars().count()) {
        return Err(AppError::Validation(format!(
            "{field} must contain {min_characters} to {max_characters} characters"
        )));
    }
    if value.chars().any(|character| character == '\0') {
        return Err(AppError::Validation(format!(
            "{field} contains an unsupported control character"
        )));
    }
    Ok(value)
}

pub(crate) fn normalize_agent_memory_summary(value: String) -> AppResult<String> {
    let value = normalize_agent_memory_text(value, 2_000, "memory summary", 10)?;
    let lowered = value.to_lowercase();
    if [
        "-----begin private key-----",
        "github_pat_",
        "ghp_",
        "agk_",
        "openai_api_key=",
        "password=",
        "密码：",
    ]
    .iter()
    .any(|marker| lowered.contains(marker))
    {
        return Err(AppError::Validation(
            "memory summary appears to contain a credential or secret; store conclusions, never secrets"
                .into(),
        ));
    }
    Ok(value)
}

pub(crate) fn normalize_agent_memory_optional_text(
    value: String,
    max_characters: usize,
    field: &str,
) -> AppResult<String> {
    let value = value.trim().to_string();
    if value.chars().count() > max_characters {
        return Err(AppError::Validation(format!(
            "{field} must not exceed {max_characters} characters"
        )));
    }
    Ok(value)
}

pub(crate) fn normalize_agent_memory_tags(tags: Vec<String>) -> AppResult<Vec<String>> {
    if tags.len() > 12 {
        return Err(AppError::Validation(
            "an Agent memory supports at most 12 tags".into(),
        ));
    }
    let mut normalized = Vec::new();
    let mut seen = HashSet::new();
    for tag in tags {
        let tag = tag.trim().to_lowercase();
        if tag.is_empty() || tag.chars().count() > 40 {
            return Err(AppError::Validation(
                "memory tags must contain 1 to 40 characters".into(),
            ));
        }
        if seen.insert(tag.clone()) {
            normalized.push(tag);
        }
    }
    Ok(normalized)
}

pub(crate) fn validate_agent_memory_source_refs(
    source_refs: &[AgentMemorySourceRef],
) -> AppResult<()> {
    if source_refs.len() > 20 {
        return Err(AppError::Validation(
            "an Agent memory supports at most 20 source references".into(),
        ));
    }
    for source in source_refs {
        if !matches!(
            source.source_type.as_str(),
            "message" | "task" | "run" | "project" | "human" | "manual"
        ) {
            return Err(AppError::Validation(
                "memory source_type must be message, task, run, project, human, or manual".into(),
            ));
        }
        if source
            .label
            .as_ref()
            .is_some_and(|label| label.chars().count() > 200)
        {
            return Err(AppError::Validation(
                "memory source label must not exceed 200 characters".into(),
            ));
        }
    }
    Ok(())
}
