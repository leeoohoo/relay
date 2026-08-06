use ai_chat_shared::{now_utc, AppError, AppResult};
use uuid::Uuid;

use super::storage::truncate;
use super::*;

impl CodexControlStore {
    pub fn list_mcp_target_selectors(&self) -> AppResult<Vec<String>> {
        let _lock = self.lock()?;
        let mut selectors = vec!["default".to_string()];
        selectors.extend(
            self.read_profiles_unlocked()?
                .profiles
                .into_iter()
                .filter(|profile| profile.status == CODEX_AUTH_PROFILE_STATUS_ACTIVE)
                .map(|profile| profile.selector),
        );
        selectors.sort();
        selectors.dedup();
        Ok(selectors)
    }

    pub fn enqueue_mcp_refresh(&self, company_id: Uuid, selector: String) -> AppResult<()> {
        let _lock = self.lock()?;
        self.ensure_company_selector_unlocked(company_id, &selector)?;
        self.ensure_no_pending_mcp_operation_unlocked(&selector)?;
        self.write_request_unlocked(&CodexControlRequest {
            id: Uuid::new_v4(),
            kind: CodexControlRequestKind::RefreshMcp,
            company_id: Some(company_id),
            profile_id: None,
            api_key: None,
            base_url: None,
            target_selector: Some(selector.clone()),
            mcp_server: None,
            mcp_server_name: None,
            created_at: now_utc(),
        })?;
        self.update_mcp_operation_unlocked(
            &selector,
            CODEX_MCP_OPERATION_REFRESH_PENDING,
            None,
            None,
        )?;
        Ok(())
    }

    pub fn enqueue_mcp_add(&self, company_id: Uuid, input: CodexMcpServerInput) -> AppResult<()> {
        validate_mcp_server_input(&input)?;
        let _lock = self.lock()?;
        self.ensure_company_selector_unlocked(company_id, &input.target_selector)?;
        self.ensure_no_pending_mcp_operation_unlocked(&input.target_selector)?;
        let selector = input.target_selector.clone();
        let server_name = input.name.clone();
        self.write_request_unlocked(&CodexControlRequest {
            id: Uuid::new_v4(),
            kind: CodexControlRequestKind::AddMcp,
            company_id: Some(company_id),
            profile_id: None,
            api_key: None,
            base_url: None,
            target_selector: Some(selector.clone()),
            mcp_server: Some(input),
            mcp_server_name: Some(server_name.clone()),
            created_at: now_utc(),
        })?;
        self.update_mcp_operation_unlocked(
            &selector,
            CODEX_MCP_OPERATION_ADD_PENDING,
            Some(server_name),
            None,
        )?;
        Ok(())
    }

    pub fn enqueue_mcp_remove(
        &self,
        company_id: Uuid,
        selector: String,
        server_name: String,
    ) -> AppResult<()> {
        validate_mcp_selector(&selector)?;
        validate_mcp_server_name(&server_name)?;
        let _lock = self.lock()?;
        self.ensure_company_selector_unlocked(company_id, &selector)?;
        self.ensure_no_pending_mcp_operation_unlocked(&selector)?;
        let snapshot = self
            .read_mcp_catalog_unlocked()?
            .environments
            .into_iter()
            .find(|snapshot| snapshot.selector == selector)
            .ok_or_else(|| {
                AppError::Conflict("refresh the MCP list before removing a server".into())
            })?;
        let server = snapshot
            .servers
            .iter()
            .find(|server| server.name == server_name)
            .ok_or_else(|| AppError::NotFound("Codex MCP server not found".into()))?;
        if !server.configured_by_user {
            return Err(AppError::Conflict(
                "plugin-provided MCP servers cannot be removed from user Codex configuration"
                    .into(),
            ));
        }
        self.write_request_unlocked(&CodexControlRequest {
            id: Uuid::new_v4(),
            kind: CodexControlRequestKind::RemoveMcp,
            company_id: Some(company_id),
            profile_id: None,
            api_key: None,
            base_url: None,
            target_selector: Some(selector.clone()),
            mcp_server: None,
            mcp_server_name: Some(server_name.clone()),
            created_at: now_utc(),
        })?;
        self.update_mcp_operation_unlocked(
            &selector,
            CODEX_MCP_OPERATION_REMOVE_PENDING,
            Some(server_name),
            None,
        )?;
        Ok(())
    }

    pub fn publish_mcp_snapshot(
        &self,
        selector: &str,
        servers: Vec<CodexMcpServerView>,
        error: Option<String>,
    ) -> AppResult<()> {
        validate_mcp_selector(selector)?;
        let _lock = self.lock()?;
        let mut catalog = self.read_mcp_catalog_unlocked()?;
        let snapshot = mcp_snapshot_mut(&mut catalog, selector);
        snapshot.status = if error.is_some() { "failed" } else { "ready" }.into();
        snapshot.operation_status = if error.is_some() {
            CODEX_MCP_OPERATION_FAILED
        } else {
            CODEX_MCP_OPERATION_IDLE
        }
        .into();
        snapshot.pending_server_name = None;
        if error.is_none() {
            snapshot.servers = servers;
        }
        snapshot.last_checked_at = Some(now_utc());
        snapshot.last_error = error.map(|message| truncate(&message, 1_000));
        self.write_mcp_catalog_unlocked(&catalog)
    }

    fn ensure_company_selector_unlocked(&self, company_id: Uuid, selector: &str) -> AppResult<()> {
        validate_mcp_selector(selector)?;
        if selector == "default" {
            return Ok(());
        }
        let allowed = self
            .read_profiles_unlocked()?
            .profiles
            .into_iter()
            .any(|profile| {
                profile.company_id == company_id
                    && profile.selector == selector
                    && profile.status == CODEX_AUTH_PROFILE_STATUS_ACTIVE
            });
        if allowed {
            Ok(())
        } else {
            Err(AppError::NotFound(
                "Codex authentication environment not found".into(),
            ))
        }
    }

    fn ensure_no_pending_mcp_operation_unlocked(&self, selector: &str) -> AppResult<()> {
        let catalog = self.read_mcp_catalog_unlocked()?;
        let pending = catalog
            .environments
            .iter()
            .find(|snapshot| snapshot.selector == selector)
            .is_some_and(|snapshot| {
                !matches!(
                    snapshot.operation_status.as_str(),
                    CODEX_MCP_OPERATION_IDLE | CODEX_MCP_OPERATION_FAILED
                )
            });
        if pending {
            Err(AppError::Conflict(
                "another Codex MCP operation is already pending for this environment".into(),
            ))
        } else {
            Ok(())
        }
    }

    pub(super) fn update_mcp_operation_unlocked(
        &self,
        selector: &str,
        operation_status: &str,
        pending_server_name: Option<String>,
        error: Option<String>,
    ) -> AppResult<()> {
        let mut catalog = self.read_mcp_catalog_unlocked()?;
        let snapshot = mcp_snapshot_mut(&mut catalog, selector);
        snapshot.operation_status = operation_status.into();
        snapshot.pending_server_name = pending_server_name;
        snapshot.last_error = error;
        self.write_mcp_catalog_unlocked(&catalog)
    }

    pub(super) fn mark_mcp_request_processing(
        &self,
        request: &CodexControlRequest,
        operation_status: &str,
    ) -> AppResult<()> {
        let selector = request
            .target_selector
            .as_deref()
            .ok_or_else(|| AppError::Internal("Codex MCP request has no target selector".into()))?;
        let _lock = self.lock()?;
        self.update_mcp_operation_unlocked(
            selector,
            operation_status,
            request.mcp_server_name.clone(),
            None,
        )
    }
}
fn mcp_snapshot_mut<'a>(
    catalog: &'a mut CodexMcpCatalogFile,
    selector: &str,
) -> &'a mut CodexMcpEnvironmentSnapshot {
    if let Some(index) = catalog
        .environments
        .iter()
        .position(|snapshot| snapshot.selector == selector)
    {
        return &mut catalog.environments[index];
    }
    catalog
        .environments
        .push(CodexMcpEnvironmentSnapshot::new(selector.to_string()));
    catalog.environments.last_mut().expect("MCP snapshot")
}

fn validate_mcp_selector(value: &str) -> AppResult<()> {
    if value == "default" || managed_profile_id(value).is_some() {
        Ok(())
    } else {
        Err(AppError::Validation(
            "Codex MCP target environment is invalid".into(),
        ))
    }
}

fn validate_mcp_server_name(value: &str) -> AppResult<()> {
    if value.trim() != value
        || value.is_empty()
        || value.len() > 80
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "_-".contains(character))
    {
        return Err(AppError::Validation(
            "Codex MCP server name must contain only letters, numbers, underscores, or hyphens"
                .into(),
        ));
    }
    Ok(())
}

pub(super) fn validate_mcp_server_input(input: &CodexMcpServerInput) -> AppResult<()> {
    validate_mcp_selector(&input.target_selector)?;
    validate_mcp_server_name(&input.name)?;
    if input.args.len() > 64
        || input.args.iter().any(|argument| {
            argument.is_empty() || argument.len() > 2_048 || argument.chars().any(char::is_control)
        })
    {
        return Err(AppError::Validation(
            "Codex MCP stdio arguments are invalid".into(),
        ));
    }
    match input.transport.as_str() {
        CODEX_MCP_TRANSPORT_HTTP => {
            let url = input
                .url
                .as_deref()
                .ok_or_else(|| AppError::Validation("Streamable HTTP MCP requires a URL".into()))?;
            let parsed_url = reqwest::Url::parse(url)
                .map_err(|_| AppError::Validation("Codex MCP URL is invalid".into()))?;
            if url.len() > 2_048
                || url.chars().any(char::is_control)
                || !matches!(parsed_url.scheme(), "https" | "http")
                || !parsed_url.username().is_empty()
                || parsed_url.password().is_some()
                || parsed_url.query().is_some()
                || parsed_url.fragment().is_some()
            {
                return Err(AppError::Validation("Codex MCP URL is invalid".into()));
            }
            if input.command.is_some() || !input.args.is_empty() {
                return Err(AppError::Validation(
                    "HTTP MCP cannot include a stdio command".into(),
                ));
            }
            if let Some(name) = input.bearer_token_env_var.as_deref() {
                validate_environment_name(name)?;
            }
        }
        CODEX_MCP_TRANSPORT_STDIO => {
            let command = input
                .command
                .as_deref()
                .ok_or_else(|| AppError::Validation("stdio MCP requires a command".into()))?;
            if command.trim() != command
                || command.is_empty()
                || command.len() > 1_024
                || command.chars().any(char::is_control)
            {
                return Err(AppError::Validation(
                    "Codex MCP stdio command is invalid".into(),
                ));
            }
            if input.url.is_some() || input.bearer_token_env_var.is_some() {
                return Err(AppError::Validation(
                    "stdio MCP cannot include HTTP configuration".into(),
                ));
            }
        }
        _ => {
            return Err(AppError::Validation(
                "unsupported Codex MCP transport".into(),
            ))
        }
    }
    Ok(())
}

fn validate_environment_name(value: &str) -> AppResult<()> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
        || value.as_bytes()[0].is_ascii_digit()
    {
        return Err(AppError::Validation(
            "Codex MCP bearer token environment variable name is invalid".into(),
        ));
    }
    Ok(())
}
