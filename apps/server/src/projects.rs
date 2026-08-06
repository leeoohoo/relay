use super::*;

pub(super) async fn create_company_project_for_human(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<Uuid>,
    Json(input): Json<CreateCompanyProjectRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
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

    let project_id = Uuid::new_v4();
    let destination = workspace_root.join(managed_project_directory_name(&input.name, project_id));
    let description = input.description.clone().unwrap_or_default();
    let mut type_evidence = Vec::new();
    let mut imported_local_folder = false;

    match input.source_kind.as_str() {
        "local_folder" => {
            let requested = input.source_local_path.as_deref().ok_or_else(|| {
                AppError::Validation("source_local_path is required for local_folder".into())
            })?;
            let source =
                validate_project_source_folder(requested, &state.folder_reference_allowed_roots)?;
            if destination.starts_with(&source) || source == workspace_root {
                return Err(AppError::Validation(
                    "managed destination cannot be inside the imported source folder".into(),
                )
                .into());
            }
            type_evidence = collect_directory_structure(&source)?;
            let source_for_copy = source.clone();
            let destination_for_copy = destination.clone();
            tokio::task::spawn_blocking(move || {
                import_project_folder(&source_for_copy, &destination_for_copy)
            })
            .await
            .map_err(|error| {
                AppError::Internal(format!("project import task failed: {error}"))
            })??;
            imported_local_folder = true;
        }
        "git" => {
            if input
                .git_remote_url
                .as_deref()
                .is_none_or(|value| value.trim().is_empty())
            {
                return Err(AppError::Validation(
                    "git_remote_url is required for git source".into(),
                )
                .into());
            }
        }
        _ => {
            return Err(
                AppError::Validation("source_kind must be local_folder or git".into()).into(),
            )
        }
    }

    let (inferred_type, inferred_confidence, inferred_evidence) =
        infer_company_project_type(&input.name, &description, &type_evidence);
    let human_selected_type = input
        .project_type
        .as_ref()
        .is_some_and(|value| !value.is_empty());
    let project_type = input.project_type.clone().unwrap_or(inferred_type);
    let project_result =
        state
            .platform
            .create_company_project_for_human(CreateCompanyProjectForHumanInput {
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
            });
    let project = match project_result {
        Ok(project) => project,
        Err(error) => {
            if imported_local_folder {
                let _ = fs::remove_dir_all(&destination);
            }
            return Err(error.into());
        }
    };

    let git = if imported_local_folder {
        Some(
            state
                .platform
                .configure_managed_local_project_git_for_human(
                    ConfigureManagedLocalProjectGitForHumanInput {
                        human_user_id: human.id,
                        company_id,
                        project_id,
                        managed_local_path: destination.to_string_lossy().into_owned(),
                    },
                )?,
        )
    } else {
        Some(state.platform.upsert_company_project_git_for_human(
            UpsertCompanyProjectGitForHumanInput {
                human_user_id: human.id,
                company_id,
                project_id,
                remote_url: input.git_remote_url.unwrap_or_default(),
                host_local_path: Some(destination.to_string_lossy().into_owned()),
                default_branch: input.default_branch,
                auth_profile: input.auth_profile,
                allow_agent_push: Some(true),
                branch_prefix: Some("relay/".into()),
            },
        )?)
    };
    Ok(Json(serde_json::json!({
        "project": project,
        "git": git,
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
    let project_id = Uuid::new_v4();
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
    let project_result =
        state
            .platform
            .create_company_project_for_human(CreateCompanyProjectForHumanInput {
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
            });
    let project = match project_result {
        Ok(project) => project,
        Err(error) => {
            let _ = fs::remove_dir_all(&destination);
            return Err(error.into());
        }
    };
    let git = state
        .platform
        .configure_managed_local_project_git_for_human(
            ConfigureManagedLocalProjectGitForHumanInput {
                human_user_id: human.id,
                company_id,
                project_id,
                managed_local_path: destination.to_string_lossy().into_owned(),
            },
        )?;
    Ok(Json(serde_json::json!({
        "project": project,
        "git": git,
        "managed_local_path": destination,
    })))
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
    let automatic_profile = github_token_profile_name(project_id);
    let github_token_configured = git.as_ref().is_some_and(|git| {
        git.auth_profile.as_deref() == Some(automatic_profile.as_str())
            && state.git_credential_store.has_github_token(project_id)
    });
    let managed_profile = managed_token_profile_name(project_id);
    let managed_token_configured = git.as_ref().is_some_and(|git| {
        git.auth_profile.as_deref() == Some(managed_profile.as_str())
            && state.git_credential_store.has_managed_git_token(project_id)
    });
    Ok(Json(serde_json::json!({
        "git": git,
        "github_token_configured": github_token_configured,
        "managed_token_configured": managed_token_configured,
    })))
}

pub(super) async fn upsert_company_project_git(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, project_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<UpsertCompanyProjectGitRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let existing =
        state
            .platform
            .get_company_project_git_for_human(GetCompanyProjectGitForHumanInput {
                human_user_id: human.id,
                company_id,
                project_id,
            })?;
    let github_token = input
        .github_token
        .as_deref()
        .filter(|token| !token.is_empty())
        .map(str::to_string);
    if input.clear_github_token && github_token.is_some() {
        return Err(AppError::Validation(
            "github_token and clear_github_token cannot be used together".into(),
        )
        .into());
    }
    if let Some(token) = github_token.as_deref() {
        validate_github_token(token)?;
        if !input.remote_url.starts_with("https://") {
            return Err(AppError::Validation(
                "GitHub Token requires an https Git Remote URL".into(),
            )
            .into());
        }
    }
    let automatic_profile = github_token_profile_name(project_id);
    let auth_profile = if github_token.is_some() {
        Some(automatic_profile.clone())
    } else if input.clear_github_token {
        None
    } else {
        input
            .auth_profile
            .clone()
            .or_else(|| existing.as_ref().and_then(|git| git.auth_profile.clone()))
    };
    let should_remove_github_token = input.clear_github_token
        || (existing
            .as_ref()
            .and_then(|git| git.auth_profile.as_deref())
            == Some(automatic_profile.as_str())
            && auth_profile.as_deref() != Some(automatic_profile.as_str()));
    let git = state.platform.upsert_company_project_git_for_human(
        UpsertCompanyProjectGitForHumanInput {
            human_user_id: human.id,
            company_id,
            project_id,
            remote_url: input.remote_url,
            host_local_path: input.host_local_path,
            default_branch: input.default_branch,
            auth_profile,
            allow_agent_push: input.allow_agent_push,
            branch_prefix: input.branch_prefix,
        },
    )?;
    let credential_result = if let Some(token) = github_token.as_deref() {
        state
            .git_credential_store
            .store_github_token(project_id, token)
    } else if should_remove_github_token {
        state.git_credential_store.remove_github_token(project_id)
    } else {
        Ok(())
    };
    if let Err(error) = credential_result {
        if let Err(rollback_error) =
            rollback_company_project_git(&state, human.id, company_id, project_id, existing)
        {
            tracing::error!(
                project_id = %project_id,
                error = %rollback_error,
                "failed to roll back project Git configuration after credential write failure"
            );
        }
        return Err(error.into());
    }
    let github_token_configured = git.auth_profile.as_deref() == Some(automatic_profile.as_str())
        && state.git_credential_store.has_github_token(project_id);
    let managed_profile = managed_token_profile_name(project_id);
    let managed_token_configured = git.auth_profile.as_deref() == Some(managed_profile.as_str())
        && state.git_credential_store.has_managed_git_token(project_id);
    Ok(Json(serde_json::json!({
        "git": git,
        "github_token_configured": github_token_configured,
        "managed_token_configured": managed_token_configured,
    })))
}

pub(super) async fn delete_company_project_git(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, project_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    state
        .platform
        .delete_company_project_git_for_human(DeleteCompanyProjectGitForHumanInput {
            human_user_id: human.id,
            company_id,
            project_id,
        })?;
    state
        .git_credential_store
        .remove_project_tokens(project_id)?;
    Ok(Json(serde_json::json!({ "configured": false })))
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
