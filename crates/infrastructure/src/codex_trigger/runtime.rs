use super::*;

impl CodexTriggerRunner {
    pub async fn run(&self, request: CodexRunRequest) -> AppResult<CodexRunResult> {
        validate_request(&request)?;
        if let Some(thread_id) = request.existing_thread_id.as_deref() {
            let resumed = self.run_once(&request, Some(thread_id)).await?;
            if should_replace_session(&resumed) {
                report_progress(
                    request.progress_handler.as_ref(),
                    "session_recovery",
                    "Codex 原会话无法稳定完成，正在创建下一代会话继续处理",
                    Some(thread_id),
                );
                let created = self.run_once(&request, None).await?;
                return Ok(to_public_result(created, false, true));
            }
            return Ok(to_public_result(resumed, true, false));
        }
        let created = self.run_once(&request, None).await?;
        Ok(to_public_result(created, false, false))
    }

    async fn run_once(
        &self,
        request: &CodexRunRequest,
        resume_thread_id: Option<&str>,
    ) -> AppResult<ProcessOutcome> {
        if request.approval_handler.is_some() {
            self.run_app_server_once(request, resume_thread_id).await
        } else {
            self.run_exec_once(request, resume_thread_id).await
        }
    }

    async fn run_exec_once(
        &self,
        request: &CodexRunRequest,
        resume_thread_id: Option<&str>,
    ) -> AppResult<ProcessOutcome> {
        let sandbox_mode = codex_sandbox_mode(&request.sandbox_mode)?;
        let mut command = Command::new(&self.executable);
        command
            .args(&self.prefix_args)
            .arg("--ask-for-approval")
            .arg("never");
        command
            .arg("exec")
            .arg("--skip-git-repo-check")
            .arg("--json");
        self.apply_run_profile_arguments(&mut command, request)
            .await?;
        if let Some(model) = request.model.as_deref() {
            command.arg("--model").arg(model);
        }
        apply_managed_cli_settings(&mut command, request, self.auto_compact_token_limit);
        apply_managed_mcp_settings(&mut command, &request.managed_mcp_servers);
        command
            .arg("--sandbox")
            .arg(sandbox_mode)
            .arg("--config")
            .arg(format!(
                "mcp_servers.{}.url={}",
                self.mcp_server_name,
                toml_string(&self.mcp_url)
            ))
            .arg("--config")
            .arg(format!(
                "mcp_servers.{}.required=true",
                self.mcp_server_name
            ))
            .arg("--config")
            .arg(format!(
                "mcp_servers.{}.env_http_headers={{ \"x-agent-run-token\" = {}, \"x-relay-session-kind\" = {} }}",
                self.mcp_server_name,
                toml_string(&self.run_token_env_name),
                toml_string(SESSION_KIND_ENV)
            ))
            .arg("--config")
            .arg(format!(
                "shell_environment_policy.exclude={}",
                toml_string_array(&self.shell_excluded_environment_names)
            ))
            .arg("--config")
            .arg("shell_environment_policy.ignore_default_excludes=true");
        if let Some(thread_id) = resume_thread_id {
            command.arg("resume").arg(thread_id);
        }
        command
            .arg(&request.prompt)
            .current_dir(&request.cwd)
            .env_clear()
            .envs(&self.inherited_environment)
            .env(&self.run_token_env_name, &request.run_token)
            .env(SESSION_KIND_ENV, &request.session_kind)
            .env("GIT_TERMINAL_PROMPT", "0")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        self.apply_profile_environment(&mut command, &request.codex_profile)?;
        for (key, value) in &request.environment {
            command.env(key, value);
        }
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.as_std_mut().process_group(0);
        }

        let mut child = command.spawn().map_err(|error| {
            AppError::Validation(format!(
                "failed to start Codex executable {}: {}",
                self.executable.display(),
                sanitize_error(&error.to_string())
            ))
        })?;
        let process_id = child.id();
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AppError::Validation("Codex stdout pipe was not available".into()))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| AppError::Validation("Codex stderr pipe was not available".into()))?;
        report_progress(
            request.progress_handler.as_ref(),
            "starting",
            "正在启动本地 Codex",
            resume_thread_id,
        );
        let stdout_task = tokio::spawn(read_jsonl_events(stdout, request.progress_handler.clone()));
        let stderr_task = tokio::spawn(read_limited_text(stderr, MAX_STDERR_BYTES));

        report_progress(
            request.progress_handler.as_ref(),
            "starting",
            "正在连接 Codex app-server",
            resume_thread_id,
        );

        let mut timed_out = false;
        let mut cancelled = false;
        let wait_result = tokio::select! {
            result = timeout(Duration::from_secs(request.max_run_seconds), child.wait()) => Some(result),
            _ = wait_for_cancellation(request.cancellation_handler.as_ref()) => {
                cancelled = true;
                None
            }
        };
        let exit_status = match wait_result {
            Some(Ok(result)) => result.map_err(process_error)?,
            Some(Err(_)) => {
                timed_out = true;
                terminate_process_tree(process_id);
                match timeout(Duration::from_secs(5), child.wait()).await {
                    Ok(result) => result.map_err(process_error)?,
                    Err(_) => {
                        kill_process_tree(process_id);
                        child.wait().await.map_err(process_error)?
                    }
                }
            }
            None => {
                terminate_process_tree(process_id);
                match timeout(Duration::from_secs(5), child.wait()).await {
                    Ok(result) => result.map_err(process_error)?,
                    Err(_) => {
                        kill_process_tree(process_id);
                        child.wait().await.map_err(process_error)?
                    }
                }
            }
        };
        let events = stdout_task.await.map_err(join_error)??;
        let stderr = stderr_task.await.map_err(join_error)??;
        let mut error_message = events.error_message.clone();
        if error_message.is_none() && !exit_status.success() && !stderr.trim().is_empty() {
            error_message = Some(truncate(&sanitize_error(&stderr), 2_000));
        }
        if cancelled {
            error_message = Some("Codex run cancelled because the project was paused".into());
        } else if timed_out {
            error_message = Some(format!(
                "Codex run exceeded {} seconds",
                request.max_run_seconds
            ));
        }
        let mut status = if cancelled {
            CodexRunStatus::Cancelled
        } else if timed_out {
            CodexRunStatus::TimedOut
        } else if exit_status.success() && events.turn_completed && !events.turn_failed {
            CodexRunStatus::Succeeded
        } else {
            CodexRunStatus::Failed
        };
        let thread_id = events
            .thread_id
            .or_else(|| resume_thread_id.map(str::to_string));
        if status == CodexRunStatus::Succeeded && thread_id.is_none() {
            status = CodexRunStatus::Failed;
            error_message = Some("Codex completed without emitting a thread.started event".into());
        }
        if status == CodexRunStatus::Succeeded {
            // Codex may emit transient transport errors while its own reconnect loop is
            // recovering. A completed turn is authoritative, so those intermediate
            // warnings must not be persisted as the final run error.
            error_message = None;
        }
        if status == CodexRunStatus::Failed && error_message.is_none() {
            error_message = Some("Codex run failed before completing the turn".into());
        }
        Ok(ProcessOutcome {
            status,
            thread_id,
            exit_code: exit_status.code(),
            final_message: events.final_message,
            error_message,
            turn_started: events.turn_started,
        })
    }

    async fn run_app_server_once(
        &self,
        request: &CodexRunRequest,
        resume_thread_id: Option<&str>,
    ) -> AppResult<ProcessOutcome> {
        let sandbox_mode = codex_sandbox_mode(&request.sandbox_mode)?;
        let approval_handler = request.approval_handler.as_ref().ok_or_else(|| {
            AppError::Validation("Codex app-server execution requires an approval handler".into())
        })?;
        let mut command = Command::new(&self.executable);
        command.args(&self.prefix_args);
        self.apply_run_profile_arguments(&mut command, request)
            .await?;
        apply_managed_cli_settings(&mut command, request, self.auto_compact_token_limit);
        apply_managed_mcp_settings(&mut command, &request.managed_mcp_servers);
        command
            .arg("--config")
            .arg(format!(
                "mcp_servers.{}.url={}",
                self.mcp_server_name,
                toml_string(&self.mcp_url)
            ))
            .arg("--config")
            .arg(format!(
                "mcp_servers.{}.required=true",
                self.mcp_server_name
            ))
            .arg("--config")
            .arg(format!(
                "mcp_servers.{}.env_http_headers={{ \"x-agent-run-token\" = {}, \"x-relay-session-kind\" = {} }}",
                self.mcp_server_name,
                toml_string(&self.run_token_env_name),
                toml_string(SESSION_KIND_ENV)
            ))
            .arg("--config")
            .arg(format!(
                "shell_environment_policy.exclude={}",
                toml_string_array(&self.shell_excluded_environment_names)
            ))
            .arg("--config")
            .arg("shell_environment_policy.ignore_default_excludes=true");
        command
            .arg("app-server")
            .arg("--stdio")
            .current_dir(&request.cwd)
            .env_clear()
            .envs(&self.inherited_environment)
            .env(&self.run_token_env_name, &request.run_token)
            .env(SESSION_KIND_ENV, &request.session_kind)
            .env("GIT_TERMINAL_PROMPT", "0")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        self.apply_profile_environment(&mut command, &request.codex_profile)?;
        for (key, value) in &request.environment {
            command.env(key, value);
        }
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.as_std_mut().process_group(0);
        }

        let mut child = command.spawn().map_err(|error| {
            AppError::Validation(format!(
                "failed to start Codex app-server {}: {}",
                self.executable.display(),
                sanitize_error(&error.to_string())
            ))
        })?;
        let process_id = child.id();
        let mut stdin = child.stdin.take().ok_or_else(|| {
            AppError::Validation("Codex app-server stdin pipe was not available".into())
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            AppError::Validation("Codex app-server stdout pipe was not available".into())
        })?;
        let stderr = child.stderr.take().ok_or_else(|| {
            AppError::Validation("Codex app-server stderr pipe was not available".into())
        })?;
        let stderr_task = tokio::spawn(read_limited_text(stderr, MAX_STDERR_BYTES));

        let drive_result = tokio::select! {
            result = timeout(
                Duration::from_secs(request.max_run_seconds),
                drive_app_server(
                    &mut stdin,
                    stdout,
                    request,
                    resume_thread_id,
                    sandbox_mode,
                    approval_handler.as_ref(),
                ),
            ) => Some(result),
            _ = wait_for_cancellation(request.cancellation_handler.as_ref()) => None,
        };
        let mut outcome = match drive_result {
            Some(Ok(Ok(outcome))) => outcome,
            Some(Ok(Err(error))) => ProcessOutcome {
                status: CodexRunStatus::Failed,
                thread_id: resume_thread_id.map(str::to_string),
                exit_code: Some(1),
                final_message: None,
                error_message: Some(truncate(&sanitize_error(&error.to_string()), 2_000)),
                turn_started: false,
            },
            Some(Err(_)) => ProcessOutcome {
                status: CodexRunStatus::TimedOut,
                thread_id: resume_thread_id.map(str::to_string),
                exit_code: None,
                final_message: None,
                error_message: Some(format!(
                    "Codex run exceeded {} seconds while waiting for completion or approval",
                    request.max_run_seconds
                )),
                turn_started: true,
            },
            None => ProcessOutcome {
                status: CodexRunStatus::Cancelled,
                thread_id: resume_thread_id.map(str::to_string),
                exit_code: None,
                final_message: None,
                error_message: Some("Codex run cancelled because the project was paused".into()),
                turn_started: true,
            },
        };

        drop(stdin);
        let exit_status = match timeout(Duration::from_secs(3), child.wait()).await {
            Ok(result) => Some(result.map_err(process_error)?),
            Err(_) => {
                terminate_process_tree(process_id);
                match timeout(Duration::from_secs(3), child.wait()).await {
                    Ok(result) => Some(result.map_err(process_error)?),
                    Err(_) => {
                        kill_process_tree(process_id);
                        Some(child.wait().await.map_err(process_error)?)
                    }
                }
            }
        };
        let stderr = stderr_task.await.map_err(join_error)??;
        if outcome.status == CodexRunStatus::Failed
            && outcome.error_message.is_none()
            && !stderr.trim().is_empty()
        {
            outcome.error_message = Some(truncate(&sanitize_error(&stderr), 2_000));
        }
        if outcome.exit_code.is_none() {
            outcome.exit_code = exit_status.and_then(|status| status.code());
        }
        Ok(outcome)
    }

    pub(super) fn apply_profile_arguments(
        &self,
        command: &mut Command,
        codex_profile: &str,
    ) -> AppResult<()> {
        validate_config_key(codex_profile, "Codex profile")?;
        if codex_profile.starts_with("relay_") {
            managed_profile_id(codex_profile).ok_or_else(|| {
                AppError::Validation("managed Codex profile selector is invalid".into())
            })?;
        } else if codex_profile != "default" {
            command.arg("--profile").arg(codex_profile);
        }
        Ok(())
    }

    pub(super) fn apply_profile_environment(
        &self,
        command: &mut Command,
        codex_profile: &str,
    ) -> AppResult<()> {
        if codex_profile.starts_with("relay_") {
            let profile_id = managed_profile_id(codex_profile).ok_or_else(|| {
                AppError::Validation("managed Codex profile selector is invalid".into())
            })?;
            command.env(
                "CODEX_HOME",
                self.managed_profile_homes_root.join(profile_id.to_string()),
            );
        }
        Ok(())
    }

    fn codex_home_for_selector(&self, target_selector: &str) -> AppResult<Option<PathBuf>> {
        if target_selector.starts_with("relay_") {
            let profile_id = managed_profile_id(target_selector).ok_or_else(|| {
                AppError::Validation("managed Codex profile selector is invalid".into())
            })?;
            return Ok(Some(
                self.managed_profile_homes_root.join(profile_id.to_string()),
            ));
        }
        if target_selector != "default" {
            return Err(AppError::Validation(
                "Codex MCP target environment is invalid".into(),
            ));
        }
        Ok(self
            .inherited_environment
            .get("CODEX_HOME")
            .filter(|value| !value.trim().is_empty())
            .map(PathBuf::from)
            .or_else(|| {
                self.inherited_environment
                    .get("HOME")
                    .filter(|value| !value.trim().is_empty())
                    .map(|home| PathBuf::from(home).join(".codex"))
            }))
    }

    pub(super) fn configured_mcp_server_names(
        &self,
        target_selector: &str,
    ) -> AppResult<std::collections::HashSet<String>> {
        let Some(codex_home) = self.codex_home_for_selector(target_selector)? else {
            return Ok(std::collections::HashSet::new());
        };
        let config_path = codex_home.join("config.toml");
        let Ok(metadata) = std::fs::metadata(&config_path) else {
            return Ok(std::collections::HashSet::new());
        };
        if metadata.len() > 2 * 1024 * 1024 {
            return Ok(std::collections::HashSet::new());
        }
        let Ok(content) = std::fs::read_to_string(config_path) else {
            return Ok(std::collections::HashSet::new());
        };
        Ok(content
            .lines()
            .map(str::trim)
            .filter(|line| line.starts_with('[') && line.ends_with(']'))
            .filter_map(|line| first_section_name(&line[1..line.len() - 1], "mcp_servers."))
            .collect())
    }
}
