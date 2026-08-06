use super::*;

pub fn collect_codex_models(value: &Value, models: &mut BTreeMap<String, CodexModelInfo>) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect_codex_models(item, models);
            }
        }
        Value::Object(object) => {
            let is_selectable = object
                .get("visibility")
                .and_then(Value::as_str)
                .map(|visibility| visibility.eq_ignore_ascii_case("list"))
                .unwrap_or(true);
            let id = object
                .get("slug")
                .or_else(|| object.get("model"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|id| !id.is_empty() && id.len() <= 128);
            if let Some(id) = id.filter(|_| is_selectable) {
                let display_name = object
                    .get("display_name")
                    .or_else(|| object.get("name"))
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|name| !name.is_empty())
                    .unwrap_or(id);
                let default_reasoning_effort = object
                    .get("default_reasoning_level")
                    .or_else(|| object.get("defaultReasoningEffort"))
                    .and_then(Value::as_str)
                    .filter(|effort| is_codex_reasoning_effort(effort))
                    .map(str::to_string);
                let mut reasoning_efforts = Vec::new();
                if let Some(values) = object
                    .get("supported_reasoning_levels")
                    .or_else(|| object.get("supportedReasoningEfforts"))
                    .and_then(Value::as_array)
                {
                    for reasoning in values {
                        let Some(effort) = reasoning
                            .get("effort")
                            .or_else(|| reasoning.get("reasoningEffort"))
                            .and_then(Value::as_str)
                            .filter(|effort| is_codex_reasoning_effort(effort))
                        else {
                            continue;
                        };
                        if reasoning_efforts
                            .iter()
                            .any(|item: &CodexModelReasoningEffort| item.effort == effort)
                        {
                            continue;
                        }
                        let description = reasoning
                            .get("description")
                            .and_then(Value::as_str)
                            .map(str::trim)
                            .filter(|description| !description.is_empty())
                            .unwrap_or(effort);
                        reasoning_efforts.push(CodexModelReasoningEffort {
                            effort: effort.to_string(),
                            description: description.to_string(),
                        });
                    }
                }
                models.insert(
                    id.to_string(),
                    CodexModelInfo {
                        id: id.to_string(),
                        display_name: display_name.to_string(),
                        default_reasoning_effort,
                        reasoning_efforts,
                    },
                );
            }
            for nested in object.values() {
                if nested.is_array() || nested.is_object() {
                    collect_codex_models(nested, models);
                }
            }
        }
        Value::String(id) if !id.trim().is_empty() && id.len() <= 128 => {
            if (id.contains("gpt-") || id.contains("codex"))
                && id.chars().all(|character| {
                    character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
                })
            {
                models.insert(
                    id.clone(),
                    CodexModelInfo {
                        id: id.clone(),
                        display_name: id.clone(),
                        default_reasoning_effort: None,
                        reasoning_efforts: Vec::new(),
                    },
                );
            }
        }
        _ => {}
    }
}

pub(super) fn is_codex_reasoning_effort(value: &str) -> bool {
    matches!(
        value,
        "none" | "minimal" | "low" | "medium" | "high" | "xhigh" | "max" | "ultra"
    )
}
