use super::*;

const PROJECT_PROVISIONING_LEASE_MINUTES: i64 = 120;

pub(super) async fn create_company_project_for_human(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<Uuid>,
    Json(input): Json<CreateCompanyProjectRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let project_id = Uuid::new_v4();
    state.platform.validate_company_project_creation_for_human(
        CreateCompanyProjectForHumanInput {
            human_user_id: human.id,
            company_id,
            owner_agent_id: input.owner_agent_id,
            name: input.name.clone(),
            description: input.description.clone(),
            member_agent_ids: input.member_agent_ids.clone(),
            project_type: input.project_type.clone(),
            project_type_source: input
                .project_type
                .as_ref()
                .map(|_| PROJECT_TYPE_SOURCE_HUMAN.into()),
            project_type_confidence: None,
            project_type_evidence: Vec::new(),
            project_id: Some(project_id),
        },
    )?;
    let policy = state
        .platform
        .get_company_governance_policy_for_human(human.id, company_id)?;
    let workspace_root = resolve_company_workspace_root(
        policy.effective_settings.managed_workspace_root.as_deref(),
        company_id,
    )?;
    fs::create_dir_all(&workspace_root).map_err(|error| {
        AppError::Internal(format!(
            "failed to create managed workspace {}: {error}",
            workspace_root.display()
        ))
    })?;

    let destination = workspace_root.join(managed_project_directory_name(&input.name, project_id));
    let description = input.description.clone().unwrap_or_default();

    let type_evidence = match input.source_kind.as_str() {
        "local_folder" => {
            let requested = input.source_local_path.as_deref().ok_or_else(|| {
                AppError::Validation("source_local_path is required for local_folder".into())
            })?;
            let source =
                validate_project_source_folder(requested, &state.folder_reference_allowed_roots)?;
            let requested_host_path = PathBuf::from(requested.trim());
            if destination.starts_with(&requested_host_path)
                || destination.starts_with(&source)
                || source == workspace_root
            {
                return Err(AppError::Validation(
                    "managed destination cannot be inside the imported source folder".into(),
                )
                .into());
            }
            let type_evidence = collect_directory_structure(&source)?;
            let source_for_copy = source.clone();
            let destination_for_copy = destination.clone();
            tokio::task::spawn_blocking(move || {
                import_project_folder(&source_for_copy, &destination_for_copy)
            })
            .await
            .map_err(|error| {
                AppError::Internal(format!("project import task failed: {error}"))
            })??;
            type_evidence
        }
        "git" => {
            let remote_url = input
                .git_remote_url
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    AppError::Validation("git_remote_url is required for git source".into())
                })?;
            let branch = input.default_branch.clone();
            import_project_git(
                remote_url,
                branch.as_deref(),
                &destination,
                state.git_import_timeout,
            )
            .await?;
            collect_directory_structure(&destination)?
        }
        _ => {
            return Err(
                AppError::Validation("source_kind must be local_folder or git".into()).into(),
            )
        }
    };

    let (inferred_type, inferred_confidence, inferred_evidence) =
        infer_company_project_type(&input.name, &description, &type_evidence);
    let human_selected_type = input
        .project_type
        .as_ref()
        .is_some_and(|value| !value.is_empty());
    let project_type = input.project_type.clone().unwrap_or(inferred_type);
    let cleanup_job = begin_project_provisioning_cleanup(
        &state,
        &human,
        company_id,
        project_id,
        &input.name,
        &destination,
    )
    .await?;
    let provisioned = match provision_imported_project_git(
        &state,
        &human,
        project_id,
        &input.name,
        &description,
        &destination,
    )
    .await
    {
        Ok(git) => git,
        Err(error) => {
            let cleanup_error = remove_managed_project_directory(&destination);
            let error = project_error_with_cleanup(error, cleanup_error);
            let retry_error = release_project_provisioning_cleanup(&state, cleanup_job.id, &error);
            return Err(project_error_with_cleanup(error, retry_error).into());
        }
    };
    let project_result = state.platform.create_managed_company_project_for_human(
        CreateManagedCompanyProjectForHumanInput {
            project: CreateCompanyProjectForHumanInput {
                human_user_id: human.id,
                company_id,
                owner_agent_id: input.owner_agent_id,
                name: input.name,
                description: input.description,
                member_agent_ids: input.member_agent_ids,
                project_type: Some(project_type),
                project_type_source: Some(if human_selected_type {
                    PROJECT_TYPE_SOURCE_HUMAN.into()
                } else if input.source_kind == "local_folder" {
                    PROJECT_TYPE_SOURCE_FOLDER.into()
                } else {
                    PROJECT_TYPE_SOURCE_DESCRIPTION.into()
                }),
                project_type_confidence: Some(if human_selected_type {
                    100
                } else {
                    inferred_confidence
                }),
                project_type_evidence: if type_evidence.is_empty() {
                    inferred_evidence
                } else {
                    type_evidence.into_iter().take(24).collect()
                },
                project_id: Some(project_id),
            },
            cleanup_job_id: cleanup_job.id,
            remote_url: provisioned.remote_url.clone(),
            host_local_path: destination.to_string_lossy().into_owned(),
            default_branch: provisioned.default_branch.clone(),
            auth_profile: provisioned.auth_profile.clone(),
            allow_agent_push: true,
            branch_prefix: "relay/".into(),
        },
    );
    let (project, git) = match project_result {
        Ok(result) => result,
        Err(error) => {
            let compensated = compensate_failed_project_creation(
                &state,
                &human,
                project_id,
                &destination,
                &provisioned,
                error,
            )
            .await;
            let retry_error =
                release_project_provisioning_cleanup(&state, cleanup_job.id, &compensated);
            return Err(project_error_with_cleanup(compensated, retry_error).into());
        }
    };
    Ok(Json(serde_json::json!({
        "project": project,
        "git": Some(git),
        "managed_local_path": destination,
    })))
}

pub(super) async fn import_company_project_folder_for_human(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<Uuid>,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, ApiError> {
    const MAX_FILES: usize = 100_000;
    const MAX_BYTES: u64 = 5 * 1024 * 1024 * 1024;

    let human = authenticate_human_request(&state, &headers)?;
    let metadata_field = multipart
        .next_field()
        .await
        .map_err(|error| AppError::Validation(format!("invalid project import form: {error}")))?
        .ok_or_else(|| AppError::Validation("project import metadata is required".into()))?;
    if metadata_field.name() != Some("metadata") {
        return Err(AppError::Validation(
            "project import metadata must be the first multipart field".into(),
        )
        .into());
    }
    let metadata: ImportCompanyProjectFolderRequest = serde_json::from_str(
        &metadata_field
            .text()
            .await
            .map_err(|error| AppError::Validation(format!("invalid project metadata: {error}")))?,
    )
    .map_err(|error| AppError::Validation(format!("invalid project metadata: {error}")))?;
    if metadata.file_paths.len() > MAX_FILES {
        return Err(AppError::Validation(
            "selected folder exceeds the Relay import limit (100000 files / 5 GiB)".into(),
        )
        .into());
    }

    let project_id = Uuid::new_v4();
    state.platform.validate_company_project_creation_for_human(
        CreateCompanyProjectForHumanInput {
            human_user_id: human.id,
            company_id,
            owner_agent_id: metadata.owner_agent_id,
            name: metadata.name.clone(),
            description: metadata.description.clone(),
            member_agent_ids: metadata.member_agent_ids.clone(),
            project_type: metadata.project_type.clone(),
            project_type_source: metadata
                .project_type
                .as_ref()
                .map(|_| PROJECT_TYPE_SOURCE_HUMAN.into()),
            project_type_confidence: None,
            project_type_evidence: Vec::new(),
            project_id: Some(project_id),
        },
    )?;

    let mut normalized_paths = Vec::with_capacity(metadata.file_paths.len());
    let mut unique_paths = HashSet::new();
    for raw in &metadata.file_paths {
        let normalized = normalize_uploaded_project_path(raw)?;
        if let Some(path) = normalized.as_ref() {
            if !unique_paths.insert(path.clone()) {
                return Err(AppError::Validation(format!(
                    "uploaded project contains a duplicate path: {path}"
                ))
                .into());
            }
        }
        normalized_paths.push(normalized);
    }

    let policy = state
        .platform
        .get_company_governance_policy_for_human(human.id, company_id)?;
    let workspace_root = resolve_company_workspace_root(
        policy.effective_settings.managed_workspace_root.as_deref(),
        company_id,
    )?;
    fs::create_dir_all(&workspace_root).map_err(|error| {
        AppError::Internal(format!(
            "failed to create managed workspace {}: {error}",
            workspace_root.display()
        ))
    })?;
    let destination =
        workspace_root.join(managed_project_directory_name(&metadata.name, project_id));
    let staging = workspace_root.join(format!(".relay-upload-{project_id}"));
    fs::create_dir(&staging).map_err(|error| {
        AppError::Internal(format!(
            "failed to create project upload staging directory: {error}"
        ))
    })?;

    let upload_result: Result<(usize, u64), ApiError> = async {
        let mut received_indices = HashSet::new();
        let mut total_bytes = 0u64;
        while let Some(mut field) = multipart.next_field().await.map_err(|error| {
            AppError::Validation(format!("invalid project upload stream: {error}"))
        })? {
            let field_name = field.name().unwrap_or_default();
            let Some(index) = field_name
                .strip_prefix("file_")
                .and_then(|value| value.parse::<usize>().ok())
            else {
                return Err(AppError::Validation(
                    "project upload contains an unexpected multipart field".into(),
                )
                .into());
            };
            let relative_path = normalized_paths.get(index).ok_or_else(|| {
                AppError::Validation("project upload file index is invalid".into())
            })?;
            if !received_indices.insert(index) {
                return Err(AppError::Validation(
                    "project upload contains a duplicate file index".into(),
                )
                .into());
            }
            let Some(relative_path) = relative_path else {
                while field
                    .chunk()
                    .await
                    .map_err(|error| {
                        AppError::Validation(format!("invalid upload chunk: {error}"))
                    })?
                    .is_some()
                {}
                continue;
            };
            let output_path = staging.join(relative_path);
            if let Some(parent) = output_path.parent() {
                tokio::fs::create_dir_all(parent).await.map_err(|error| {
                    AppError::Internal(format!("failed to create imported directory: {error}"))
                })?;
            }
            let mut output = tokio::fs::File::create(&output_path)
                .await
                .map_err(|error| {
                    AppError::Internal(format!("failed to create imported file: {error}"))
                })?;
            while let Some(chunk) = field.chunk().await.map_err(|error| {
                AppError::Validation(format!("invalid project upload chunk: {error}"))
            })? {
                total_bytes = total_bytes.saturating_add(chunk.len() as u64);
                if total_bytes > MAX_BYTES {
                    return Err(AppError::Validation(
                        "selected folder exceeds the Relay import limit (100000 files / 5 GiB)"
                            .into(),
                    )
                    .into());
                }
                output.write_all(&chunk).await.map_err(|error| {
                    AppError::Internal(format!("failed to write imported file: {error}"))
                })?;
            }
            output.flush().await.map_err(|error| {
                AppError::Internal(format!("failed to flush imported file: {error}"))
            })?;
        }
        if received_indices.len() != normalized_paths.len() {
            return Err(AppError::Validation(
                "project upload did not include every selected file".into(),
            )
            .into());
        }
        Ok((received_indices.len(), total_bytes))
    }
    .await;
    if let Err(error) = upload_result {
        let _ = tokio::fs::remove_dir_all(&staging).await;
        return Err(error);
    }

    let staging_for_finalize = staging.clone();
    let destination_for_finalize = destination.clone();
    let finalize_result = tokio::task::spawn_blocking(move || -> AppResult<()> {
        initialize_managed_project_git(&staging_for_finalize)?;
        fs::rename(&staging_for_finalize, &destination_for_finalize).map_err(|error| {
            AppError::Internal(format!("failed to finalize uploaded project: {error}"))
        })
    })
    .await
    .map_err(|error| AppError::Internal(format!("project finalization task failed: {error}")))?;
    if let Err(error) = finalize_result {
        let _ = tokio::fs::remove_dir_all(&staging).await;
        return Err(error.into());
    }

    let description = metadata.description.clone().unwrap_or_default();
    let type_evidence = normalized_paths
        .iter()
        .filter_map(Clone::clone)
        .take(5_000)
        .collect::<Vec<_>>();
    let (inferred_type, inferred_confidence, inferred_evidence) =
        infer_company_project_type(&metadata.name, &description, &type_evidence);
    let human_selected_type = metadata
        .project_type
        .as_ref()
        .is_some_and(|value| !value.is_empty());
    let project_type = metadata.project_type.clone().unwrap_or(inferred_type);
    let cleanup_job = begin_project_provisioning_cleanup(
        &state,
        &human,
        company_id,
        project_id,
        &metadata.name,
        &destination,
    )
    .await?;
    let provisioned_git = match provision_imported_project_git(
        &state,
        &human,
        project_id,
        &metadata.name,
        &description,
        &destination,
    )
    .await
    {
        Ok(git) => git,
        Err(error) => {
            let cleanup_error = remove_managed_project_directory(&destination);
            let error = project_error_with_cleanup(error, cleanup_error);
            let retry_error = release_project_provisioning_cleanup(&state, cleanup_job.id, &error);
            return Err(project_error_with_cleanup(error, retry_error).into());
        }
    };
    let project_result = state.platform.create_managed_company_project_for_human(
        CreateManagedCompanyProjectForHumanInput {
            project: CreateCompanyProjectForHumanInput {
                human_user_id: human.id,
                company_id,
                owner_agent_id: metadata.owner_agent_id,
                name: metadata.name,
                description: metadata.description,
                member_agent_ids: metadata.member_agent_ids,
                project_type: Some(project_type),
                project_type_source: Some(if human_selected_type {
                    PROJECT_TYPE_SOURCE_HUMAN.into()
                } else {
                    PROJECT_TYPE_SOURCE_FOLDER.into()
                }),
                project_type_confidence: Some(if human_selected_type {
                    100
                } else {
                    inferred_confidence
                }),
                project_type_evidence: if type_evidence.is_empty() {
                    inferred_evidence
                } else {
                    type_evidence.into_iter().take(24).collect()
                },
                project_id: Some(project_id),
            },
            cleanup_job_id: cleanup_job.id,
            remote_url: provisioned_git.remote_url.clone(),
            host_local_path: destination.to_string_lossy().into_owned(),
            default_branch: provisioned_git.default_branch.clone(),
            auth_profile: provisioned_git.auth_profile.clone(),
            allow_agent_push: true,
            branch_prefix: "relay/".into(),
        },
    );
    let (project, git) = match project_result {
        Ok(result) => result,
        Err(error) => {
            let compensated = compensate_failed_project_creation(
                &state,
                &human,
                project_id,
                &destination,
                &provisioned_git,
                error,
            )
            .await;
            let retry_error =
                release_project_provisioning_cleanup(&state, cleanup_job.id, &compensated);
            return Err(project_error_with_cleanup(compensated, retry_error).into());
        }
    };
    Ok(Json(serde_json::json!({
        "project": project,
        "git": git,
        "managed_local_path": destination,
    })))
}

async fn provision_imported_project_git(
    state: &AppState,
    human: &HumanUser,
    project_id: Uuid,
    project_name: &str,
    description: &str,
    local_path: &FsPath,
) -> AppResult<ProvisionedProjectGit> {
    state
        .harness_provisioner
        .ensure_active_account(human)
        .await?;
    let provisioned = state
        .harness_provisioner
        .provision_project_git(
            human.id,
            project_id,
            project_name,
            description,
            false,
            &state.git_credential_store,
        )
        .await?;
    let provisioned_for_push = provisioned.clone();
    let credential_store = state.git_credential_store.clone();
    let local_path = local_path.to_path_buf();
    let publish_result = tokio::task::spawn_blocking(move || {
        push_managed_project_to_remote(&local_path, &provisioned_for_push, &credential_store)
    })
    .await
    .map_err(|error| AppError::Internal(format!("Harness Git publish task failed: {error}")))?;
    if let Err(error) = publish_result {
        let cleanup_error = state
            .harness_provisioner
            .cleanup_provisioned_project_git(
                human.id,
                project_id,
                &provisioned,
                &state.git_credential_store,
            )
            .await
            .err();
        return Err(project_error_with_cleanup(error, cleanup_error));
    }
    Ok(provisioned)
}

async fn begin_project_provisioning_cleanup(
    state: &AppState,
    human: &HumanUser,
    company_id: Uuid,
    project_id: Uuid,
    project_name: &str,
    destination: &FsPath,
) -> AppResult<ProjectProvisioningCleanupJob> {
    if let Err(error) = state.harness_provisioner.ensure_active_account(human).await {
        return Err(project_error_with_cleanup(
            error,
            remove_managed_project_directory(destination),
        ));
    }
    let now = now_utc();
    let job = ProjectProvisioningCleanupJob {
        id: Uuid::new_v4(),
        human_user_id: human.id,
        company_id,
        project_id,
        managed_local_path: destination.to_string_lossy().into_owned(),
        repository_identifier: generated_repository_identifier(project_name, project_id),
        access_token_identifier: initial_project_access_token_identifier(project_id),
        status: "running".into(),
        attempts: 0,
        next_attempt_at: now,
        lease_expires_at: Some(now + chrono::Duration::minutes(PROJECT_PROVISIONING_LEASE_MINUTES)),
        last_error: None,
        created_at: now,
        updated_at: now,
        completed_at: None,
    };
    if let Err(error) = state
        .platform
        .save_project_provisioning_cleanup_job(job.clone())
    {
        return Err(project_error_with_cleanup(
            error,
            remove_managed_project_directory(destination),
        ));
    }
    Ok(job)
}

fn remove_managed_project_directory(destination: &FsPath) -> Option<AppError> {
    match fs::remove_dir_all(destination) {
        Ok(()) => None,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => Some(AppError::Internal(format!(
            "failed to remove managed project directory {}: {error}",
            destination.display()
        ))),
    }
}

fn release_project_provisioning_cleanup(
    state: &AppState,
    cleanup_job_id: Uuid,
    original_error: &AppError,
) -> Option<AppError> {
    let now = now_utc();
    let message = original_error
        .to_string()
        .chars()
        .take(2_000)
        .collect::<String>();
    state
        .platform
        .retry_project_provisioning_cleanup_job(cleanup_job_id, message, now, now)
        .err()
}

async fn compensate_failed_project_creation(
    state: &AppState,
    human: &HumanUser,
    project_id: Uuid,
    destination: &FsPath,
    provisioned: &ProvisionedProjectGit,
    original: AppError,
) -> AppError {
    let mut cleanup_failures = Vec::new();
    match fs::remove_dir_all(destination) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => cleanup_failures.push(format!("remove managed directory: {error}")),
    }
    if let Err(error) = state
        .harness_provisioner
        .cleanup_provisioned_project_git(
            human.id,
            project_id,
            provisioned,
            &state.git_credential_store,
        )
        .await
    {
        cleanup_failures.push(error.to_string());
    }
    if cleanup_failures.is_empty() {
        original
    } else {
        AppError::Internal(format!(
            "{original}; automatic project cleanup failed: {}",
            cleanup_failures.join("; ")
        ))
    }
}

fn project_error_with_cleanup(original: AppError, cleanup_error: Option<AppError>) -> AppError {
    match cleanup_error {
        Some(cleanup_error) => AppError::Internal(format!(
            "{original}; automatic project cleanup failed: {cleanup_error}"
        )),
        None => original,
    }
}

pub(super) fn spawn_project_provisioning_cleanup_worker(state: AppState) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(StdDuration::from_secs(15));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            interval.tick().await;
            loop {
                let now = now_utc();
                let lease_expires_at = now + chrono::Duration::minutes(5);
                let job = match state
                    .platform
                    .claim_due_project_provisioning_cleanup_job(now, lease_expires_at)
                {
                    Ok(Some(job)) => job,
                    Ok(None) => break,
                    Err(error) => {
                        tracing::error!(%error, "failed to claim project provisioning cleanup job");
                        break;
                    }
                };
                let result = cleanup_project_provisioning_job(&state, &job).await;
                let finished_at = now_utc();
                match result {
                    Ok(()) => {
                        if let Err(error) = state
                            .platform
                            .complete_project_provisioning_cleanup_job(job.id, finished_at)
                        {
                            tracing::error!(job_id = %job.id, %error, "failed to complete project provisioning cleanup job");
                        }
                    }
                    Err(error) => {
                        let exponent = job.attempts.clamp(1, 8) as u32;
                        let retry_seconds = (15_i64 * 2_i64.pow(exponent)).min(3_600);
                        let next_attempt_at =
                            finished_at + chrono::Duration::seconds(retry_seconds);
                        let message = error.to_string().chars().take(2_000).collect::<String>();
                        if let Err(store_error) =
                            state.platform.retry_project_provisioning_cleanup_job(
                                job.id,
                                message,
                                next_attempt_at,
                                finished_at,
                            )
                        {
                            tracing::error!(job_id = %job.id, %store_error, "failed to reschedule project provisioning cleanup job");
                        }
                    }
                }
            }
        }
    });
}

async fn cleanup_project_provisioning_job(
    state: &AppState,
    job: &ProjectProvisioningCleanupJob,
) -> AppResult<()> {
    let mut failures = Vec::new();
    match fs::remove_dir_all(&job.managed_local_path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => failures.push(format!("remove managed directory: {error}")),
    }
    if let Err(error) = state
        .harness_provisioner
        .cleanup_project_git_resources_by_identifier(
            job.human_user_id,
            job.project_id,
            &job.repository_identifier,
            &job.access_token_identifier,
            &state.git_credential_store,
        )
        .await
    {
        failures.push(error.to_string());
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(AppError::Internal(failures.join("; ")))
    }
}

pub(super) async fn get_company_project_git(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, project_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let git =
        state
            .platform
            .get_company_project_git_for_human(GetCompanyProjectGitForHumanInput {
                human_user_id: human.id,
                company_id,
                project_id,
            })?;
    if let Some(git) = git.as_ref() {
        if !state.harness_provisioner.is_enabled() {
            return Err(AppError::Conflict(
                "Relay 当前未启用 Harness 集成，请以 self_hosted 或 official 模式重新启动".into(),
            )
            .into());
        }
        if !state
            .harness_provisioner
            .is_managed_repository(human.id, git.remote_url.as_str())?
        {
            return Err(AppError::Validation(
                "项目仓库不是当前用户空间中的 Harness 托管仓库".into(),
            )
            .into());
        }
    }
    let managed_profile = managed_token_profile_name(project_id);
    let managed_token_configured = git.as_ref().is_some_and(|git| {
        git.auth_profile.as_deref() == Some(managed_profile.as_str())
            && state.git_credential_store.has_managed_git_token(project_id)
    });
    Ok(Json(serde_json::json!({
        "git": git,
        "managed_token_configured": managed_token_configured,
    })))
}

pub(super) async fn update_company_project_rule(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, project_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<UpdateCompanyProjectRuleRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let rule = state.platform.update_company_project_rule_for_human(
        UpdateCompanyProjectRuleForHumanInput {
            human_user_id: human.id,
            company_id,
            project_id,
            content: input.content,
        },
    )?;
    Ok(Json(serde_json::json!({ "rule": rule })))
}

pub(super) async fn transfer_company_project_owner(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, project_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<TransferCompanyProjectOwnerRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let project = state.platform.transfer_company_project_owner_for_human(
        TransferCompanyProjectOwnerForHumanInput {
            human_user_id: human.id,
            company_id,
            project_id,
            owner_agent_id: input.owner_agent_id,
        },
    )?;
    Ok(Json(serde_json::json!({ "project": project })))
}

pub(super) async fn request_company_project_rule_generation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, project_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<RequestCompanyProjectRuleGenerationRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    state
        .platform
        .request_company_project_rule_generation_for_human(
            RequestCompanyProjectRuleGenerationForHumanInput {
                human_user_id: human.id,
                company_id,
                project_id,
                agent_id: input.agent_id,
                instructions: input.instructions,
            },
        )?;
    Ok(Json(serde_json::json!({ "requested": true })))
}

pub(super) async fn upsert_company_project_asset_refresh(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, project_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<UpsertCompanyProjectAssetRefreshRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let asset_refresh = state
        .platform
        .upsert_company_project_asset_refresh_for_human(
            UpsertCompanyProjectAssetRefreshForHumanInput {
                human_user_id: human.id,
                company_id,
                project_id,
                maintainer_agent_id: input.maintainer_agent_id,
                interval_minutes: input.interval_minutes,
                enabled: input.enabled,
                run_now: input.run_now,
            },
        )?;
    Ok(Json(serde_json::json!({ "asset_refresh": asset_refresh })))
}

pub(super) async fn list_company_project_tasks_for_human(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let tasks = state
        .platform
        .list_company_project_tasks_for_human(human.id, company_id)?;
    Ok(Json(serde_json::json!({ "tasks": tasks })))
}

pub(super) async fn create_company_project_task_for_human(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, project_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<CreateCompanyProjectTaskRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let task = state.platform.create_company_project_task_for_human(
        CreateCompanyProjectTaskForHumanInput {
            human_user_id: human.id,
            company_id,
            project_id,
            title: input.title,
            description: input.description,
            priority: input.priority,
            assignee_agent_id: input.assignee_agent_id,
            due_at: input.due_at,
            depends_on_task_ids: input.depends_on_task_ids,
        },
    )?;
    Ok(Json(serde_json::json!({ "task": task })))
}

pub(super) async fn update_company_project_task_for_human(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, project_id, task_id)): Path<(Uuid, Uuid, Uuid)>,
    Json(input): Json<UpdateCompanyProjectTaskRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let task = state.platform.update_company_project_task_for_human(
        UpdateCompanyProjectTaskForHumanInput {
            human_user_id: human.id,
            company_id,
            project_id,
            task_id,
            title: input.title,
            description: input.description,
            status: input.status,
            priority: input.priority,
            assignee_agent_id: input.assignee_agent_id,
            clear_assignee: input.clear_assignee,
            due_at: input.due_at,
            clear_due_at: input.clear_due_at,
            depends_on_task_ids: input.depends_on_task_ids,
        },
    )?;
    Ok(Json(serde_json::json!({ "task": task })))
}

pub(super) async fn list_project_gates_for_human(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, project_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let gates = state
        .platform
        .list_project_gates_for_human(ListProjectGatesForHumanInput {
            human_user_id: human.id,
            company_id,
            project_id,
        })?;
    let requirements = state
        .platform
        .list_project_gate_requirements_for_human(human.id, company_id, project_id)?;
    Ok(Json(serde_json::json!({
        "gates": gates,
        "requirements": requirements,
    })))
}

pub(super) async fn create_project_gate_for_human(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, project_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<CreateProjectGateRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let gate = state
        .platform
        .create_project_gate_for_human(CreateProjectGateForHumanInput {
            human_user_id: human.id,
            company_id,
            project_id,
            gate_key: input.gate_key,
            gate_type: input.gate_type,
            title: input.title,
            related_task_id: input.related_task_id,
            required_evidence: input.required_evidence,
        })?;
    Ok(Json(serde_json::json!({ "gate": gate })))
}

pub(super) async fn decide_project_gate_for_human(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, project_id, gate_id)): Path<(Uuid, Uuid, Uuid)>,
    Json(input): Json<DecideProjectGateRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let gate = state
        .platform
        .decide_project_gate_for_human(DecideProjectGateForHumanInput {
            human_user_id: human.id,
            company_id,
            project_id,
            gate_id,
            status: input.status,
            decision_summary: input.decision_summary,
        })?;
    Ok(Json(serde_json::json!({ "gate": gate })))
}
pub(super) async fn set_project_task_gate_requirement_for_human(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, project_id, task_id, gate_id)): Path<(Uuid, Uuid, Uuid, Uuid)>,
    Json(input): Json<SetProjectTaskGateRequirementRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let requirement = state.platform.set_project_task_gate_requirement_for_human(
        SetProjectTaskGateRequirementForHumanInput {
            human_user_id: human.id,
            company_id,
            project_id,
            task_id,
            gate_id,
            required_status: input.required_status,
        },
    )?;
    Ok(Json(serde_json::json!({ "requirement": requirement })))
}
