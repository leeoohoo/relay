use super::*;

pub(crate) fn normalize_agent_memory_scope(value: &str) -> AppResult<String> {
    let value = value.trim().to_lowercase();
    if matches!(
        value.as_str(),
        AGENT_MEMORY_SCOPE_AGENT
            | AGENT_MEMORY_SCOPE_CONTROL
            | AGENT_MEMORY_SCOPE_PROJECT
            | AGENT_MEMORY_SCOPE_SESSION
    ) {
        Ok(value)
    } else {
        Err(AppError::Validation(
            "memory scope must be agent, control, project, or session".into(),
        ))
    }
}

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

pub(crate) struct AgentMemoryClassification {
    pub(crate) memory_tier: String,
    pub(crate) reason: String,
    pub(crate) estimated_ttl_days: Option<i32>,
    pub(crate) injection_cost_chars: i32,
}

pub(crate) fn classify_agent_memory(
    requested_tier: &str,
    memory_type: &str,
    title: &str,
    summary: &str,
    when_to_use: &str,
) -> AgentMemoryClassification {
    let text = format!("{title}\n{summary}\n{when_to_use}").to_lowercase();
    let transient_markers = [
        "当前任务",
        "当前阶段",
        "目前",
        "今天",
        "本次",
        "临时",
        "进行中",
        "等待审批",
        "等待环境",
        "等待依赖",
        "已完成",
        "本轮",
        "current task",
        "current phase",
        "today",
        "temporary",
        "in progress",
        "waiting for",
        "this run",
        "this attempt",
        "http://",
        "https://",
        "commit ",
        "revision ",
        "branch ",
    ];
    let has_iso_date = text.as_bytes().windows(10).any(|window| {
        window[0..4].iter().all(u8::is_ascii_digit)
            && window[4] == b'-'
            && window[5..7].iter().all(u8::is_ascii_digit)
            && window[7] == b'-'
            && window[8..10].iter().all(u8::is_ascii_digit)
    });
    let transient = has_iso_date || transient_markers.iter().any(|marker| text.contains(marker));
    let memory_tier = if requested_tier == AGENT_MEMORY_TIER_LONG_TERM && transient {
        AGENT_MEMORY_TIER_SHORT_TERM
    } else {
        requested_tier
    };
    let reason = if requested_tier == AGENT_MEMORY_TIER_LONG_TERM && transient {
        "downgraded to short_term because the content contains temporary status, runtime coordinates, or handoff context"
    } else if memory_tier == AGENT_MEMORY_TIER_LONG_TERM {
        "accepted as long_term because the content describes reusable guidance without temporary execution state"
    } else {
        "classified as short_term because it is intended for on-demand or time-bounded context"
    };
    AgentMemoryClassification {
        memory_tier: memory_tier.into(),
        reason: reason.into(),
        estimated_ttl_days: (memory_tier == AGENT_MEMORY_TIER_SHORT_TERM)
            .then_some(if memory_type == "handoff" { 14 } else { 30 }),
        injection_cost_chars: (title.chars().count()
            + summary.chars().count()
            + when_to_use.chars().count()
            + 96) as i32,
    }
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
    for (index, source) in source_refs.iter().enumerate() {
        if !matches!(
            source.source_type.as_str(),
            "message" | "task" | "run" | "project" | "human" | "git_commit" | "manual"
        ) {
            return Err(AppError::Validation(format!(
                "memory source_refs[{index}].source_type must be message, task, run, project, human, git_commit, or manual"
            )));
        }
        let source_id = source.source_id.trim();
        if matches!(
            source.source_type.as_str(),
            "message" | "task" | "run" | "project" | "human"
        ) {
            Uuid::parse_str(source_id).map_err(|_| {
                AppError::Validation(format!(
                    "memory source_refs[{index}].source_id must be a canonical Relay UUID when source_type is {}",
                    source.source_type
                ))
            })?;
        } else if source.source_type == "git_commit" {
            if !(7..=64).contains(&source_id.len())
                || !source_id.bytes().all(|byte| byte.is_ascii_hexdigit())
            {
                return Err(AppError::Validation(format!(
                    "memory source_refs[{index}].source_id must be a 7 to 64 character hexadecimal Git object ID when source_type is git_commit"
                )));
            }
        } else if source_id.is_empty()
            || source_id.chars().count() > 200
            || source_id.chars().any(char::is_control)
        {
            return Err(AppError::Validation(format!(
                "memory source_refs[{index}].source_id must contain 1 to 200 non-control characters when source_type is manual"
            )));
        }
        if source
            .label
            .as_ref()
            .is_some_and(|label| label.chars().count() > 200)
        {
            return Err(AppError::Validation(format!(
                "memory source_refs[{index}].label must not exceed 200 characters"
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_reference_validation_accepts_git_commit_sha() {
        validate_agent_memory_source_refs(&[AgentMemorySourceRef {
            source_type: "git_commit".into(),
            source_id: "7ed28bec6af9d8cbd8b438f371bd70299a0c4858".into(),
            label: Some("QA evidence commit".into()),
        }])
        .expect("a full Git commit SHA should be accepted");
    }

    #[test]
    fn source_reference_validation_names_invalid_internal_id_field() {
        let error = validate_agent_memory_source_refs(&[AgentMemorySourceRef {
            source_type: "task".into(),
            source_id: "T06-not-a-relay-uuid".into(),
            label: None,
        }])
        .expect_err("a composite task reference should be rejected");

        assert!(error.to_string().contains("source_refs[0].source_id"));
        assert!(error.to_string().contains("canonical Relay UUID"));
    }
}
