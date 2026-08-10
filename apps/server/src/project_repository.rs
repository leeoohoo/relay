use super::*;

const MAX_REPOSITORY_FILE_BYTES: u64 = 1024 * 1024;
const DEFAULT_DIRECTORY_PAGE_SIZE: usize = 100;
const MAX_DIRECTORY_PAGE_SIZE: usize = 200;

#[derive(Debug, Deserialize)]
pub(super) struct ProjectRepositoryTreeQuery {
    #[serde(rename = "ref")]
    reference: Option<String>,
    path: Option<String>,
    page: Option<usize>,
    per_page: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ProjectRepositoryFileQuery {
    #[serde(rename = "ref")]
    reference: Option<String>,
    path: String,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct ProjectRepositoryRef {
    pub(super) name: String,
    pub(super) full_name: String,
    pub(super) commit: String,
    pub(super) kind: String,
    pub(super) is_default: bool,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct ProjectRepositoryEntry {
    pub(super) name: String,
    pub(super) path: String,
    pub(super) kind: String,
    pub(super) size: Option<u64>,
    pub(super) mode: String,
}

#[derive(Debug, Serialize)]
pub(super) struct ProjectRepositoryRefsResponse {
    refs: Vec<ProjectRepositoryRef>,
    default_ref: String,
    refreshed_at: String,
    source: String,
}

#[derive(Debug, Serialize)]
pub(super) struct ProjectRepositoryTreeResponse {
    reference: String,
    commit: String,
    path: String,
    entries: Vec<ProjectRepositoryEntry>,
    page: usize,
    per_page: usize,
    total: usize,
    total_pages: usize,
}

#[derive(Debug, Serialize)]
pub(super) struct ProjectRepositoryFileResponse {
    pub(super) reference: String,
    pub(super) commit: String,
    pub(super) path: String,
    pub(super) name: String,
    pub(super) size: u64,
    pub(super) line_count: Option<usize>,
    pub(super) binary: bool,
    pub(super) content: Option<String>,
    pub(super) language: String,
}

pub(super) async fn list_company_project_repository_refs(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, project_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<ProjectRepositoryRefsResponse>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let git = project_repository_git(&state, human.id, company_id, project_id)?;
    let refs = state
        .harness_provisioner
        .list_repository_refs(human.id, git.remote_url.as_str())
        .await?
        .ok_or_else(|| non_harness_repository_error(state.harness_provisioner.is_enabled()))?;
    let refs = harness_repository_refs(refs, git.default_branch.as_str());
    Ok(Json(repository_refs_response(refs, "harness_api")?))
}

pub(super) async fn list_company_project_repository_tree(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, project_id)): Path<(Uuid, Uuid)>,
    Query(query): Query<ProjectRepositoryTreeQuery>,
) -> Result<Json<ProjectRepositoryTreeResponse>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let git = project_repository_git(&state, human.id, company_id, project_id)?;
    let path = normalize_repository_path(query.path.as_deref().unwrap_or(""), true)?;
    let refs = state
        .harness_provisioner
        .list_repository_refs(human.id, git.remote_url.as_str())
        .await?
        .ok_or_else(|| non_harness_repository_error(state.harness_provisioner.is_enabled()))?;
    let refs = harness_repository_refs(refs, git.default_branch.as_str());
    let reference = selected_repository_ref(&refs, query.reference.as_deref())?;
    let content = state
        .harness_provisioner
        .get_repository_content(
            human.id,
            git.remote_url.as_str(),
            reference.name.as_str(),
            path.as_str(),
        )
        .await?
        .ok_or_else(|| non_harness_repository_error(state.harness_provisioner.is_enabled()))?;
    let HarnessRepositoryContent::Directory { entries, .. } = content else {
        return Err(
            AppError::Validation("selected repository path is not a directory".into()).into(),
        );
    };
    let entries = entries
        .into_iter()
        .map(|entry| ProjectRepositoryEntry {
            name: entry.name,
            path: entry.path,
            kind: entry.kind,
            size: None,
            mode: String::new(),
        })
        .collect();
    Ok(Json(paginate_repository_tree(
        reference,
        path,
        entries,
        query.page,
        query.per_page,
    )))
}

pub(super) async fn get_company_project_repository_file(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, project_id)): Path<(Uuid, Uuid)>,
    Query(query): Query<ProjectRepositoryFileQuery>,
) -> Result<Json<ProjectRepositoryFileResponse>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let git = project_repository_git(&state, human.id, company_id, project_id)?;
    let path = normalize_repository_path(&query.path, false)?;
    let refs = state
        .harness_provisioner
        .list_repository_refs(human.id, git.remote_url.as_str())
        .await?
        .ok_or_else(|| non_harness_repository_error(state.harness_provisioner.is_enabled()))?;
    let refs = harness_repository_refs(refs, git.default_branch.as_str());
    let reference = selected_repository_ref(&refs, query.reference.as_deref())?;
    let content = state
        .harness_provisioner
        .get_repository_content(
            human.id,
            git.remote_url.as_str(),
            reference.name.as_str(),
            path.as_str(),
        )
        .await?
        .ok_or_else(|| non_harness_repository_error(state.harness_provisioner.is_enabled()))?;
    let HarnessRepositoryContent::File(file) = content else {
        return Err(AppError::Validation("selected repository path is not a file".into()).into());
    };
    Ok(Json(repository_file_response(
        &reference, file.path, file.name, file.bytes,
    )?))
}

fn non_harness_repository_error(harness_enabled: bool) -> AppError {
    if harness_enabled {
        AppError::Validation("项目仓库不是 Relay 管理的 Harness 仓库".into())
    } else {
        AppError::Conflict(
            "Relay 当前未启用 Harness 集成，请以 self_hosted 或 official 模式重新启动".into(),
        )
    }
}

fn harness_repository_refs(
    refs: Vec<ai_chat_infrastructure::harness::HarnessRepositoryRef>,
    default_branch: &str,
) -> Vec<ProjectRepositoryRef> {
    refs.into_iter()
        .map(|reference| ProjectRepositoryRef {
            is_default: reference.is_default || reference.name == default_branch,
            full_name: reference.name.clone(),
            name: reference.name,
            commit: reference.commit,
            kind: "branch".into(),
        })
        .collect()
}

fn repository_refs_response(
    refs: Vec<ProjectRepositoryRef>,
    source: &str,
) -> AppResult<ProjectRepositoryRefsResponse> {
    let default_ref = refs
        .iter()
        .find(|reference| reference.is_default)
        .or_else(|| refs.first())
        .map(|reference| reference.full_name.clone())
        .ok_or_else(|| AppError::Conflict("project repository has no browsable refs".into()))?;
    Ok(ProjectRepositoryRefsResponse {
        refs,
        default_ref,
        refreshed_at: now_utc().to_rfc3339(),
        source: source.into(),
    })
}

fn paginate_repository_tree(
    reference: ProjectRepositoryRef,
    path: String,
    mut entries: Vec<ProjectRepositoryEntry>,
    requested_page: Option<usize>,
    requested_per_page: Option<usize>,
) -> ProjectRepositoryTreeResponse {
    entries.sort_by(|left, right| {
        repository_kind_order(&left.kind)
            .cmp(&repository_kind_order(&right.kind))
            .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
    });
    let total = entries.len();
    let per_page = requested_per_page
        .unwrap_or(DEFAULT_DIRECTORY_PAGE_SIZE)
        .clamp(1, MAX_DIRECTORY_PAGE_SIZE);
    let total_pages = total.div_ceil(per_page).max(1);
    let page = requested_page.unwrap_or(1).clamp(1, total_pages);
    let start = (page - 1) * per_page;
    let entries = entries.into_iter().skip(start).take(per_page).collect();
    ProjectRepositoryTreeResponse {
        reference: reference.full_name,
        commit: reference.commit,
        path,
        entries,
        page,
        per_page,
        total,
        total_pages,
    }
}

fn project_repository_git(
    state: &AppState,
    human_user_id: Uuid,
    company_id: Uuid,
    project_id: Uuid,
) -> AppResult<ai_chat_application::CompanyProjectGitAdminView> {
    state
        .platform
        .get_company_project_git_for_human(GetCompanyProjectGitForHumanInput {
            human_user_id,
            company_id,
            project_id,
        })?
        .ok_or_else(|| AppError::NotFound("project Git configuration not found".into()))
}

pub(super) fn selected_repository_ref(
    refs: &[ProjectRepositoryRef],
    requested: Option<&str>,
) -> AppResult<ProjectRepositoryRef> {
    if let Some(requested) = requested {
        return refs
            .iter()
            .find(|reference| reference.full_name == requested)
            .cloned()
            .ok_or_else(|| AppError::Validation("unknown repository ref".into()));
    }
    let selected = refs
        .iter()
        .find(|reference| reference.is_default)
        .or_else(|| refs.first())
        .cloned();
    selected.ok_or_else(|| AppError::Conflict("project repository has no browsable refs".into()))
}

fn repository_file_response(
    reference: &ProjectRepositoryRef,
    path: String,
    name: String,
    bytes: Vec<u8>,
) -> AppResult<ProjectRepositoryFileResponse> {
    let size = u64::try_from(bytes.len())
        .map_err(|_| AppError::Validation("repository file is too large to preview".into()))?;
    if size > MAX_REPOSITORY_FILE_BYTES {
        return Err(AppError::Validation(format!(
            "repository file is larger than the {} MiB preview limit",
            MAX_REPOSITORY_FILE_BYTES / (1024 * 1024)
        )));
    }
    let language = repository_file_language(&name);
    let binary = bytes.contains(&0) || std::str::from_utf8(&bytes).is_err();
    let (content, line_count) = if binary {
        (None, None)
    } else {
        let content = String::from_utf8(bytes)
            .map_err(|_| AppError::Internal("repository text file changed encoding".into()))?;
        let line_count = content.lines().count();
        (Some(content), Some(line_count))
    };
    Ok(ProjectRepositoryFileResponse {
        reference: reference.full_name.clone(),
        commit: reference.commit.clone(),
        path,
        name,
        size,
        line_count,
        binary,
        content,
        language,
    })
}

pub(super) fn normalize_repository_path(raw: &str, allow_empty: bool) -> AppResult<String> {
    let normalized = raw.replace('\\', "/");
    if normalized.is_empty() {
        return if allow_empty {
            Ok(String::new())
        } else {
            Err(AppError::Validation(
                "repository file path is required".into(),
            ))
        };
    }
    if normalized.len() > 4_096
        || normalized.starts_with('/')
        || normalized.ends_with('/')
        || normalized.chars().any(char::is_control)
        || normalized
            .split('/')
            .any(|component| component.is_empty() || matches!(component, "." | ".."))
    {
        return Err(AppError::Validation(
            "invalid repository relative path".into(),
        ));
    }
    Ok(normalized)
}

fn repository_file_language(name: &str) -> String {
    let lower = name.to_ascii_lowercase();
    if lower == "dockerfile" || lower.starts_with("dockerfile.") {
        return "dockerfile".into();
    }
    lower
        .rsplit_once('.')
        .map(|(_, extension)| extension.to_string())
        .unwrap_or_else(|| "text".into())
}

fn repository_kind_order(kind: &str) -> u8 {
    match kind {
        "directory" => 0,
        "file" | "symlink" => 1,
        _ => 2,
    }
}
