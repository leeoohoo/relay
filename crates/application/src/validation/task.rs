use super::*;

pub(crate) fn project_task_dependency_would_cycle(
    dependencies: &[CompanyProjectTaskDependency],
    task_id: Uuid,
    depends_on_task_id: Uuid,
) -> bool {
    let mut stack = vec![depends_on_task_id];
    let mut visited = HashSet::new();
    while let Some(current_task_id) = stack.pop() {
        if current_task_id == task_id {
            return true;
        }
        if !visited.insert(current_task_id) {
            continue;
        }
        stack.extend(
            dependencies
                .iter()
                .filter(|dependency| dependency.task_id == current_task_id)
                .map(|dependency| dependency.depends_on_task_id),
        );
    }
    false
}

pub(crate) fn normalize_project_task_dependency_condition(
    value: Option<&str>,
) -> AppResult<String> {
    let condition = value
        .unwrap_or(PROJECT_TASK_DEPENDENCY_SUCCESS)
        .trim()
        .to_lowercase();
    if matches!(
        condition.as_str(),
        PROJECT_TASK_DEPENDENCY_SUCCESS
            | PROJECT_TASK_DEPENDENCY_COMPLETION
            | PROJECT_TASK_DEPENDENCY_FAILURE
    ) {
        Ok(condition)
    } else {
        Err(AppError::Validation(
            "task dependency condition must be success, completion, or failure".into(),
        ))
    }
}

pub(crate) fn normalize_project_status(value: &str) -> AppResult<String> {
    let status = value.trim().to_lowercase();
    if matches!(
        status.as_str(),
        PROJECT_STATUS_PLANNED
            | PROJECT_STATUS_ACTIVE
            | PROJECT_STATUS_BLOCKED
            | PROJECT_STATUS_COMPLETED
            | PROJECT_STATUS_CANCELLED
    ) {
        Ok(status)
    } else {
        Err(AppError::Validation(
            "project status must be planned, active, blocked, completed, or cancelled".into(),
        ))
    }
}

pub(crate) fn normalize_project_task_title(value: &str) -> AppResult<String> {
    let title = value.trim();
    if title.is_empty() || title.chars().count() > 200 {
        return Err(AppError::Validation(
            "project task title must contain 1 to 200 characters".into(),
        ));
    }
    Ok(title.to_string())
}

pub(crate) fn normalize_project_task_status(value: &str) -> AppResult<String> {
    let status = value.trim().to_lowercase();
    if matches!(
        status.as_str(),
        PROJECT_TASK_STATUS_TODO
            | PROJECT_TASK_STATUS_IN_PROGRESS
            | PROJECT_TASK_STATUS_BLOCKED
            | PROJECT_TASK_STATUS_DONE
            | PROJECT_TASK_STATUS_FAILED
            | PROJECT_TASK_STATUS_CANCELLED
    ) {
        Ok(status)
    } else {
        Err(AppError::Validation(
            "project task status must be todo, in_progress, blocked, done, failed, or cancelled"
                .into(),
        ))
    }
}

pub(crate) fn normalize_project_task_priority(value: Option<&str>) -> AppResult<String> {
    let priority = value
        .unwrap_or(PROJECT_TASK_PRIORITY_NORMAL)
        .trim()
        .to_lowercase();
    if matches!(
        priority.as_str(),
        PROJECT_TASK_PRIORITY_LOW
            | PROJECT_TASK_PRIORITY_NORMAL
            | PROJECT_TASK_PRIORITY_HIGH
            | PROJECT_TASK_PRIORITY_URGENT
    ) {
        Ok(priority)
    } else {
        Err(AppError::Validation(
            "project task priority must be low, normal, high, or urgent".into(),
        ))
    }
}

pub(crate) fn normalize_project_status_items(
    items: Vec<String>,
    field_name: &str,
) -> AppResult<Vec<String>> {
    if items.len() > 20 {
        return Err(AppError::Validation(format!(
            "project {field_name} supports at most 20 items"
        )));
    }
    let mut normalized = Vec::new();
    for item in items {
        let item = item.trim();
        if item.is_empty() {
            continue;
        }
        if item.chars().count() > 300 {
            return Err(AppError::Validation(format!(
                "each project {field_name} item must not exceed 300 characters"
            )));
        }
        if !normalized.iter().any(|existing| existing == item) {
            normalized.push(item.to_string());
        }
    }
    Ok(normalized)
}
