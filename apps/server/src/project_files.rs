use super::*;

pub(super) fn validate_company_workspace_root(raw: String) -> AppResult<String> {
    let value = raw.trim();
    let path = FsPath::new(value);
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
            "managed_workspace_root must be an absolute normalized non-root path".into(),
        ));
    }
    Ok(value.to_string())
}

pub(super) fn resolve_company_workspace_root(
    configured: Option<&str>,
    company_id: Uuid,
) -> AppResult<PathBuf> {
    if let Some(configured) = configured.filter(|value| !value.trim().is_empty()) {
        return validate_company_workspace_root(configured.to_string()).map(PathBuf::from);
    }
    let base = std::env::var("RELAY_DEFAULT_WORKSPACE_ROOT")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var("HOME")
                .ok()
                .filter(|value| !value.trim().is_empty())
                .map(|home| PathBuf::from(home).join(".relay"))
        })
        .unwrap_or_else(|| PathBuf::from("/tmp/.relay"));
    validate_company_workspace_root(
        base.join("companies")
            .join(company_id.to_string())
            .to_string_lossy()
            .into_owned(),
    )
    .map(PathBuf::from)
}

pub(super) fn managed_project_directory_name(name: &str, project_id: Uuid) -> String {
    let slug = name
        .trim()
        .to_lowercase()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    let slug = if slug.is_empty() { "project" } else { &slug };
    format!(
        "{}-{}",
        slug.chars().take(48).collect::<String>(),
        &project_id.to_string()[..8]
    )
}

pub(super) fn validate_project_source_folder(
    raw: &str,
    allowed_roots: &[PathBuf],
) -> AppResult<PathBuf> {
    let requested = PathBuf::from(raw.trim());
    if !requested.is_absolute() {
        return Err(AppError::Validation(
            "selected project folder must resolve to a host absolute path".into(),
        ));
    }
    let source = fs::canonicalize(&requested).map_err(|error| {
        AppError::Validation(format!(
            "selected project folder is not accessible on this Relay host: {error}"
        ))
    })?;
    if !source.is_dir() {
        return Err(AppError::Validation(
            "selected project source is not a directory".into(),
        ));
    }
    let effective_roots = if allowed_roots.is_empty() {
        std::env::var("HOME")
            .ok()
            .and_then(|home| fs::canonicalize(home).ok())
            .into_iter()
            .collect::<Vec<_>>()
    } else {
        allowed_roots.to_vec()
    };
    if effective_roots.is_empty()
        || !effective_roots
            .iter()
            .any(|root| source.starts_with(root) && &source != root)
    {
        return Err(AppError::Unauthorized(
            "selected folder is outside the local directories allowed for Relay imports".into(),
        ));
    }
    Ok(source)
}

pub(super) fn import_project_folder(source: &FsPath, destination: &FsPath) -> AppResult<()> {
    const MAX_FILES: usize = 100_000;
    const MAX_BYTES: u64 = 5 * 1024 * 1024 * 1024;
    const EXCLUDED: &[&str] = &[
        ".git",
        ".relay",
        ".relay-agent-trigger",
        "node_modules",
        "target",
    ];
    if destination.exists() {
        return Err(AppError::Conflict(
            "managed project destination already exists".into(),
        ));
    }
    let parent = destination
        .parent()
        .ok_or_else(|| AppError::Validation("managed project destination has no parent".into()))?;
    fs::create_dir_all(parent).map_err(|error| {
        AppError::Internal(format!("failed to create managed workspace: {error}"))
    })?;
    let staging = parent.join(format!(".relay-import-{}", Uuid::new_v4()));
    fs::create_dir(&staging).map_err(|error| {
        AppError::Internal(format!(
            "failed to create project import staging directory: {error}"
        ))
    })?;

    fn copy_tree(
        source_root: &FsPath,
        source: &FsPath,
        destination_root: &FsPath,
        excluded: &[&str],
        files: &mut usize,
        bytes: &mut u64,
    ) -> AppResult<()> {
        for entry in fs::read_dir(source).map_err(|error| {
            AppError::Validation(format!("cannot read imported folder: {error}"))
        })? {
            let entry = entry.map_err(|error| {
                AppError::Validation(format!("cannot inspect imported folder entry: {error}"))
            })?;
            let file_type = entry.file_type().map_err(|error| {
                AppError::Validation(format!("cannot inspect imported file type: {error}"))
            })?;
            if file_type.is_symlink() {
                continue;
            }
            let file_name = entry.file_name();
            if file_type.is_dir()
                && file_name
                    .to_str()
                    .is_some_and(|name| excluded.contains(&name))
            {
                continue;
            }
            let source_path = entry.path();
            let relative = source_path.strip_prefix(source_root).map_err(|_| {
                AppError::Validation("imported folder entry escaped its source root".into())
            })?;
            let destination_path = destination_root.join(relative);
            if file_type.is_dir() {
                fs::create_dir_all(&destination_path).map_err(|error| {
                    AppError::Internal(format!("failed to create imported directory: {error}"))
                })?;
                copy_tree(
                    source_root,
                    &source_path,
                    destination_root,
                    excluded,
                    files,
                    bytes,
                )?;
            } else if file_type.is_file() {
                *files += 1;
                *bytes = bytes.saturating_add(
                    entry
                        .metadata()
                        .map_err(|error| {
                            AppError::Validation(format!("cannot inspect imported file: {error}"))
                        })?
                        .len(),
                );
                if *files > MAX_FILES || *bytes > MAX_BYTES {
                    return Err(AppError::Validation(
                        "selected folder exceeds the Relay import limit (100000 files / 5 GiB)"
                            .into(),
                    ));
                }
                fs::copy(&source_path, &destination_path).map_err(|error| {
                    AppError::Internal(format!("failed to copy imported file: {error}"))
                })?;
            }
        }
        Ok(())
    }

    let result = (|| {
        let mut files = 0usize;
        let mut bytes = 0u64;
        copy_tree(source, source, &staging, EXCLUDED, &mut files, &mut bytes)?;
        initialize_managed_project_git(&staging)?;
        fs::rename(&staging, destination).map_err(|error| {
            AppError::Internal(format!("failed to finalize imported project: {error}"))
        })?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&staging);
    }
    result
}

pub(super) fn initialize_managed_project_git(path: &FsPath) -> AppResult<()> {
    fn run(path: &FsPath, args: &[&str]) -> AppResult<()> {
        let output = Command::new("git")
            .args(args)
            .current_dir(path)
            .env("GIT_TERMINAL_PROMPT", "0")
            .output()
            .map_err(|error| AppError::Internal(format!("failed to start git: {error}")))?;
        if output.status.success() {
            return Ok(());
        }
        Err(AppError::Validation(format!(
            "failed to initialize imported project Git repository: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )))
    }
    run(path, &["init", "-b", "main"])?;
    run(path, &["config", "user.name", "Relay Import"])?;
    run(
        path,
        &["config", "user.email", "relay-import@local.invalid"],
    )?;
    run(path, &["add", "-A"])?;
    run(
        path,
        &["commit", "--allow-empty", "-m", "Import project into Relay"],
    )
}

pub(super) fn load_folder_reference_allowed_roots() -> anyhow::Result<Vec<PathBuf>> {
    let configured = std::env::var("HUMAN_FOLDER_REFERENCE_ALLOWED_ROOTS")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| std::env::var("AGENT_TRIGGER_ALLOWED_LOCAL_ROOTS").ok())
        .unwrap_or_default();
    configured
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .map(|path| {
            if !path.is_absolute() {
                anyhow::bail!("HUMAN_FOLDER_REFERENCE_ALLOWED_ROOTS entries must be absolute");
            }
            let canonical = std::fs::canonicalize(&path).map_err(|error| {
                anyhow::anyhow!(
                    "cannot access HUMAN_FOLDER_REFERENCE_ALLOWED_ROOTS entry {}: {error}",
                    path.display()
                )
            })?;
            if !canonical.is_dir() {
                anyhow::bail!(
                    "HUMAN_FOLDER_REFERENCE_ALLOWED_ROOTS entry is not a directory: {}",
                    canonical.display()
                );
            }
            Ok(canonical)
        })
        .collect()
}

pub(super) fn normalize_uploaded_project_path(raw: &str) -> AppResult<Option<String>> {
    const EXCLUDED: &[&str] = &[
        ".git",
        ".relay",
        ".relay-agent-trigger",
        "node_modules",
        "target",
    ];
    let normalized = raw.replace('\\', "/");
    if normalized.is_empty()
        || normalized.len() > 4_096
        || normalized.starts_with('/')
        || normalized.ends_with('/')
        || normalized.chars().any(char::is_control)
    {
        return Err(AppError::Validation(
            "uploaded project contains an invalid relative path".into(),
        ));
    }
    let components = normalized.split('/').collect::<Vec<_>>();
    if components
        .iter()
        .any(|component| component.is_empty() || matches!(*component, "." | ".."))
    {
        return Err(AppError::Validation(
            "uploaded project path may not contain empty, current, or parent components".into(),
        ));
    }
    if components
        .iter()
        .any(|component| EXCLUDED.contains(component))
    {
        return Ok(None);
    }
    Ok(Some(normalized))
}

pub(super) fn rollback_company_project_git(
    state: &AppState,
    human_user_id: Uuid,
    company_id: Uuid,
    project_id: Uuid,
    existing: Option<ai_chat_application::CompanyProjectGitAdminView>,
) -> Result<(), AppError> {
    if let Some(existing) = existing {
        state.platform.upsert_company_project_git_for_human(
            UpsertCompanyProjectGitForHumanInput {
                human_user_id,
                company_id,
                project_id,
                remote_url: existing.remote_url,
                host_local_path: Some(existing.host_local_path),
                default_branch: Some(existing.default_branch),
                auth_profile: existing.auth_profile,
                allow_agent_push: Some(existing.allow_agent_push),
                branch_prefix: Some(existing.branch_prefix),
            },
        )?;
    } else {
        state.platform.delete_company_project_git_for_human(
            DeleteCompanyProjectGitForHumanInput {
                human_user_id,
                company_id,
                project_id,
            },
        )?;
    }
    Ok(())
}
