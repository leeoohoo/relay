use super::*;

pub(crate) fn normalize_staffing_text(
    value: Option<String>,
    max_chars: usize,
    field_name: &str,
) -> AppResult<String> {
    let normalized = normalize_optional_text(value).unwrap_or_default();
    if normalized.chars().count() > max_chars {
        return Err(AppError::Validation(format!(
            "{field_name} must contain at most {max_chars} characters"
        )));
    }
    Ok(normalized)
}

pub(crate) fn normalize_optional_idempotency_key(
    value: Option<String>,
) -> AppResult<Option<String>> {
    let normalized = normalize_optional_text(value);
    if normalized
        .as_ref()
        .is_some_and(|value| value.chars().count() > 200)
    {
        return Err(AppError::Validation(
            "idempotency_key must contain at most 200 characters".into(),
        ));
    }
    Ok(normalized)
}

pub(crate) fn normalize_message_page_limit(limit: usize) -> usize {
    limit.clamp(1, MAX_MESSAGE_PAGE_LIMIT)
}

pub(crate) fn org_unit_is_within_scope(
    org_units: &[OrgUnit],
    scope_org_unit_id: Uuid,
    target_org_unit_id: Uuid,
) -> bool {
    let parents = org_units
        .iter()
        .map(|org_unit| (org_unit.id, org_unit.parent_org_unit_id))
        .collect::<HashMap<_, _>>();
    let mut current = Some(target_org_unit_id);
    let mut visited = HashSet::new();
    while let Some(org_unit_id) = current {
        if org_unit_id == scope_org_unit_id {
            return true;
        }
        if !visited.insert(org_unit_id) {
            return false;
        }
        current = parents.get(&org_unit_id).copied().flatten();
    }
    false
}

pub(crate) fn company_agent_role_rank(role_key: &str) -> u8 {
    match role_key {
        COMPANY_AGENT_ROLE_MANAGER => 2,
        _ => 1,
    }
}

pub(crate) fn payload_uuid_field(payload: &serde_json::Value, key: &str) -> AppResult<Uuid> {
    let value = payload
        .get(key)
        .and_then(|value| value.as_str())
        .ok_or_else(|| AppError::Validation(format!("missing payload field: {key}")))?;
    Uuid::parse_str(value)
        .map_err(|error| AppError::Validation(format!("invalid payload uuid for {key}: {error}")))
}

pub(crate) fn payload_uuid_field_optional(payload: &serde_json::Value, key: &str) -> Option<Uuid> {
    payload
        .get(key)
        .and_then(|value| value.as_str())
        .and_then(|value| Uuid::parse_str(value).ok())
}

pub(crate) fn company_group_context_matches(
    context: &ConversationContext,
    company_id: Uuid,
) -> bool {
    context.company_id == Some(company_id)
        && matches!(
            context.context_type.as_str(),
            CONVERSATION_CONTEXT_COMPANY_ALL
                | CONVERSATION_CONTEXT_COMPANY_GROUP
                | CONVERSATION_CONTEXT_PROJECT_GROUP
        )
        && context.visibility == "members"
}

pub(crate) fn payload_string_field(payload: &serde_json::Value, key: &str) -> Option<String> {
    payload
        .get(key)
        .and_then(|value| value.as_str())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}
