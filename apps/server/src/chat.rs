use super::*;

pub(super) async fn open_human_company_direct_conversation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<Uuid>,
    Json(input): Json<OpenHumanDirectConversationRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let conversation = state.platform.open_human_company_direct_conversation(
        OpenHumanCompanyDirectConversationInput {
            human_user_id: human.id,
            company_id,
            target_agent_id: input.target_agent_id,
        },
    )?;
    Ok(Json(serde_json::json!({ "conversation": conversation })))
}

pub(super) async fn send_human_company_message(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, conversation_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<SendHumanCompanyMessageRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    state
        .platform
        .ensure_human_can_send_company_messages(human.id, company_id)?;
    let attachments = build_local_folder_attachments(
        &input.folder_references,
        &state.folder_reference_allowed_roots,
    )?;
    let message = state.platform.send_human_company_message_with_attachments(
        SendHumanCompanyMessageWithAttachmentsInput {
            human_user_id: human.id,
            company_id,
            conversation_id,
            content: input.content,
            mentioned_agent_ids: input.mentioned_agent_ids,
            mention_all: input.mention_all,
            attachments,
        },
    )?;
    Ok(Json(serde_json::json!({ "message": message })))
}

struct PendingUploadedFile {
    file_name: String,
    content_type: String,
    bytes: Bytes,
}

pub(super) async fn send_human_company_message_with_attachments(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, conversation_id)): Path<(Uuid, Uuid)>,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    state
        .platform
        .ensure_human_can_send_company_messages(human.id, company_id)?;
    let mut content = String::new();
    let mut mentioned_agent_ids = Vec::new();
    let mut mention_all = false;
    let mut relative_paths = Vec::<String>::new();
    let mut folder_references = Vec::<HumanFolderReferenceRequest>::new();
    let mut pending_files = Vec::<PendingUploadedFile>::new();
    let mut total_bytes = 0usize;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|error| AppError::Validation(format!("invalid attachment form: {error}")))?
    {
        let field_name = field.name().unwrap_or_default().to_string();
        match field_name.as_str() {
            "content" => {
                content = field.text().await.map_err(|error| {
                    AppError::Validation(format!("invalid message content: {error}"))
                })?;
            }
            "mentioned_agent_ids" => {
                let value = field.text().await.map_err(|error| {
                    AppError::Validation(format!("invalid mentioned Agent list: {error}"))
                })?;
                mentioned_agent_ids = serde_json::from_str(&value).map_err(|_| {
                    AppError::Validation("mentioned_agent_ids must be a UUID array".into())
                })?;
            }
            "mention_all" => {
                mention_all = field
                    .text()
                    .await
                    .map_err(|error| {
                        AppError::Validation(format!("invalid mention_all value: {error}"))
                    })?
                    .parse::<bool>()
                    .map_err(|_| AppError::Validation("mention_all must be boolean".into()))?;
            }
            "relative_paths" => {
                let value = field.text().await.map_err(|error| {
                    AppError::Validation(format!("invalid relative path list: {error}"))
                })?;
                relative_paths = serde_json::from_str(&value).map_err(|_| {
                    AppError::Validation("relative_paths must be a string array".into())
                })?;
            }
            "folder_references" => {
                let value = field.text().await.map_err(|error| {
                    AppError::Validation(format!("invalid folder reference list: {error}"))
                })?;
                folder_references = serde_json::from_str(&value).map_err(|_| {
                    AppError::Validation("folder_references must be an array".into())
                })?;
            }
            "file" => {
                if pending_files.len() >= 20 {
                    return Err(AppError::Validation(
                        "a message can contain at most 20 files".into(),
                    )
                    .into());
                }
                let file_name =
                    sanitize_attachment_file_name(field.file_name().unwrap_or("attachment.bin"))?;
                let content_type = field
                    .content_type()
                    .unwrap_or("application/octet-stream")
                    .to_string();
                let bytes = field.bytes().await.map_err(|error| {
                    AppError::Validation(format!("cannot read uploaded file: {error}"))
                })?;
                if bytes.len() > 20 * 1024 * 1024 {
                    return Err(AppError::Validation(format!(
                        "file {file_name} exceeds the 20 MiB limit"
                    ))
                    .into());
                }
                total_bytes = total_bytes.saturating_add(bytes.len());
                if total_bytes > 100 * 1024 * 1024 {
                    return Err(AppError::Validation(
                        "message attachments exceed the 100 MiB total limit".into(),
                    )
                    .into());
                }
                pending_files.push(PendingUploadedFile {
                    file_name,
                    content_type,
                    bytes,
                });
            }
            _ => {}
        }
    }

    if !relative_paths.is_empty() && relative_paths.len() != pending_files.len() {
        return Err(AppError::Validation(
            "relative_paths must match the uploaded file count".into(),
        )
        .into());
    }
    let mut attachments =
        build_local_folder_attachments(&folder_references, &state.folder_reference_allowed_roots)?;
    if attachments.len() + pending_files.len() > 20 {
        return Err(AppError::Validation(
            "a message can contain at most 20 attachments and folder references".into(),
        )
        .into());
    }

    let mut stored_paths = Vec::<PathBuf>::new();
    for (index, pending) in pending_files.into_iter().enumerate() {
        let attachment_id = Uuid::new_v4();
        let storage_key = format!("{}/{}", &attachment_id.to_string()[..2], attachment_id);
        let storage_path = state.message_attachments_root.join(&storage_key);
        if let Some(parent) = storage_path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|error| {
                AppError::Validation(format!("cannot create attachment directory: {error}"))
            })?;
        }
        tokio::fs::write(&storage_path, &pending.bytes)
            .await
            .map_err(|error| AppError::Validation(format!("cannot store attachment: {error}")))?;
        stored_paths.push(storage_path);
        let relative_path = relative_paths
            .get(index)
            .map(String::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(validate_attachment_relative_path)
            .transpose()?;
        attachments.push(MessageAttachmentView {
            id: attachment_id,
            kind: if pending.content_type.starts_with("image/") {
                "image".into()
            } else {
                "file".into()
            },
            file_name: pending.file_name,
            relative_path,
            content_type: pending.content_type,
            byte_size: i64::try_from(pending.bytes.len()).unwrap_or(i64::MAX),
            local_path: None,
            directory_entries: Vec::new(),
            purpose: None,
            storage_key,
        });
    }

    let send_result = state.platform.send_human_company_message_with_attachments(
        SendHumanCompanyMessageWithAttachmentsInput {
            human_user_id: human.id,
            company_id,
            conversation_id,
            content,
            mentioned_agent_ids,
            mention_all,
            attachments,
        },
    );
    let message = match send_result {
        Ok(message) => message,
        Err(error) => {
            for path in stored_paths {
                let _ = tokio::fs::remove_file(path).await;
            }
            return Err(error.into());
        }
    };
    Ok(Json(serde_json::json!({ "message": message })))
}

pub(super) async fn download_message_attachment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((conversation_id, message_id, attachment_id)): Path<(Uuid, Uuid, Uuid)>,
) -> Result<Response, ApiError> {
    let messages = if bearer_token(&headers).is_ok() {
        let human = authenticate_human_request(&state, &headers)?;
        state
            .platform
            .get_owned_conversation_messages(human.id, conversation_id)?
    } else {
        let agent_key = agent_key_from_headers(&headers)
            .ok_or_else(|| AppError::Unauthorized("missing authentication token".into()))?;
        let agent = state.platform.authenticate_agent_key(&agent_key)?;
        state
            .platform
            .get_agent_conversation_messages(agent.id, conversation_id)?
    };
    let attachment = messages
        .into_iter()
        .find(|message| message.id == message_id)
        .and_then(|message| {
            message
                .attachments
                .into_iter()
                .find(|attachment| attachment.id == attachment_id)
        })
        .filter(|attachment| matches!(attachment.kind.as_str(), "file" | "image"))
        .ok_or_else(|| AppError::NotFound("message attachment not found".into()))?;
    if attachment.storage_key.is_empty()
        || attachment.storage_key.contains("..")
        || PathBuf::from(&attachment.storage_key).is_absolute()
    {
        return Err(AppError::NotFound("message attachment file is unavailable".into()).into());
    }
    let bytes = tokio::fs::read(state.message_attachments_root.join(&attachment.storage_key))
        .await
        .map_err(|_| AppError::NotFound("message attachment file is unavailable".into()))?;
    let mut response = Response::new(Body::from(bytes));
    response.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_str(&attachment.content_type)
            .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream")),
    );
    response.headers_mut().insert(
        CONTENT_DISPOSITION,
        HeaderValue::from_str(if attachment.kind == "image" {
            "inline"
        } else {
            "attachment"
        })
        .expect("static content disposition is valid"),
    );
    Ok(response)
}

pub(super) fn sanitize_attachment_file_name(value: &str) -> Result<String, ApiError> {
    let file_name = std::path::Path::new(value)
        .file_name()
        .and_then(|value| value.to_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::Validation("attachment file name is invalid".into()))?;
    if file_name.chars().any(char::is_control) || file_name.chars().count() > 255 {
        return Err(AppError::Validation("attachment file name is invalid".into()).into());
    }
    Ok(file_name.to_string())
}

pub(super) fn validate_attachment_relative_path(value: &str) -> Result<String, ApiError> {
    let normalized = value.replace('\\', "/");
    if normalized.chars().any(char::is_control)
        || normalized.len() > 2_000
        || normalized.starts_with('/')
        || normalized
            .split('/')
            .any(|component| component.is_empty() || matches!(component, "." | ".."))
    {
        return Err(AppError::Validation("attachment relative path is invalid".into()).into());
    }
    Ok(normalized)
}

pub(super) fn build_local_folder_attachments(
    requests: &[HumanFolderReferenceRequest],
    allowed_roots: &[PathBuf],
) -> Result<Vec<MessageAttachmentView>, ApiError> {
    requests
        .iter()
        .map(|request| {
            let requested_path = PathBuf::from(request.local_path.trim());
            if !requested_path.is_absolute() {
                return Err(AppError::Validation(
                    "folder reference must use a host absolute path".into(),
                )
                .into());
            }
            let local_path = std::fs::canonicalize(&requested_path).map_err(|_| {
                AppError::Validation(format!(
                    "folder does not exist on the server host: {}",
                    requested_path.display()
                ))
            })?;
            if !local_path.is_dir() {
                return Err(AppError::Validation(format!(
                    "folder reference is not a directory: {}",
                    local_path.display()
                ))
                .into());
            }
            if allowed_roots.is_empty() {
                return Err(AppError::Validation(
                    "local folder references are disabled; configure HUMAN_FOLDER_REFERENCE_ALLOWED_ROOTS"
                        .into(),
                )
                .into());
            }
            if !allowed_roots.iter().any(|root| local_path.starts_with(root)) {
                return Err(AppError::Unauthorized(
                    "folder reference is outside HUMAN_FOLDER_REFERENCE_ALLOWED_ROOTS".into(),
                )
                .into());
            }
            let directory_entries = collect_directory_structure(&local_path)?;
            let file_name = local_path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("project")
                .to_string();
            Ok(MessageAttachmentView {
                id: Uuid::new_v4(),
                kind: "local_folder".into(),
                file_name,
                relative_path: None,
                content_type: "application/x-relay-local-folder".into(),
                byte_size: 0,
                local_path: Some(local_path.to_string_lossy().to_string()),
                directory_entries,
                purpose: Some("create_project_and_push_to_git".into()),
                storage_key: String::new(),
            })
        })
        .collect()
}

pub(super) fn collect_directory_structure(root: &std::path::Path) -> Result<Vec<String>, ApiError> {
    const MAX_ENTRIES: usize = 5_000;
    const MAX_DEPTH: usize = 16;
    const MAX_PATH_BYTES: usize = 512_000;
    const EXCLUDED_DIRECTORY_NAMES: &[&str] = &[
        ".git",
        ".relay",
        ".relay-agent-trigger",
        "node_modules",
        "target",
    ];
    fn visit(
        root: &std::path::Path,
        directory: &std::path::Path,
        depth: usize,
        entries: &mut Vec<String>,
        path_bytes: &mut usize,
    ) -> Result<(), ApiError> {
        if depth > MAX_DEPTH {
            return Ok(());
        }
        let mut children = std::fs::read_dir(directory)
            .map_err(|error| {
                AppError::Validation(format!(
                    "cannot read folder {}: {error}",
                    directory.display()
                ))
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| {
                AppError::Validation(format!(
                    "cannot enumerate folder {}: {error}",
                    directory.display()
                ))
            })?;
        children.sort_by_key(|entry| entry.file_name());
        for child in children {
            if entries.len() >= MAX_ENTRIES {
                return Err(AppError::Validation(format!(
                    "folder contains more than {MAX_ENTRIES} entries"
                ))
                .into());
            }
            let file_type = child.file_type().map_err(|error| {
                AppError::Validation(format!("cannot inspect folder entry: {error}"))
            })?;
            if file_type.is_symlink() {
                continue;
            }
            let file_name = child.file_name();
            if file_type.is_dir()
                && file_name
                    .to_str()
                    .is_some_and(|name| EXCLUDED_DIRECTORY_NAMES.contains(&name))
            {
                continue;
            }
            let path = child.path();
            let relative = path.strip_prefix(root).map_err(|_| {
                AppError::Validation("folder entry escaped the selected directory".into())
            })?;
            let mut display = relative.to_string_lossy().replace('\\', "/");
            if file_type.is_dir() {
                display.push('/');
            }
            *path_bytes = path_bytes.saturating_add(display.len());
            if *path_bytes > MAX_PATH_BYTES {
                return Err(AppError::Validation(format!(
                    "folder structure exceeds the {MAX_PATH_BYTES} byte metadata limit"
                ))
                .into());
            }
            entries.push(display);
            if file_type.is_dir() {
                visit(root, &path, depth + 1, entries, path_bytes)?;
            }
        }
        Ok(())
    }

    let mut entries = Vec::new();
    let mut path_bytes = 0usize;
    visit(root, root, 0, &mut entries, &mut path_bytes)?;
    Ok(entries)
}

pub(super) async fn stream_company_events_for_human(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<Uuid>,
    Query(query): Query<RealtimeEventsQuery>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ApiError> {
    let session_token = bearer_token(&headers)?.to_string();
    let human = authenticate_human_request(&state, &headers)?;
    state
        .platform
        .list_company_realtime_events_for_human(human.id, company_id, 0, 1)?;
    let after_sequence_id = requested_realtime_cursor(&headers, query.after_sequence_id)
        .map(Ok)
        .unwrap_or_else(|| state.platform.latest_company_realtime_sequence(company_id))?;
    Ok(build_company_event_sse(
        state,
        company_id,
        after_sequence_id,
        query.limit.unwrap_or(200).clamp(1, 500),
        Some(session_token),
    ))
}

pub(super) async fn stream_company_events_for_agent(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<Uuid>,
    Query(query): Query<RealtimeEventsQuery>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ApiError> {
    let agent_key = agent_key_from_headers(&headers)
        .or_else(|| state.mcp_config.agent_key.clone())
        .ok_or_else(|| AppError::Unauthorized("missing x-agent-key or bearer token".into()))?;
    let agent = state.platform.authenticate_agent_key(&agent_key)?;
    state
        .platform
        .list_company_realtime_events_for_agent(agent.id, company_id, 0, 1)?;
    let after_sequence_id = requested_realtime_cursor(&headers, query.after_sequence_id)
        .map(Ok)
        .unwrap_or_else(|| state.platform.latest_company_realtime_sequence(company_id))?;
    Ok(build_company_event_sse(
        state,
        company_id,
        after_sequence_id,
        query.limit.unwrap_or(200).clamp(1, 500),
        None,
    ))
}

pub(super) fn requested_realtime_cursor(
    headers: &HeaderMap,
    query_cursor: Option<i64>,
) -> Option<i64> {
    query_cursor
        .or_else(|| {
            headers
                .get("last-event-id")
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<i64>().ok())
        })
        .map(|value| value.max(0))
}

pub(super) fn build_company_event_sse(
    state: AppState,
    company_id: Uuid,
    after_sequence_id: i64,
    batch_limit: usize,
    presence_session_token: Option<String>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let (sender, receiver) = mpsc::channel::<CompanyRealtimeEvent>(256);
    let mut signal_receiver = state.realtime_sender.subscribe();
    tokio::spawn(async move {
        let mut cursor = after_sequence_id;
        let mut next_presence_refresh = tokio::time::Instant::now();
        loop {
            if tokio::time::Instant::now() >= next_presence_refresh {
                if let Some(session_token) = presence_session_token.as_deref() {
                    if let Err(error) = state.platform.authenticate_human_session(session_token) {
                        tracing::info!(
                            company_id = %company_id,
                            error = %error,
                            "company SSE stream stopped because the Human session is no longer active"
                        );
                        return;
                    }
                }
                next_presence_refresh = tokio::time::Instant::now() + StdDuration::from_secs(30);
            }
            let events =
                match state
                    .platform
                    .read_company_realtime_events(company_id, cursor, batch_limit)
                {
                    Ok(events) => events,
                    Err(error) => {
                        tracing::error!(
                            company_id = %company_id,
                            cursor,
                            error = %error,
                            "company SSE stream stopped after repository failure"
                        );
                        return;
                    }
                };
            let had_events = !events.is_empty();
            for event in events {
                cursor = cursor.max(event.sequence_id);
                if sender.send(event).await.is_err() {
                    return;
                }
            }
            if had_events {
                continue;
            }

            tokio::select! {
                signal = signal_receiver.recv() => {
                    match signal {
                        Ok(signal) if signal.company_id == company_id && signal.sequence_id > cursor => {}
                        Ok(_) | Err(broadcast::error::RecvError::Lagged(_)) => {}
                        Err(broadcast::error::RecvError::Closed) => {
                            tokio::time::sleep(StdDuration::from_secs(1)).await;
                        }
                    }
                }
                _ = tokio::time::sleep(StdDuration::from_secs(1)) => {}
            }
        }
    });

    let stream = futures_util::stream::unfold(receiver, |mut receiver| async move {
        receiver.recv().await.map(|event| {
            let data = serde_json::to_string(&event).unwrap_or_else(|_| "{}".into());
            let sse_event = Event::default()
                .id(event.sequence_id.to_string())
                .event(event.event_type)
                .data(data);
            (Ok(sse_event), receiver)
        })
    });
    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(StdDuration::from_secs(15))
            .text("keep-alive"),
    )
}

pub(super) async fn list_owner_agents(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(human_user_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    require_same_human(human.id, human_user_id)?;
    let agents = state.platform.list_owner_agents(human_user_id)?;
    Ok(Json(serde_json::json!({ "agents": agents })))
}

pub(super) async fn list_agent_conversations(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let conversations = state
        .platform
        .list_owned_agent_conversations(human.id, agent_id)?;
    Ok(Json(serde_json::json!({ "conversations": conversations })))
}

pub(super) async fn get_conversation_messages(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(conversation_id): Path<Uuid>,
    Query(query): Query<ConversationMessagesQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let page = state.platform.get_owned_conversation_message_page(
        human.id,
        conversation_id,
        query.before_message_id,
        query.limit.unwrap_or(50),
    )?;
    Ok(Json(serde_json::json!({
        "messages": page.messages,
        "next_cursor": page.next_cursor,
        "has_more": page.has_more,
    })))
}
