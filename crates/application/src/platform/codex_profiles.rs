use super::*;

impl<R: PlatformRepository, V: OwnershipProofVerifier> PlatformApp<R, V> {
    pub fn list_company_codex_runner_profiles_for_human(
        &self,
        input: ListCompanyCodexRunnerProfilesForHumanInput,
    ) -> AppResult<Vec<CompanyCodexRunnerProfileView>> {
        self.ensure_company_human_manager(input.company_id, input.human_user_id)?;
        Ok(self
            .repo
            .list_company_codex_runner_profiles(input.company_id)
            .into_iter()
            .map(|profile| CompanyCodexRunnerProfileView {
                assigned_agent_count: self
                    .repo
                    .list_codex_runner_profile_agent_ids(profile.id)
                    .len(),
                profile,
            })
            .collect())
    }

    pub fn upsert_company_codex_runner_profile_for_human(
        &self,
        input: UpsertCompanyCodexRunnerProfileForHumanInput,
    ) -> AppResult<CompanyCodexRunnerProfileView> {
        self.ensure_company_human_manager(input.company_id, input.human_user_id)?;
        let name = input.name.trim();
        if name.is_empty() || name.chars().count() > 80 {
            return Err(AppError::Validation(
                "Codex runner profile name must contain 1 to 80 characters".into(),
            ));
        }
        if !(10..=604_800).contains(&input.interval_seconds) {
            return Err(AppError::Validation(
                "Codex runner profile interval_seconds must be between 10 and 604800".into(),
            ));
        }
        if !(60..=7_200).contains(&input.max_run_seconds) {
            return Err(AppError::Validation(
                "Codex runner profile max_run_seconds must be between 60 and 7200".into(),
            ));
        }
        if !matches!(
            input.sandbox_mode.as_str(),
            AGENT_CODEX_SETTING_INHERIT
                | AGENT_CODEX_SANDBOX_READ_ONLY
                | AGENT_CODEX_SANDBOX_WORKSPACE_WRITE
        ) {
            return Err(AppError::Validation(
                "Codex runner profile sandbox_mode must be inherit, read_only, or workspace_write"
                    .into(),
            ));
        }
        if !matches!(
            input.approval_policy.as_str(),
            AGENT_CODEX_SETTING_INHERIT
                | AGENT_CODEX_APPROVAL_POLICY_NEVER
                | AGENT_CODEX_APPROVAL_POLICY_ON_REQUEST
        ) {
            return Err(AppError::Validation(
                "Codex runner profile approval_policy must be inherit, never, or on-request".into(),
            ));
        }
        let codex_profile = validate_codex_profile_name(&input.codex_profile)?;
        let model = input
            .model
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        if model
            .as_ref()
            .is_some_and(|value| value.chars().count() > 128 || value.chars().any(char::is_control))
        {
            return Err(AppError::Validation(
                "Codex runner profile model must contain at most 128 safe characters".into(),
            ));
        }
        let reasoning_effort = validate_optional_codex_reasoning_effort(
            input.reasoning_effort,
            "Codex runner profile reasoning_effort",
        )?;
        let reasoning_summary = validate_optional_codex_choice(
            input.reasoning_summary,
            &["auto", "concise", "detailed", "none"],
            "Codex runner profile reasoning_summary",
        )?;
        let verbosity = validate_optional_codex_choice(
            input.verbosity,
            &["low", "medium", "high"],
            "Codex runner profile verbosity",
        )?;
        let personality = validate_optional_codex_choice(
            input.personality,
            &["none", "friendly", "pragmatic"],
            "Codex runner profile personality",
        )?;
        let existing = input
            .profile_id
            .map(|profile_id| {
                self.repo
                    .get_company_codex_runner_profile(profile_id)
                    .filter(|profile| profile.company_id == input.company_id)
                    .ok_or_else(|| AppError::NotFound("Codex runner profile not found".into()))
            })
            .transpose()?;
        let now = now_utc();
        let is_default = input.is_default
            || existing.as_ref().is_some_and(|profile| profile.is_default)
            || (existing.is_none()
                && self
                    .repo
                    .list_company_codex_runner_profiles(input.company_id)
                    .is_empty());
        let profile = CompanyCodexRunnerProfile {
            id: existing
                .as_ref()
                .map(|profile| profile.id)
                .unwrap_or_else(Uuid::new_v4),
            company_id: input.company_id,
            name: name.to_string(),
            interval_seconds: input.interval_seconds,
            codex_profile,
            model,
            reasoning_effort,
            reasoning_summary,
            verbosity,
            personality,
            service_tier: None,
            sandbox_mode: input.sandbox_mode,
            approval_policy: input.approval_policy,
            network_access: None,
            web_search: None,
            feature_multi_agent: None,
            feature_remote_plugin: None,
            feature_hooks: None,
            feature_goals: None,
            feature_shell_tool: None,
            max_run_seconds: input.max_run_seconds,
            is_default,
            created_by_human_user_id: existing
                .as_ref()
                .map(|profile| profile.created_by_human_user_id)
                .unwrap_or(input.human_user_id),
            updated_by_human_user_id: Some(input.human_user_id),
            created_at: existing
                .as_ref()
                .map(|profile| profile.created_at)
                .unwrap_or(now),
            updated_at: now,
        };
        if profile.is_default {
            self.repo
                .clear_company_codex_runner_profile_defaults(input.company_id, profile.id)?;
        }
        self.repo
            .save_company_codex_runner_profile(profile.clone())?;

        for agent_id in self.repo.list_codex_runner_profile_agent_ids(profile.id) {
            if let Some(mut config) = self
                .repo
                .get_agent_codex_trigger_config_by_agent_result(agent_id)?
            {
                config.interval_seconds = profile.interval_seconds;
                config.codex_profile = profile.codex_profile.clone();
                config.model = profile.model.clone();
                config.reasoning_effort = profile.reasoning_effort.clone();
                config.reasoning_summary = profile.reasoning_summary.clone();
                config.verbosity = profile.verbosity.clone();
                config.personality = profile.personality.clone();
                config.service_tier = None;
                config.sandbox_mode = profile.sandbox_mode.clone();
                config.approval_policy = profile.approval_policy.clone();
                config.network_access = None;
                config.web_search = None;
                config.feature_multi_agent = None;
                config.feature_remote_plugin = None;
                config.feature_hooks = None;
                config.feature_goals = None;
                config.feature_shell_tool = None;
                config.max_run_seconds = profile.max_run_seconds;
                config.updated_by_human_user_id = Some(input.human_user_id);
                config.updated_at = now;
                self.repo.save_agent_codex_trigger_config(config)?;
            }
        }

        Ok(CompanyCodexRunnerProfileView {
            assigned_agent_count: self
                .repo
                .list_codex_runner_profile_agent_ids(profile.id)
                .len(),
            profile,
        })
    }

    pub fn delete_company_codex_runner_profile_for_human(
        &self,
        input: DeleteCompanyCodexRunnerProfileForHumanInput,
    ) -> AppResult<()> {
        self.ensure_company_human_manager(input.company_id, input.human_user_id)?;
        self.repo
            .get_company_codex_runner_profile(input.profile_id)
            .filter(|profile| profile.company_id == input.company_id)
            .ok_or_else(|| AppError::NotFound("Codex runner profile not found".into()))?;
        if !self
            .repo
            .list_codex_runner_profile_agent_ids(input.profile_id)
            .is_empty()
        {
            return Err(AppError::Conflict(
                "Codex runner profile is still assigned to one or more Agents".into(),
            ));
        }
        self.repo
            .delete_company_codex_runner_profile(input.profile_id)
    }

    pub fn list_company_codex_plugins_for_human(
        &self,
        input: ListCompanyCodexPluginsForHumanInput,
    ) -> AppResult<CompanyCodexPluginsView> {
        self.ensure_company_human_manager(input.company_id, input.human_user_id)?;
        let mut allowed_selectors = self
            .repo
            .list_company_codex_runner_profiles(input.company_id)
            .into_iter()
            .map(|profile| profile.codex_profile)
            .collect::<std::collections::HashSet<_>>();
        allowed_selectors.insert("default".into());
        Ok(CompanyCodexPluginsView {
            catalogs: self
                .repo
                .list_codex_plugin_catalog_snapshots()?
                .into_iter()
                .filter(|catalog| allowed_selectors.contains(&catalog.target_selector))
                .collect(),
            operations: self.repo.list_company_codex_plugin_operations(
                input.company_id,
                input.operation_limit.clamp(1, 200),
            )?,
        })
    }

    pub fn request_codex_plugin_operation_for_human(
        &self,
        input: RequestCodexPluginOperationForHumanInput,
    ) -> AppResult<CodexPluginOperation> {
        self.ensure_company_human_manager(input.company_id, input.human_user_id)?;
        let target_runner_id = input.target_runner_id.trim();
        if target_runner_id.is_empty()
            || target_runner_id.chars().count() > 160
            || target_runner_id.chars().any(char::is_control)
        {
            return Err(AppError::Validation(
                "Codex plugin target runner is invalid".into(),
            ));
        }
        let target_selector = input.target_selector.trim();
        let selector_is_available = target_selector == "default"
            || self
                .repo
                .list_company_codex_runner_profiles(input.company_id)
                .into_iter()
                .any(|profile| profile.codex_profile == target_selector);
        if !selector_is_available {
            return Err(AppError::Validation(
                "Codex plugin authentication environment is unavailable".into(),
            ));
        }
        if !matches!(
            input.operation.as_str(),
            CODEX_PLUGIN_OPERATION_INSTALL
                | CODEX_PLUGIN_OPERATION_REMOVE
                | CODEX_PLUGIN_OPERATION_REFRESH
        ) {
            return Err(AppError::Validation(
                "Codex plugin operation must be install, remove, or refresh".into(),
            ));
        }
        let catalogs = self.repo.list_codex_plugin_catalog_snapshots()?;
        let catalog = catalogs
            .iter()
            .find(|catalog| {
                catalog.runner_id == target_runner_id && catalog.target_selector == target_selector
            })
            .ok_or_else(|| AppError::NotFound("Codex plugin runner not found".into()))?;
        let plugin_id = if input.operation == CODEX_PLUGIN_OPERATION_REFRESH {
            None
        } else {
            let plugin_id = validate_codex_plugin_id(input.plugin_id.as_deref())?;
            let source = if input.operation == CODEX_PLUGIN_OPERATION_INSTALL {
                &catalog.available
            } else {
                &catalog.installed
            };
            if !codex_plugin_catalog_contains(source, &plugin_id) {
                return Err(AppError::Conflict(format!(
                    "plugin `{plugin_id}` is not present in the current {} catalog",
                    if input.operation == CODEX_PLUGIN_OPERATION_INSTALL {
                        "available"
                    } else {
                        "installed"
                    }
                )));
            }
            Some(plugin_id)
        };
        let now = now_utc();
        let operation = CodexPluginOperation {
            id: Uuid::new_v4(),
            company_id: input.company_id,
            target_runner_id: target_runner_id.to_string(),
            target_selector: target_selector.to_string(),
            operation: input.operation,
            plugin_id,
            status: CODEX_PLUGIN_OPERATION_STATUS_QUEUED.into(),
            requested_by_human_user_id: input.human_user_id,
            lease_owner: None,
            lease_expires_at: None,
            attempt_count: 0,
            error_message: None,
            result: json!({}),
            requested_at: now,
            started_at: None,
            finished_at: None,
            updated_at: now,
        };
        self.repo.insert_codex_plugin_operation(operation.clone())?;
        Ok(operation)
    }

    pub fn save_codex_plugin_catalog_snapshot(
        &self,
        snapshot: CodexPluginCatalogSnapshot,
    ) -> AppResult<()> {
        self.repo.save_codex_plugin_catalog_snapshot(snapshot)
    }

    pub fn codex_plugin_fingerprint(&self, runner_id: &str) -> AppResult<Option<String>> {
        Ok(self
            .repo
            .list_codex_plugin_catalog_snapshots()?
            .into_iter()
            .find(|catalog| catalog.runner_id == runner_id)
            .map(|catalog| catalog.fingerprint))
    }

    pub fn claim_codex_plugin_operations(
        &self,
        target_runner_id: &str,
        lease_owner: &str,
        limit: usize,
    ) -> AppResult<Vec<CodexPluginOperation>> {
        if target_runner_id.trim().is_empty() || lease_owner.trim().is_empty() {
            return Err(AppError::Validation(
                "Codex plugin runner and lease owner are required".into(),
            ));
        }
        self.repo.claim_codex_plugin_operations(
            target_runner_id,
            lease_owner,
            now_utc(),
            limit.clamp(1, 10),
        )
    }

    pub fn finish_codex_plugin_operation(
        &self,
        operation_id: Uuid,
        lease_owner: &str,
        succeeded: bool,
        result: Value,
        error_message: Option<String>,
    ) -> AppResult<()> {
        self.repo.finish_codex_plugin_operation(
            operation_id,
            lease_owner,
            succeeded,
            result,
            error_message,
            now_utc(),
        )
    }

    pub fn get_company_agent_codex_trigger_for_human(
        &self,
        input: GetCompanyAgentCodexTriggerForHumanInput,
    ) -> AppResult<Option<CompanyAgentCodexTriggerView>> {
        self.ensure_company_agent_for_human_manager(
            input.human_user_id,
            input.company_id,
            input.agent_id,
        )?;
        let Some(config) = self
            .repo
            .get_agent_codex_trigger_config_by_agent_result(input.agent_id)?
        else {
            return Ok(None);
        };
        Ok(Some(CompanyAgentCodexTriggerView {
            recent_runs: self.recent_agent_codex_runs_for_human(input.agent_id, 20)?,
            active_intents: self.active_agent_execution_intents(input.agent_id),
            recent_sessions: self.recent_agent_codex_sessions_for_human(input.agent_id, 10),
            runner_profile_id: self
                .repo
                .get_agent_codex_runner_profile_assignment(input.agent_id),
            config,
        }))
    }

    pub fn upsert_company_agent_codex_trigger_for_human(
        &self,
        input: UpsertCompanyAgentCodexTriggerForHumanInput,
    ) -> AppResult<CompanyAgentCodexTriggerView> {
        self.ensure_company_agent_for_human_manager(
            input.human_user_id,
            input.company_id,
            input.agent_id,
        )?;
        let existing = self
            .repo
            .get_agent_codex_trigger_config_by_agent(input.agent_id);
        let runner_profile = input
            .runner_profile_id
            .map(|profile_id| {
                self.repo
                    .get_company_codex_runner_profile(profile_id)
                    .filter(|profile| profile.company_id == input.company_id)
                    .ok_or_else(|| AppError::NotFound("Codex runner profile not found".into()))
            })
            .transpose()?;
        let interval_seconds = runner_profile
            .as_ref()
            .map(|profile| profile.interval_seconds)
            .or(input.interval_seconds)
            .or_else(|| existing.as_ref().map(|config| config.interval_seconds))
            .unwrap_or(30);
        if !(10..=604_800).contains(&interval_seconds) {
            return Err(AppError::Validation(
                "Codex trigger interval_seconds must be between 10 and 604800".into(),
            ));
        }
        let max_run_seconds = runner_profile
            .as_ref()
            .map(|profile| profile.max_run_seconds)
            .or(input.max_run_seconds)
            .or_else(|| existing.as_ref().map(|config| config.max_run_seconds))
            .unwrap_or(3_600);
        if !(60..=7_200).contains(&max_run_seconds) {
            return Err(AppError::Validation(
                "Codex trigger max_run_seconds must be between 60 and 7200".into(),
            ));
        }
        let sandbox_mode = runner_profile
            .as_ref()
            .map(|profile| profile.sandbox_mode.clone())
            .or(input.sandbox_mode)
            .or_else(|| existing.as_ref().map(|config| config.sandbox_mode.clone()))
            .unwrap_or_else(|| AGENT_CODEX_SANDBOX_READ_ONLY.into());
        if !matches!(
            sandbox_mode.as_str(),
            AGENT_CODEX_SETTING_INHERIT
                | AGENT_CODEX_SANDBOX_READ_ONLY
                | AGENT_CODEX_SANDBOX_WORKSPACE_WRITE
        ) {
            return Err(AppError::Validation(
                "Codex trigger sandbox_mode must be inherit, read_only, or workspace_write".into(),
            ));
        }
        let approval_policy = runner_profile
            .as_ref()
            .map(|profile| profile.approval_policy.clone())
            .or(input.approval_policy)
            .or_else(|| {
                existing
                    .as_ref()
                    .map(|config| config.approval_policy.clone())
            })
            .unwrap_or_else(|| AGENT_CODEX_APPROVAL_POLICY_NEVER.into());
        if !matches!(
            approval_policy.as_str(),
            AGENT_CODEX_SETTING_INHERIT
                | AGENT_CODEX_APPROVAL_POLICY_NEVER
                | AGENT_CODEX_APPROVAL_POLICY_ON_REQUEST
        ) {
            return Err(AppError::Validation(
                "Codex trigger approval_policy must be inherit, never, or on-request".into(),
            ));
        }
        let default_profile = "default".to_string();
        let codex_profile = validate_codex_profile_name(
            runner_profile
                .as_ref()
                .map(|profile| profile.codex_profile.as_str())
                .or(input.codex_profile.as_deref())
                .or_else(|| {
                    existing
                        .as_ref()
                        .map(|config| config.codex_profile.as_str())
                })
                .unwrap_or(&default_profile),
        )?;
        let model = if let Some(profile) = runner_profile.as_ref() {
            profile.model.clone()
        } else {
            input
                .model
                .or_else(|| existing.as_ref().and_then(|config| config.model.clone()))
        };
        let reasoning_effort = validate_optional_codex_reasoning_effort(
            if let Some(profile) = runner_profile.as_ref() {
                profile.reasoning_effort.clone()
            } else {
                input.reasoning_effort.or_else(|| {
                    existing
                        .as_ref()
                        .and_then(|config| config.reasoning_effort.clone())
                })
            },
            "Codex trigger reasoning_effort",
        )?;
        let reasoning_summary = validate_optional_codex_choice(
            if let Some(profile) = runner_profile.as_ref() {
                profile.reasoning_summary.clone()
            } else {
                input.reasoning_summary.or_else(|| {
                    existing
                        .as_ref()
                        .and_then(|config| config.reasoning_summary.clone())
                })
            },
            &["auto", "concise", "detailed", "none"],
            "Codex trigger reasoning_summary",
        )?;
        let verbosity = validate_optional_codex_choice(
            if let Some(profile) = runner_profile.as_ref() {
                profile.verbosity.clone()
            } else {
                input.verbosity.or_else(|| {
                    existing
                        .as_ref()
                        .and_then(|config| config.verbosity.clone())
                })
            },
            &["low", "medium", "high"],
            "Codex trigger verbosity",
        )?;
        let personality = validate_optional_codex_choice(
            if let Some(profile) = runner_profile.as_ref() {
                profile.personality.clone()
            } else {
                input.personality.or_else(|| {
                    existing
                        .as_ref()
                        .and_then(|config| config.personality.clone())
                })
            },
            &["none", "friendly", "pragmatic"],
            "Codex trigger personality",
        )?;
        let now = now_utc();
        let config = AgentCodexTriggerConfig {
            id: existing
                .as_ref()
                .map(|config| config.id)
                .unwrap_or_else(Uuid::new_v4),
            company_id: input.company_id,
            agent_profile_id: input.agent_id,
            status: existing
                .as_ref()
                .map(|config| config.status.clone())
                .unwrap_or_else(|| AGENT_CODEX_TRIGGER_STATUS_ACTIVE.into()),
            interval_seconds,
            codex_profile,
            model,
            reasoning_effort,
            reasoning_summary,
            verbosity,
            personality,
            service_tier: None,
            sandbox_mode,
            approval_policy,
            network_access: None,
            web_search: None,
            feature_multi_agent: None,
            feature_remote_plugin: None,
            feature_hooks: None,
            feature_goals: None,
            feature_shell_tool: None,
            max_run_seconds,
            next_run_at: existing
                .as_ref()
                .map(|config| config.next_run_at)
                .unwrap_or(now),
            lease_owner: existing
                .as_ref()
                .and_then(|config| config.lease_owner.clone()),
            lease_expires_at: existing.as_ref().and_then(|config| config.lease_expires_at),
            manual_run_requested_at: existing
                .as_ref()
                .and_then(|config| config.manual_run_requested_at),
            wake_requested_at: existing
                .as_ref()
                .and_then(|config| config.wake_requested_at),
            wake_reason: existing
                .as_ref()
                .and_then(|config| config.wake_reason.clone()),
            last_run_at: existing.as_ref().and_then(|config| config.last_run_at),
            last_success_at: existing.as_ref().and_then(|config| config.last_success_at),
            last_error: existing
                .as_ref()
                .and_then(|config| config.last_error.clone()),
            consecutive_failure_count: existing
                .as_ref()
                .map(|config| config.consecutive_failure_count)
                .unwrap_or(0),
            created_by_human_user_id: existing
                .as_ref()
                .map(|config| config.created_by_human_user_id)
                .unwrap_or(input.human_user_id),
            updated_by_human_user_id: Some(input.human_user_id),
            created_at: existing
                .as_ref()
                .map(|config| config.created_at)
                .unwrap_or(now),
            updated_at: now,
        };
        self.repo.save_agent_codex_trigger_config(config.clone())?;
        if let Some(profile) = runner_profile {
            self.repo.assign_agent_codex_runner_profile(
                input.agent_id,
                profile.id,
                input.human_user_id,
                now,
            )?;
        }
        Ok(CompanyAgentCodexTriggerView {
            recent_runs: self.recent_agent_codex_runs_for_human(input.agent_id, 20)?,
            active_intents: self.active_agent_execution_intents(input.agent_id),
            recent_sessions: self.recent_agent_codex_sessions_for_human(input.agent_id, 10),
            runner_profile_id: self
                .repo
                .get_agent_codex_runner_profile_assignment(input.agent_id),
            config,
        })
    }
}
