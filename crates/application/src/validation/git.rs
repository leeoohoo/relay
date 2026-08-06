use super::*;

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_project_git_config(
    project_id: Uuid,
    remote_url: String,
    host_local_path: String,
    default_branch: Option<String>,
    auth_profile: Option<String>,
    allow_agent_push: bool,
    branch_prefix: Option<String>,
    created_by_agent_id: Option<Uuid>,
    created_by_human_user_id: Option<Uuid>,
    updated_by_agent_id: Option<Uuid>,
    updated_by_human_user_id: Option<Uuid>,
    created_at: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
) -> AppResult<CompanyProjectGitConfig> {
    let (remote_url, git_host) = validate_git_remote_url(&remote_url)?;
    let host_local_path = validate_host_local_path(&host_local_path)?;
    let default_branch = validate_git_branch_name(
        default_branch.as_deref().unwrap_or("main"),
        "git_default_branch",
    )?;
    let auth_profile = auth_profile
        .map(|profile| validate_git_auth_profile(&profile))
        .transpose()?;
    let branch_prefix = validate_git_branch_prefix(branch_prefix.as_deref().unwrap_or("relay/"))?;
    Ok(CompanyProjectGitConfig {
        project_id,
        remote_url,
        default_branch,
        git_host,
        host_local_path,
        auth_profile,
        allow_agent_push,
        branch_prefix,
        created_by_agent_id,
        created_by_human_user_id,
        updated_by_agent_id,
        updated_by_human_user_id,
        created_at: created_at.unwrap_or(now),
        updated_at: now,
    })
}

pub(crate) fn resolve_project_host_local_path(
    project_id: Uuid,
    requested: Option<String>,
    existing: Option<&CompanyProjectGitConfig>,
) -> AppResult<String> {
    if let Some(requested) = requested.filter(|value| !value.trim().is_empty()) {
        return validate_host_local_path(&requested);
    }
    if let Some(existing) = existing {
        return validate_host_local_path(&existing.host_local_path);
    }

    let managed_root = std::env::var("AGENT_TRIGGER_MANAGED_PROJECTS_ROOT")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "/tmp/relay-agent-trigger/projects".to_string());
    let managed_root = validate_managed_projects_root(&managed_root)?;
    validate_host_local_path(&managed_root.join(project_id.to_string()).to_string_lossy())
}

pub(crate) fn validate_managed_projects_root(raw: &str) -> AppResult<std::path::PathBuf> {
    let value = raw.trim();
    let path = Path::new(value);
    if value.is_empty()
        || value != raw
        || value.chars().count() > 4_096
        || value.chars().any(char::is_control)
        || !path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::CurDir))
        || path.parent().is_none()
    {
        return Err(AppError::Validation(
            "AGENT_TRIGGER_MANAGED_PROJECTS_ROOT must be an absolute normalized non-root path"
                .into(),
        ));
    }
    Ok(path.to_path_buf())
}

pub(crate) fn validate_host_local_path(raw: &str) -> AppResult<String> {
    let value = raw.trim();
    if value.is_empty()
        || value != raw
        || value.chars().count() > 4_096
        || value.chars().any(char::is_control)
    {
        return Err(AppError::Validation(
            "host_local_path must contain 1 to 4096 non-control characters without surrounding whitespace"
                .into(),
        ));
    }
    let path = Path::new(value);
    if !path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::CurDir))
    {
        return Err(AppError::Validation(
            "host_local_path must be an absolute normalized path without . or .. components".into(),
        ));
    }
    if path.parent().is_none()
        || std::env::var("HOME")
            .ok()
            .filter(|home| !home.trim().is_empty())
            .is_some_and(|home| Path::new(&home) == path)
    {
        return Err(AppError::Validation(
            "host_local_path cannot be the filesystem root or the Trigger user's home directory"
                .into(),
        ));
    }
    let allowed_roots = std::env::var("AGENT_TRIGGER_ALLOWED_LOCAL_ROOTS").unwrap_or_default();
    let allowed_roots = allowed_roots
        .split(',')
        .map(str::trim)
        .filter(|root| !root.is_empty())
        .collect::<Vec<_>>();
    if !allowed_roots.is_empty()
        && !allowed_roots.iter().any(|root| {
            let root = Path::new(root);
            root.is_absolute() && path.starts_with(root) && path != root
        })
    {
        return Err(AppError::Validation(
            "host_local_path is outside AGENT_TRIGGER_ALLOWED_LOCAL_ROOTS".into(),
        ));
    }
    Ok(value.to_string())
}

pub(crate) fn validate_git_remote_url(raw: &str) -> AppResult<(String, String)> {
    let remote_url = raw.trim();
    if remote_url.is_empty() || remote_url.chars().count() > 2048 {
        return Err(AppError::Validation(
            "git_remote_url must contain 1 to 2048 characters".into(),
        ));
    }
    if remote_url != raw
        || remote_url.starts_with('-')
        || remote_url.chars().any(|character| {
            character.is_control() || character.is_whitespace() || character == '\\'
        })
    {
        return Err(AppError::Validation(
            "git_remote_url contains unsupported whitespace or control characters".into(),
        ));
    }

    let git_host = if remote_url.contains("://") {
        let parsed = Url::parse(remote_url)
            .map_err(|_| AppError::Validation("git_remote_url is not a valid URL".into()))?;
        if !matches!(parsed.scheme(), "https" | "ssh") {
            return Err(AppError::Validation(
                "git_remote_url must use https or ssh".into(),
            ));
        }
        if parsed.password().is_some()
            || (parsed.scheme() == "https" && !parsed.username().is_empty())
        {
            return Err(AppError::Validation(
                "git_remote_url must not contain embedded credentials".into(),
            ));
        }
        if parsed.query().is_some() || parsed.fragment().is_some() {
            return Err(AppError::Validation(
                "git_remote_url must not contain a query or fragment".into(),
            ));
        }
        if parsed.path().is_empty() || parsed.path() == "/" {
            return Err(AppError::Validation(
                "git_remote_url must include a repository path".into(),
            ));
        }
        parsed
            .host_str()
            .map(str::to_lowercase)
            .ok_or_else(|| AppError::Validation("git_remote_url must include a host".into()))?
    } else {
        let (identity, repository_path) = remote_url.rsplit_once(':').ok_or_else(|| {
            AppError::Validation(
                "git_remote_url must be https, ssh, or user@host:path syntax".into(),
            )
        })?;
        let (username, host) = identity.split_once('@').ok_or_else(|| {
            AppError::Validation("scp-style git_remote_url must use user@host:path syntax".into())
        })?;
        if username.is_empty()
            || host.is_empty()
            || repository_path.is_empty()
            || repository_path.starts_with('-')
            || !username.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
            })
            || !host.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '-' | '.')
            })
        {
            return Err(AppError::Validation(
                "scp-style git_remote_url is invalid".into(),
            ));
        }
        host.to_lowercase()
    };
    ensure_git_host_allowed(&git_host)?;
    Ok((remote_url.to_string(), git_host))
}

pub(crate) fn ensure_git_host_allowed(git_host: &str) -> AppResult<()> {
    let allowed_hosts = std::env::var("AGENT_TRIGGER_ALLOWED_GIT_HOSTS").unwrap_or_default();
    let allowed_hosts = allowed_hosts
        .split(',')
        .map(str::trim)
        .filter(|host| !host.is_empty())
        .collect::<Vec<_>>();
    if allowed_hosts.is_empty()
        || allowed_hosts.iter().any(|allowed| {
            allowed.eq_ignore_ascii_case(git_host)
                || allowed
                    .strip_prefix("*.")
                    .is_some_and(|suffix| git_host.ends_with(&format!(".{suffix}")))
        })
    {
        return Ok(());
    }
    Err(AppError::Validation(format!(
        "Git host {git_host} is not present in AGENT_TRIGGER_ALLOWED_GIT_HOSTS"
    )))
}

pub(crate) fn validate_git_branch_name(raw: &str, field_name: &str) -> AppResult<String> {
    let value = raw.trim();
    let invalid = value.is_empty()
        || value.chars().count() > 255
        || value != raw
        || value.starts_with('-')
        || value.starts_with('/')
        || value.ends_with('/')
        || value.ends_with('.')
        || value.ends_with(".lock")
        || value.contains("..")
        || value.contains("//")
        || value.contains("@{")
        || value.chars().any(|character| {
            character.is_control()
                || character.is_whitespace()
                || matches!(character, '~' | '^' | ':' | '?' | '*' | '[' | '\\')
        })
        || value
            .split('/')
            .any(|segment| segment.is_empty() || segment.starts_with('.'));
    if invalid {
        return Err(AppError::Validation(format!(
            "{field_name} is not a valid Git branch name"
        )));
    }
    Ok(value.to_string())
}

pub(crate) fn validate_git_branch_prefix(raw: &str) -> AppResult<String> {
    let value = raw.trim();
    if !value.ends_with('/') || value.chars().count() > 80 {
        return Err(AppError::Validation(
            "branch_prefix must be a valid Git branch prefix ending in /".into(),
        ));
    }
    let branch = value.trim_end_matches('/');
    validate_git_branch_name(branch, "branch_prefix")?;
    Ok(value.to_string())
}

pub(crate) fn validate_git_auth_profile(raw: &str) -> AppResult<String> {
    let value = raw.trim();
    if value.is_empty()
        || value.chars().count() > 64
        || value != raw
        || !value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        })
    {
        return Err(AppError::Validation(
            "auth_profile must be a safe credential profile label".into(),
        ));
    }
    Ok(value.to_string())
}
