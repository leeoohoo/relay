use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use ai_chat_shared::{AppError, AppResult};

pub(super) fn absolute_path(path: PathBuf) -> AppResult<PathBuf> {
    if path.is_absolute() {
        return Ok(path);
    }
    std::env::current_dir()
        .map(|current_dir| current_dir.join(path))
        .map_err(file_error)
}

pub(super) fn validate_project_root(raw: &str, allowed_roots: &[PathBuf]) -> AppResult<PathBuf> {
    let path = PathBuf::from(raw);
    if !path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir | std::path::Component::CurDir
            )
        })
    {
        return Err(AppError::Validation(
            "project host_local_path must be an absolute normalized path".into(),
        ));
    }
    if path.parent().is_none()
        || std::env::var("HOME")
            .ok()
            .filter(|home| !home.trim().is_empty())
            .is_some_and(|home| Path::new(&home) == path)
    {
        return Err(AppError::Validation(
            "project host_local_path cannot be the filesystem root or Trigger user's home".into(),
        ));
    }
    if !allowed_roots.is_empty()
        && !allowed_roots
            .iter()
            .any(|root| path.starts_with(root) && path != *root)
    {
        return Err(AppError::Validation(
            "project host_local_path is outside AGENT_TRIGGER_ALLOWED_LOCAL_ROOTS".into(),
        ));
    }
    fs::create_dir_all(&path).map_err(file_error)?;
    let canonical = fs::canonicalize(&path).map_err(file_error)?;
    if canonical.parent().is_none()
        || std::env::var("HOME")
            .ok()
            .and_then(|home| fs::canonicalize(home).ok())
            .is_some_and(|home| canonical == home)
    {
        return Err(AppError::Validation(
            "canonical project host_local_path is too broad".into(),
        ));
    }
    if !allowed_roots.is_empty() {
        let mut allowed = false;
        for root in allowed_roots {
            let canonical_root = fs::canonicalize(root).map_err(|error| {
                AppError::Validation(format!(
                    "failed to resolve allowed local root {}: {error}",
                    root.display()
                ))
            })?;
            if canonical.starts_with(&canonical_root) && canonical != canonical_root {
                allowed = true;
                break;
            }
        }
        if !allowed {
            return Err(AppError::Validation(
                "canonical project host_local_path escapes AGENT_TRIGGER_ALLOWED_LOCAL_ROOTS"
                    .into(),
            ));
        }
    }
    Ok(canonical)
}

pub(super) fn validate_profile_name(value: &str) -> AppResult<()> {
    if value.is_empty()
        || value.chars().count() > 64
        || !value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        })
    {
        return Err(AppError::Validation(
            "Git auth profile name contains unsupported characters".into(),
        ));
    }
    Ok(())
}

pub(super) fn validate_profile_environment(environment: &HashMap<String, String>) -> AppResult<()> {
    const ALLOWED: &[&str] = &[
        "GIT_SSH_COMMAND",
        "GIT_ASKPASS",
        "GIT_CONFIG_GLOBAL",
        "GIT_CONFIG_NOSYSTEM",
        "SSH_AUTH_SOCK",
    ];
    for (key, value) in environment {
        if !ALLOWED.contains(&key.as_str()) {
            return Err(AppError::Validation(format!(
                "Git auth profile environment key {key} is not allowed"
            )));
        }
        if value.is_empty() || value.chars().any(char::is_control) {
            return Err(AppError::Validation(format!(
                "Git auth profile environment value for {key} is invalid"
            )));
        }
    }
    Ok(())
}

pub(super) fn sanitize_branch_component(value: &str) -> String {
    let mut normalized = value
        .trim()
        .trim_start_matches('@')
        .to_ascii_lowercase()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    while normalized.contains("--") {
        normalized = normalized.replace("--", "-");
    }
    let normalized = normalized.trim_matches('-');
    if normalized.is_empty() {
        "agent".into()
    } else {
        normalized.chars().take(48).collect()
    }
}

pub(super) fn path_string(path: &Path) -> AppResult<String> {
    path.to_str()
        .map(str::to_string)
        .ok_or_else(|| AppError::Validation("Git workspace path must be valid UTF-8".into()))
}

pub(super) fn file_error(error: std::io::Error) -> AppError {
    AppError::Validation(format!("Git workspace filesystem error: {error}"))
}
