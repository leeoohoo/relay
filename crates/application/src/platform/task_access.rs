use super::*;
use ai_chat_domain::company::{
    project_task_dependency_satisfied, AGENT_CODEX_WAKE_REASON_TASK_READY,
    PROJECT_TASK_DEPENDENCY_SUCCESS,
};

impl<R: PlatformRepository, V: OwnershipProofVerifier> PlatformApp<R, V> {
    pub(super) fn ensure_company_project_access(
        &self,
        company_id: Uuid,
        project_id: Uuid,
        actor_agent_id: Uuid,
    ) -> AppResult<CompanyProject> {
        let membership =
            self.ensure_active_company_conversation_member(company_id, actor_agent_id)?;
        let project = self
            .repo
            .get_company_project_result(project_id)?
            .filter(|project| project.company_id == company_id)
            .ok_or_else(|| AppError::NotFound("company project not found".into()))?;
        let is_project_member = self
            .repo
            .get_company_project_member(project.id, actor_agent_id)
            .is_some_and(|member| member.left_at.is_none());
        let can_manage = membership
            .permissions
            .iter()
            .any(|permission| permission == COMPANY_PERMISSION_PROJECT_MANAGE);
        if !is_project_member && !can_manage {
            return Err(AppError::Unauthorized(
                "agent is not an active project member".into(),
            ));
        }
        Ok(project)
    }

    pub(super) fn ensure_project_conversation_not_paused(
        &self,
        context: &ConversationContext,
    ) -> AppResult<()> {
        let project_is_paused = if context.context_type == CONVERSATION_CONTEXT_PROJECT_GROUP {
            match context.project_id {
                Some(project_id) => self
                    .repo
                    .get_company_project_result(project_id)?
                    .is_some_and(|project| project.status == PROJECT_STATUS_PAUSED),
                None => false,
            }
        } else {
            false
        };
        if project_is_paused {
            return Err(AppError::Conflict(
                "project is paused; its group cannot send messages".into(),
            ));
        }
        Ok(())
    }

    pub(super) fn ensure_project_not_paused(&self, project: &CompanyProject) -> AppResult<()> {
        if project.status == PROJECT_STATUS_PAUSED {
            return Err(AppError::Conflict(
                "project is paused; resume it before changing project work".into(),
            ));
        }
        Ok(())
    }

    pub(super) fn ensure_active_project_member(
        &self,
        project_id: Uuid,
        agent_id: Uuid,
    ) -> AppResult<CompanyProjectMember> {
        self.repo
            .get_company_project_member(project_id, agent_id)
            .filter(|member| member.left_at.is_none())
            .ok_or_else(|| AppError::Validation("assignee must be an active project member".into()))
    }

    pub(super) fn validate_project_task_dependency_selection(
        &self,
        project_id: Uuid,
        task_id: Uuid,
        task_status: &str,
        dependency_ids: Vec<Uuid>,
    ) -> AppResult<Vec<Uuid>> {
        let mut seen = HashSet::new();
        let mut normalized = Vec::new();
        for dependency_id in dependency_ids {
            if dependency_id == task_id {
                return Err(AppError::Validation(
                    "a project task cannot depend on itself".into(),
                ));
            }
            if seen.insert(dependency_id) {
                self.repo
                    .get_company_project_task(dependency_id)
                    .filter(|task| task.project_id == project_id)
                    .ok_or_else(|| {
                        AppError::NotFound("dependency project task not found".into())
                    })?;
                normalized.push(dependency_id);
            }
        }

        let existing = self.repo.list_company_project_task_dependencies(project_id);
        let current_ids = existing
            .iter()
            .filter(|dependency| dependency.task_id == task_id)
            .map(|dependency| dependency.depends_on_task_id)
            .collect::<HashSet<_>>();
        let desired_ids = normalized.iter().copied().collect::<HashSet<_>>();
        if matches!(
            task_status,
            PROJECT_TASK_STATUS_DONE | PROJECT_TASK_STATUS_FAILED | PROJECT_TASK_STATUS_CANCELLED
        ) && current_ids != desired_ids
        {
            return Err(AppError::Conflict(
                "dependencies cannot be changed on a terminal task".into(),
            ));
        }

        let mut graph = existing
            .into_iter()
            .filter(|dependency| dependency.task_id != task_id)
            .collect::<Vec<_>>();
        for dependency_id in &normalized {
            if project_task_dependency_would_cycle(&graph, task_id, *dependency_id) {
                return Err(AppError::Validation(
                    "project task dependency would create a cycle".into(),
                ));
            }
            graph.push(CompanyProjectTaskDependency {
                id: Uuid::new_v4(),
                project_id,
                task_id,
                depends_on_task_id: *dependency_id,
                dependency_condition: PROJECT_TASK_DEPENDENCY_SUCCESS.into(),
                created_by_agent_id: Some(Uuid::nil()),
                created_by_human_user_id: None,
                created_at: now_utc(),
            });
        }
        Ok(normalized)
    }

    pub(super) fn sync_project_task_dependencies_for_human(
        &self,
        project_id: Uuid,
        task_id: Uuid,
        dependency_ids: &[Uuid],
        human_user_id: Uuid,
    ) -> AppResult<()> {
        let existing = self
            .repo
            .list_company_project_task_dependencies(project_id)
            .into_iter()
            .filter(|dependency| dependency.task_id == task_id)
            .collect::<Vec<_>>();
        let desired_ids = dependency_ids.iter().copied().collect::<HashSet<_>>();
        for dependency in &existing {
            if !desired_ids.contains(&dependency.depends_on_task_id) {
                self.repo.remove_company_project_task_dependency(
                    project_id,
                    task_id,
                    dependency.depends_on_task_id,
                    None,
                    Some(human_user_id),
                )?;
            }
        }
        let existing_ids = existing
            .iter()
            .map(|dependency| dependency.depends_on_task_id)
            .collect::<HashSet<_>>();
        for dependency_id in dependency_ids {
            if !existing_ids.contains(dependency_id) {
                self.repo
                    .insert_company_project_task_dependency(CompanyProjectTaskDependency {
                        id: Uuid::new_v4(),
                        project_id,
                        task_id,
                        depends_on_task_id: *dependency_id,
                        dependency_condition: PROJECT_TASK_DEPENDENCY_SUCCESS.into(),
                        created_by_agent_id: None,
                        created_by_human_user_id: Some(human_user_id),
                        created_at: now_utc(),
                    })?;
            }
        }
        Ok(())
    }

    pub(super) fn ensure_project_task_dependency_ids_resolved(
        &self,
        dependency_ids: &[Uuid],
    ) -> AppResult<()> {
        let unresolved = dependency_ids
            .iter()
            .filter_map(|dependency_id| self.repo.get_company_project_task(*dependency_id))
            .filter(|dependency_task| {
                !project_task_dependency_satisfied(
                    PROJECT_TASK_DEPENDENCY_SUCCESS,
                    &dependency_task.status,
                )
            })
            .map(|dependency_task| dependency_task.title)
            .collect::<Vec<_>>();
        if unresolved.is_empty() {
            Ok(())
        } else {
            Err(AppError::Conflict(format!(
                "project task has unresolved dependencies: {}",
                unresolved.join(", ")
            )))
        }
    }

    pub(super) fn ensure_project_task_dependencies_resolved(
        &self,
        project_id: Uuid,
        task_id: Uuid,
    ) -> AppResult<()> {
        let unresolved = self
            .repo
            .list_company_project_task_dependencies(project_id)
            .into_iter()
            .filter(|dependency| dependency.task_id == task_id)
            .filter_map(|dependency| {
                self.repo
                    .get_company_project_task(dependency.depends_on_task_id)
                    .map(|dependency_task| (dependency, dependency_task))
            })
            .filter(|(dependency, dependency_task)| {
                !project_task_dependency_satisfied(
                    &dependency.dependency_condition,
                    &dependency_task.status,
                )
            })
            .map(|(_, dependency_task)| dependency_task.title)
            .collect::<Vec<_>>();
        if unresolved.is_empty() {
            Ok(())
        } else {
            Err(AppError::Conflict(format!(
                "project task has unresolved dependencies: {}",
                unresolved.join(", ")
            )))
        }
    }

    pub(super) fn notify_project_tasks_ready_after_changes(
        &self,
        project: &CompanyProject,
        changed_task_ids: &[Uuid],
        ready_at: DateTime<Utc>,
    ) -> AppResult<usize> {
        if changed_task_ids.is_empty() || project.status == PROJECT_STATUS_PAUSED {
            return Ok(0);
        }
        let changed_task_ids = changed_task_ids.iter().copied().collect::<HashSet<_>>();
        let tasks = self.repo.list_company_project_tasks_result(project.id)?;
        let tasks_by_id = tasks
            .iter()
            .map(|task| (task.id, task))
            .collect::<HashMap<_, _>>();
        let dependencies = self.repo.list_company_project_task_dependencies(project.id);
        let mut notified = 0;
        for task in tasks
            .iter()
            .filter(|task| task.status == PROJECT_TASK_STATUS_TODO)
        {
            let task_dependencies = dependencies
                .iter()
                .filter(|dependency| dependency.task_id == task.id)
                .collect::<Vec<_>>();
            if task_dependencies.is_empty()
                || !task_dependencies
                    .iter()
                    .any(|dependency| changed_task_ids.contains(&dependency.depends_on_task_id))
                || task_dependencies.iter().any(|dependency| {
                    tasks_by_id
                        .get(&dependency.depends_on_task_id)
                        .is_none_or(|dependency_task| {
                            !project_task_dependency_satisfied(
                                &dependency.dependency_condition,
                                &dependency_task.status,
                            )
                        })
                })
            {
                continue;
            }
            if !self.project_task_gate_requirements_satisfied(project.id, task.id) {
                continue;
            }
            let Some(assignee_agent_id) = task.assignee_agent_id else {
                continue;
            };
            if self
                .repo
                .get_company_project_member(project.id, assignee_agent_id)
                .is_none_or(|member| member.left_at.is_some())
            {
                continue;
            }
            self.enqueue_agent_event(
                assignee_agent_id,
                "company.project.task_ready",
                json!({
                    "company_id": project.company_id,
                    "project_id": project.id,
                    "project_name": project.name,
                    "task_id": task.id,
                    "task_title": task.title,
                    "unlocked_by_task_ids": changed_task_ids,
                }),
                50,
            )?;
            self.repo.request_agent_codex_trigger_wake(
                assignee_agent_id,
                ready_at,
                AGENT_CODEX_WAKE_REASON_TASK_READY,
            )?;
            notified += 1;
        }
        Ok(notified)
    }

    pub(super) fn company_project_view(
        &self,
        project: CompanyProject,
    ) -> AppResult<CompanyProjectView> {
        let members = self
            .repo
            .list_company_project_members(project.id)
            .into_iter()
            .filter(|member| member.left_at.is_none())
            .map(|member| {
                let agent_profile = self
                    .repo
                    .get_agent_profile(member.agent_profile_id)
                    .ok_or_else(|| AppError::NotFound("project member Agent not found".into()))?;
                Ok(CompanyProjectMemberView {
                    member,
                    agent_profile,
                })
            })
            .collect::<AppResult<Vec<_>>>()?;
        let preview = self
            .repo
            .list_agent_conversations(project.owner_agent_id)
            .into_iter()
            .find(|preview| preview.id == project.project_group_conversation_id)
            .ok_or_else(|| AppError::NotFound("project group conversation not found".into()))?;
        let context = self
            .repo
            .get_conversation_context_result(project.project_group_conversation_id)?
            .ok_or_else(|| AppError::NotFound("project group context not found".into()))?;
        Ok(CompanyProjectView {
            project: project.clone(),
            git: self
                .repo
                .get_company_project_git_config(project.id)
                .map(company_project_git_view),
            rule: self.repo.get_company_project_rule(project.id),
            assets: self.repo.list_company_project_assets(project.id),
            asset_refresh: self
                .repo
                .get_company_project_asset_refresh_config(project.id),
            members,
            tasks: self.repo.list_company_project_tasks_result(project.id)?,
            task_dependencies: self.repo.list_company_project_task_dependencies(project.id),
            task_status_history: self
                .repo
                .list_company_project_task_status_history(project.id),
            status_updates: self.repo.list_company_project_status_updates(project.id),
            project_group: CompanyConversationView {
                preview,
                context,
                member_agent_ids: self
                    .repo
                    .list_conversation_member_ids(project.project_group_conversation_id),
            },
        })
    }

    pub(super) fn ensure_company_project_for_human_manager(
        &self,
        human_user_id: Uuid,
        company_id: Uuid,
        project_id: Uuid,
    ) -> AppResult<CompanyProject> {
        self.ensure_company_human_manager(company_id, human_user_id)?;
        self.repo
            .get_company_project_result(project_id)?
            .filter(|project| project.company_id == company_id)
            .ok_or_else(|| AppError::NotFound("company project not found".into()))
    }

    pub(super) fn ensure_company_agent_for_human_manager(
        &self,
        human_user_id: Uuid,
        company_id: Uuid,
        agent_id: Uuid,
    ) -> AppResult<CompanyAgentMembership> {
        self.ensure_company_human_manager(company_id, human_user_id)?;
        self.repo
            .get_company_agent_membership(agent_id)
            .filter(|membership| {
                membership.company_id == company_id && membership.employment_status == "active"
            })
            .ok_or_else(|| AppError::NotFound("active company Agent not found".into()))
    }

    pub(super) fn set_company_agent_codex_trigger_status_for_human(
        &self,
        input: SetCompanyAgentCodexTriggerStatusForHumanInput,
        status: &str,
    ) -> AppResult<CompanyAgentCodexTriggerView> {
        self.ensure_company_agent_for_human_manager(
            input.human_user_id,
            input.company_id,
            input.agent_id,
        )?;
        let mut config = self
            .repo
            .get_agent_codex_trigger_config_by_agent_result(input.agent_id)?
            .ok_or_else(|| AppError::NotFound("Codex trigger config not found".into()))?;
        let now = now_utc();
        config.status = status.into();
        config.updated_by_human_user_id = Some(input.human_user_id);
        config.updated_at = now;
        if status == AGENT_CODEX_TRIGGER_STATUS_ACTIVE {
            config.next_run_at = now;
            config.last_error = None;
            config.consecutive_failure_count = 0;
        } else {
            config.manual_run_requested_at = None;
        }
        self.repo.save_agent_codex_trigger_config(config.clone())?;
        Ok(CompanyAgentCodexTriggerView {
            recent_runs: self
                .repo
                .list_agent_codex_trigger_runs_result(input.agent_id, 20)?,
            runner_profile_id: self
                .repo
                .get_agent_codex_runner_profile_assignment(input.agent_id),
            config,
        })
    }

    pub(super) fn ensure_active_company_conversation_member(
        &self,
        company_id: Uuid,
        agent_id: Uuid,
    ) -> AppResult<CompanyAgentMembership> {
        let membership = self
            .repo
            .get_company_agent_membership(agent_id)
            .filter(|membership| membership.company_id == company_id)
            .ok_or_else(|| AppError::Unauthorized("agent does not belong to the company".into()))?;
        if membership.employment_status == "provisioning" {
            return Err(AppError::Conflict(
                "requires_activation: this legacy Agent is still provisioning; wait for activation, then retry the same request. Newly hired Agents are activated automatically"
                    .into(),
            ));
        }
        if membership.employment_status != "active" {
            return Err(AppError::Unauthorized(
                "conversation member is not active in the company".into(),
            ));
        }
        let agent = self
            .repo
            .get_agent_profile(agent_id)
            .filter(|profile| matches!(profile.status, AgentStatus::Active))
            .ok_or_else(|| {
                AppError::Unauthorized("conversation member agent is not active".into())
            })?;
        if agent.id != membership.agent_profile_id {
            return Err(AppError::Unauthorized(
                "conversation member identity does not match membership".into(),
            ));
        }
        Ok(membership)
    }
}
