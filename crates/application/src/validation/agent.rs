use super::*;

pub(crate) fn normalize_agent_collaboration_preference(value: &str) -> AppResult<String> {
    let normalized = value.trim().to_lowercase().replace(['-', ' '], "_");
    match normalized.as_str() {
        "available" | "open" | "yes" | "default" | "可协作" => {
            Ok(AGENT_COLLABORATION_PREFERENCE_AVAILABLE.into())
        }
        "low_cost_only"
        | "low_cost"
        | "lowcost"
        | "low"
        | "async_only"
        | "compact"
        | "只接低成本请求"
        | "低成本" => Ok(AGENT_COLLABORATION_PREFERENCE_LOW_COST_ONLY.into()),
        "unavailable" | "busy" | "offline" | "paused" | "pause" | "do_not_disturb"
        | "暂不接请求" | "暂停" => Ok(AGENT_COLLABORATION_PREFERENCE_UNAVAILABLE.into()),
        _ => Err(AppError::Validation(
            "collaboration_preference must be available, low_cost_only, or unavailable".into(),
        )),
    }
}

pub(crate) fn sanitize_agent_collaboration_preference(value: &str) -> String {
    normalize_agent_collaboration_preference(value)
        .unwrap_or_else(|_| AGENT_COLLABORATION_PREFERENCE_AVAILABLE.into())
}

pub(crate) fn normalize_work_profile_items(
    field: &str,
    values: Vec<String>,
    max_items: usize,
    max_chars: usize,
) -> AppResult<Vec<String>> {
    if values.len() > max_items {
        return Err(AppError::Validation(format!(
            "{field} must contain at most {max_items} items"
        )));
    }

    let mut seen = HashSet::new();
    let mut normalized = Vec::new();
    for value in values {
        let value = value.trim();
        if value.is_empty() {
            continue;
        }
        if value.chars().count() > max_chars {
            return Err(AppError::Validation(format!(
                "each {field} item must contain at most {max_chars} characters"
            )));
        }
        let dedup_key = value.to_lowercase();
        if seen.insert(dedup_key) {
            normalized.push(value.to_string());
        }
    }
    Ok(normalized)
}
