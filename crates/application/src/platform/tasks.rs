use super::*;

impl<R: PlatformRepository, V: OwnershipProofVerifier> PlatformApp<R, V> {
    pub fn add_company_project_member(
        &self,
        input: AddCompanyProjectMemberInput,
    ) -> AppResult<CompanyProjectView> {
        self.ensure_agent_can_act(input.actor_agent_id)?;
        self.ensure_company_agent_permission(
            input.company_id,
            input.actor_agent_id,
            COMPANY_PERMISSION_PROJECT_MANAGE,
        )?;
        let project = self.ensure_company_project_access(
            input.company_id,
            input.project_id,
            input.actor_agent_id,
        )?;
        self.ensure_project_not_paused(&project)?;
        let governance = self.effective_company_governance_policy_settings(input.company_id);
        self.ensure_active_company_conversation_member(input.company_id, input.target_agent_id)?;
        if self
            .repo
            .get_company_project_member(project.id, input.target_agent_id)
            .is_some_and(|member| member.left_at.is_none())
        {
            return Err(AppError::Conflict(
                "agent is already an active project member".into(),
            ));
        }
        if self
            .repo
            .list_company_project_members(project.id)
            .into_iter()
            .filter(|member| member.left_at.is_none())
            .count()
            >= governance.max_project_members as usize
        {
            return Err(AppError::Validation(format!(
                "project supports at most {} active members",
                governance.max_project_members
            )));
        }
        let now = now_utc();
        let existing = self
            .repo
            .get_company_project_member(project.id, input.target_agent_id);
        let member = CompanyProjectMember {
            id: existing
                .as_ref()
                .map(|member| member.id)
                .unwrap_or_else(Uuid::new_v4),
            project_id: project.id,
            agent_profile_id: input.target_agent_id,
            role: PROJECT_MEMBER_ROLE_MEMBER.into(),
            joined_at: now,
            left_at: None,
            added_by_agent_id: input.actor_agent_id,
        };
        self.repo
            .complete_company_project_member_add(CompanyProjectMemberAddBundle {
                member,
                conversation_preview: CompanyConversationMemberPreview {
                    agent_id: input.target_agent_id,
                    preview: ConversationPreview {
                        id: project.project_group_conversation_id,
                        title: format!("项目 · {}", project.name),
                        conversation_type: ConversationType::Group,
                        last_message_preview: None,
                        updated_at: now,
                    },
                },
            })?;
        let _ = self.enqueue_agent_event(
            input.target_agent_id,
            "company.project.member_added",
            json!({
                "company_id": input.company_id,
                "project_id": project.id,
                "project_name": project.name,
                "conversation_id": project.project_group_conversation_id,
                "added_by_agent_id": input.actor_agent_id,
            }),
            35,
        );
        self.company_project_view(project)
    }

    pub fn remove_company_project_member(
        &self,
        input: RemoveCompanyProjectMemberInput,
    ) -> AppResult<CompanyProjectView> {
        self.ensure_agent_can_act(input.actor_agent_id)?;
        self.ensure_company_agent_permission(
            input.company_id,
            input.actor_agent_id,
            COMPANY_PERMISSION_PROJECT_MANAGE,
        )?;
        let project = self.ensure_company_project_access(
            input.company_id,
            input.project_id,
            input.actor_agent_id,
        )?;
        self.ensure_project_not_paused(&project)?;
        if input.target_agent_id == project.owner_agent_id {
            return Err(AppError::Validation(
                "project owner cannot be removed from the project".into(),
            ));
        }
        self.repo
            .get_company_project_member(project.id, input.target_agent_id)
            .filter(|member| member.left_at.is_none())
            .ok_or_else(|| AppError::NotFound("active project member not found".into()))?;
        self.repo.complete_company_project_member_remove(
            project.id,
            input.target_agent_id,
            project.project_group_conversation_id,
            now_utc(),
        )?;
        let _ = self.enqueue_agent_event(
            input.target_agent_id,
            "company.project.member_removed",
            json!({
                "company_id": input.company_id,
                "project_id": project.id,
                "project_name": project.name,
                "removed_by_agent_id": input.actor_agent_id,
            }),
            35,
        );
        self.company_project_view(project)
    }

    pub fn create_company_project_task(
        &self,
        input: CreateCompanyProjectTaskInput,
    ) -> AppResult<CompanyProjectTask> {
        self.ensure_agent_can_act(input.actor_agent_id)?;
        self.ensure_company_agent_permission(
            input.company_id,
            input.actor_agent_id,
            COMPANY_PERMISSION_TASK_ASSIGN,
        )?;
        let project = self.ensure_company_project_access(
            input.company_id,
            input.project_id,
            input.actor_agent_id,
        )?;
        self.ensure_project_not_paused(&project)?;
        let title = normalize_project_task_title(&input.title)?;
        let description = normalize_optional_text(input.description).unwrap_or_default();
        if description.chars().count() > 4000 {
            return Err(AppError::Validation(
                "project task description must not exceed 4000 characters".into(),
            ));
        }
        let priority = normalize_project_task_priority(input.priority.as_deref())?;
        if let Some(assignee_agent_id) = input.assignee_agent_id {
            self.ensure_active_project_member(project.id, assignee_agent_id)?;
        }
        let now = now_utc();
        let task = CompanyProjectTask {
            id: Uuid::new_v4(),
            project_id: project.id,
            title,
            description,
            status: PROJECT_TASK_STATUS_TODO.into(),
            priority,
            assignee_agent_id: input.assignee_agent_id,
            created_by_agent_id: Some(input.actor_agent_id),
            created_by_human_user_id: None,
            updated_by_agent_id: None,
            updated_by_human_user_id: None,
            due_at: input.due_at,
            completed_at: None,
            created_at: now,
            updated_at: now,
        };
        self.repo.insert_company_project_task(task.clone())?;
        if let Some(assignee_agent_id) = task.assignee_agent_id {
            let _ = self.enqueue_agent_event(
                assignee_agent_id,
                "company.project.task_assigned",
                json!({
                    "company_id": input.company_id,
                    "project_id": project.id,
                    "project_name": project.name,
                    "task_id": task.id,
                    "task_title": task.title,
                    "assigned_by_agent_id": input.actor_agent_id,
                }),
                45,
            );
        }
        Ok(task)
    }

    pub fn create_company_project_task_for_human(
        &self,
        input: CreateCompanyProjectTaskForHumanInput,
    ) -> AppResult<CompanyProjectTask> {
        let project = self.ensure_company_project_for_human_manager(
            input.human_user_id,
            input.company_id,
            input.project_id,
        )?;
        self.ensure_project_not_paused(&project)?;
        let title = normalize_project_task_title(&input.title)?;
        let description = normalize_optional_text(input.description).unwrap_or_default();
        if description.chars().count() > 4000 {
            return Err(AppError::Validation(
                "project task description must not exceed 4000 characters".into(),
            ));
        }
        let priority = normalize_project_task_priority(input.priority.as_deref())?;
        if let Some(assignee_agent_id) = input.assignee_agent_id {
            self.ensure_active_project_member(project.id, assignee_agent_id)?;
        }
        let now = now_utc();
        let task_id = Uuid::new_v4();
        let dependency_ids = self.validate_project_task_dependency_selection(
            project.id,
            task_id,
            PROJECT_TASK_STATUS_TODO,
            input.depends_on_task_ids,
        )?;
        let task = CompanyProjectTask {
            id: task_id,
            project_id: project.id,
            title,
            description,
            status: PROJECT_TASK_STATUS_TODO.into(),
            priority,
            assignee_agent_id: input.assignee_agent_id,
            created_by_agent_id: None,
            created_by_human_user_id: Some(input.human_user_id),
            updated_by_agent_id: None,
            updated_by_human_user_id: None,
            due_at: input.due_at,
            completed_at: None,
            created_at: now,
            updated_at: now,
        };
        self.repo.insert_company_project_task(task.clone())?;
        self.sync_project_task_dependencies_for_human(
            project.id,
            task.id,
            &dependency_ids,
            input.human_user_id,
        )?;
        if let Some(assignee_agent_id) = task.assignee_agent_id {
            let _ = self.enqueue_agent_event(
                assignee_agent_id,
                "company.project.task_assigned",
                json!({
                    "company_id": input.company_id,
                    "project_id": project.id,
                    "project_name": project.name,
                    "task_id": task.id,
                    "task_title": task.title,
                    "assigned_by_human_user_id": input.human_user_id,
                }),
                45,
            );
        }
        Ok(task)
    }

    pub fn update_company_project_task(
        &self,
        input: UpdateCompanyProjectTaskInput,
    ) -> AppResult<CompanyProjectTask> {
        self.ensure_agent_can_act(input.actor_agent_id)?;
        let membership =
            self.ensure_active_company_conversation_member(input.company_id, input.actor_agent_id)?;
        let project = self.ensure_company_project_access(
            input.company_id,
            input.project_id,
            input.actor_agent_id,
        )?;
        self.ensure_project_not_paused(&project)?;
        let mut task = self
            .repo
            .get_company_project_task(input.task_id)
            .filter(|task| task.project_id == project.id)
            .ok_or_else(|| AppError::NotFound("project task not found".into()))?;
        let can_manage = membership
            .permissions
            .iter()
            .any(|permission| permission == COMPANY_PERMISSION_TASK_ASSIGN);
        if !can_manage {
            if !membership
                .permissions
                .iter()
                .any(|permission| permission == COMPANY_PERMISSION_TASK_UPDATE)
                || task.assignee_agent_id != Some(input.actor_agent_id)
            {
                return Err(AppError::Unauthorized(
                    "only project managers or the assigned Agent can update this task".into(),
                ));
            }
            if input.title.is_some()
                || input.description.is_some()
                || input.priority.is_some()
                || input.assignee_agent_id.is_some()
                || input.due_at.is_some()
            {
                return Err(AppError::Unauthorized(
                    "assigned Agents may only update task status".into(),
                ));
            }
            if let Some(status) = input.status.as_deref() {
                let status = normalize_project_task_status(status)?;
                if !matches!(
                    status.as_str(),
                    PROJECT_TASK_STATUS_IN_PROGRESS
                        | PROJECT_TASK_STATUS_BLOCKED
                        | PROJECT_TASK_STATUS_DONE
                        | PROJECT_TASK_STATUS_FAILED
                ) {
                    return Err(AppError::Unauthorized(
                        "assigned Agents may only mark tasks in progress, blocked, done, or failed"
                            .into(),
                    ));
                }
            }
        }
        if let Some(title) = input.title {
            task.title = normalize_project_task_title(&title)?;
        }
        if let Some(description) = input.description {
            let description = description.trim().to_string();
            if description.chars().count() > 4000 {
                return Err(AppError::Validation(
                    "project task description must not exceed 4000 characters".into(),
                ));
            }
            task.description = description;
        }
        let mut status_changed = false;
        if let Some(status) = input.status {
            let status = normalize_project_task_status(&status)?;
            if matches!(
                status.as_str(),
                PROJECT_TASK_STATUS_IN_PROGRESS | PROJECT_TASK_STATUS_DONE
            ) {
                self.ensure_project_task_dependencies_resolved(project.id, task.id)?;
                self.ensure_project_task_gates_satisfied(project.id, task.id)?;
            }
            status_changed = task.status != status;
            task.status = status;
        }
        if let Some(priority) = input.priority {
            task.priority = normalize_project_task_priority(Some(&priority))?;
        }
        if let Some(assignee_agent_id) = input.assignee_agent_id {
            self.ensure_active_project_member(project.id, assignee_agent_id)?;
            task.assignee_agent_id = Some(assignee_agent_id);
        }
        if let Some(due_at) = input.due_at {
            task.due_at = Some(due_at);
        }
        let now = now_utc();
        task.updated_at = now;
        task.updated_by_agent_id = Some(input.actor_agent_id);
        task.updated_by_human_user_id = None;
        if status_changed {
            task.completed_at = matches!(
                task.status.as_str(),
                PROJECT_TASK_STATUS_DONE | PROJECT_TASK_STATUS_FAILED
            )
            .then_some(now);
        }
        self.repo.update_company_project_task(task.clone())?;
        if status_changed
            && matches!(
                task.status.as_str(),
                PROJECT_TASK_STATUS_DONE
                    | PROJECT_TASK_STATUS_FAILED
                    | PROJECT_TASK_STATUS_CANCELLED
            )
        {
            self.notify_project_tasks_ready_after_changes(&project, &[task.id], now)?;
        }
        Ok(task)
    }

    pub fn update_company_project_task_for_human(
        &self,
        input: UpdateCompanyProjectTaskForHumanInput,
    ) -> AppResult<CompanyProjectTask> {
        let project = self.ensure_company_project_for_human_manager(
            input.human_user_id,
            input.company_id,
            input.project_id,
        )?;
        self.ensure_project_not_paused(&project)?;
        let mut task = self
            .repo
            .get_company_project_task(input.task_id)
            .filter(|task| task.project_id == project.id)
            .ok_or_else(|| AppError::NotFound("project task not found".into()))?;
        let current_dependency_ids = self
            .repo
            .list_company_project_task_dependencies(project.id)
            .into_iter()
            .filter(|dependency| dependency.task_id == task.id)
            .map(|dependency| dependency.depends_on_task_id)
            .collect::<Vec<_>>();
        let dependency_ids = match input.depends_on_task_ids {
            Some(dependency_ids) => self.validate_project_task_dependency_selection(
                project.id,
                task.id,
                &task.status,
                dependency_ids,
            )?,
            None => current_dependency_ids,
        };
        if let Some(title) = input.title {
            task.title = normalize_project_task_title(&title)?;
        }
        if let Some(description) = input.description {
            let description = description.trim().to_string();
            if description.chars().count() > 4000 {
                return Err(AppError::Validation(
                    "project task description must not exceed 4000 characters".into(),
                ));
            }
            task.description = description;
        }
        let mut status_changed = false;
        if let Some(status) = input.status {
            let status = normalize_project_task_status(&status)?;
            if matches!(
                status.as_str(),
                PROJECT_TASK_STATUS_IN_PROGRESS | PROJECT_TASK_STATUS_DONE
            ) {
                self.ensure_project_task_dependency_ids_resolved(&dependency_ids)?;
                self.ensure_project_task_gates_satisfied(project.id, task.id)?;
            }
            status_changed = task.status != status;
            task.status = status;
        }
        if let Some(priority) = input.priority {
            task.priority = normalize_project_task_priority(Some(&priority))?;
        }
        let previous_assignee = task.assignee_agent_id;
        if input.clear_assignee {
            task.assignee_agent_id = None;
        } else if let Some(assignee_agent_id) = input.assignee_agent_id {
            self.ensure_active_project_member(project.id, assignee_agent_id)?;
            task.assignee_agent_id = Some(assignee_agent_id);
        }
        if input.clear_due_at {
            task.due_at = None;
        } else if let Some(due_at) = input.due_at {
            task.due_at = Some(due_at);
        }
        let now = now_utc();
        task.updated_at = now;
        task.updated_by_agent_id = None;
        task.updated_by_human_user_id = Some(input.human_user_id);
        if status_changed {
            task.completed_at = matches!(
                task.status.as_str(),
                PROJECT_TASK_STATUS_DONE | PROJECT_TASK_STATUS_FAILED
            )
            .then_some(now);
        }
        if matches!(
            task.status.as_str(),
            PROJECT_TASK_STATUS_IN_PROGRESS | PROJECT_TASK_STATUS_DONE
        ) {
            self.ensure_project_task_dependency_ids_resolved(&dependency_ids)?;
            self.ensure_project_task_gates_satisfied(project.id, task.id)?;
        }
        self.repo.update_company_project_task(task.clone())?;
        self.sync_project_task_dependencies_for_human(
            project.id,
            task.id,
            &dependency_ids,
            input.human_user_id,
        )?;
        if status_changed
            && matches!(
                task.status.as_str(),
                PROJECT_TASK_STATUS_DONE
                    | PROJECT_TASK_STATUS_FAILED
                    | PROJECT_TASK_STATUS_CANCELLED
            )
        {
            self.notify_project_tasks_ready_after_changes(&project, &[task.id], now)?;
        }
        if previous_assignee != task.assignee_agent_id {
            if let Some(assignee_agent_id) = task.assignee_agent_id {
                let _ = self.enqueue_agent_event(
                    assignee_agent_id,
                    "company.project.task_assigned",
                    json!({
                        "company_id": input.company_id,
                        "project_id": project.id,
                        "project_name": project.name,
                        "task_id": task.id,
                        "task_title": task.title,
                        "assigned_by_human_user_id": input.human_user_id,
                    }),
                    45,
                );
            }
        }
        Ok(task)
    }

    pub fn add_company_project_task_dependency(
        &self,
        input: ChangeCompanyProjectTaskDependencyInput,
    ) -> AppResult<CompanyProjectTaskDependency> {
        self.ensure_agent_can_act(input.actor_agent_id)?;
        let membership =
            self.ensure_active_company_conversation_member(input.company_id, input.actor_agent_id)?;
        if !membership
            .permissions
            .iter()
            .any(|permission| permission == COMPANY_PERMISSION_TASK_ASSIGN)
        {
            return Err(AppError::Unauthorized(
                "task assign permission is required".into(),
            ));
        }
        let project = self.ensure_company_project_access(
            input.company_id,
            input.project_id,
            input.actor_agent_id,
        )?;
        self.ensure_project_not_paused(&project)?;
        if input.task_id == input.depends_on_task_id {
            return Err(AppError::Validation(
                "a project task cannot depend on itself".into(),
            ));
        }
        let task = self
            .repo
            .get_company_project_task(input.task_id)
            .filter(|task| task.project_id == project.id)
            .ok_or_else(|| AppError::NotFound("project task not found".into()))?;
        let dependency_task = self
            .repo
            .get_company_project_task(input.depends_on_task_id)
            .filter(|dependency| dependency.project_id == project.id)
            .ok_or_else(|| AppError::NotFound("dependency project task not found".into()))?;
        let dependency_condition =
            normalize_project_task_dependency_condition(input.dependency_condition.as_deref())?;
        if matches!(
            task.status.as_str(),
            PROJECT_TASK_STATUS_DONE | PROJECT_TASK_STATUS_FAILED | PROJECT_TASK_STATUS_CANCELLED
        ) {
            return Err(AppError::Conflict(
                "dependencies cannot be added to a completed or cancelled task".into(),
            ));
        }
        if task.status == PROJECT_TASK_STATUS_IN_PROGRESS
            && !ai_chat_domain::company::project_task_dependency_satisfied(
                &dependency_condition,
                &dependency_task.status,
            )
        {
            return Err(AppError::Conflict(
                "an unresolved dependency cannot be added to an in-progress task; move the task back to todo before replanning"
                    .into(),
            ));
        }
        let existing = self.repo.list_company_project_task_dependencies(project.id);
        if existing.iter().any(|dependency| {
            dependency.task_id == input.task_id
                && dependency.depends_on_task_id == input.depends_on_task_id
        }) {
            return Err(AppError::Conflict(
                "project task dependency already exists".into(),
            ));
        }
        if project_task_dependency_would_cycle(&existing, input.task_id, input.depends_on_task_id) {
            return Err(AppError::Validation(
                "project task dependency would create a cycle".into(),
            ));
        }
        let dependency = CompanyProjectTaskDependency {
            id: Uuid::new_v4(),
            project_id: project.id,
            task_id: input.task_id,
            depends_on_task_id: input.depends_on_task_id,
            dependency_condition,
            created_by_agent_id: Some(input.actor_agent_id),
            created_by_human_user_id: None,
            created_at: now_utc(),
        };
        self.repo
            .insert_company_project_task_dependency(dependency.clone())?;
        Ok(dependency)
    }

    pub fn remove_company_project_task_dependency(
        &self,
        input: ChangeCompanyProjectTaskDependencyInput,
    ) -> AppResult<()> {
        self.ensure_agent_can_act(input.actor_agent_id)?;
        let membership =
            self.ensure_active_company_conversation_member(input.company_id, input.actor_agent_id)?;
        if !membership
            .permissions
            .iter()
            .any(|permission| permission == COMPANY_PERMISSION_TASK_ASSIGN)
        {
            return Err(AppError::Unauthorized(
                "task assign permission is required".into(),
            ));
        }
        let project = self.ensure_company_project_access(
            input.company_id,
            input.project_id,
            input.actor_agent_id,
        )?;
        self.ensure_project_not_paused(&project)?;
        if !self
            .repo
            .list_company_project_task_dependencies(project.id)
            .iter()
            .any(|dependency| {
                dependency.task_id == input.task_id
                    && dependency.depends_on_task_id == input.depends_on_task_id
            })
        {
            return Err(AppError::NotFound(
                "project task dependency not found".into(),
            ));
        }
        self.repo.remove_company_project_task_dependency(
            project.id,
            input.task_id,
            input.depends_on_task_id,
            Some(input.actor_agent_id),
            None,
        )
    }

    pub fn batch_update_company_project_tasks(
        &self,
        input: BatchUpdateCompanyProjectTasksInput,
    ) -> AppResult<Vec<CompanyProjectTask>> {
        self.ensure_agent_can_act(input.actor_agent_id)?;
        let membership =
            self.ensure_active_company_conversation_member(input.company_id, input.actor_agent_id)?;
        if !membership
            .permissions
            .iter()
            .any(|permission| permission == COMPANY_PERMISSION_TASK_ASSIGN)
        {
            return Err(AppError::Unauthorized(
                "task assign permission is required".into(),
            ));
        }
        let project = self.ensure_company_project_access(
            input.company_id,
            input.project_id,
            input.actor_agent_id,
        )?;
        self.ensure_project_not_paused(&project)?;
        let task_ids = input.task_ids.into_iter().collect::<HashSet<_>>();
        if task_ids.is_empty() || task_ids.len() > 50 {
            return Err(AppError::Validation(
                "task_ids must contain 1 to 50 unique project tasks".into(),
            ));
        }
        if input.assignee_agent_id.is_some() && input.clear_assignee {
            return Err(AppError::Validation(
                "assignee_agent_id and clear_assignee cannot be used together".into(),
            ));
        }
        if input.due_at.is_some() && input.clear_due_at {
            return Err(AppError::Validation(
                "due_at and clear_due_at cannot be used together".into(),
            ));
        }
        if input.status.is_none()
            && input.priority.is_none()
            && input.assignee_agent_id.is_none()
            && !input.clear_assignee
            && input.due_at.is_none()
            && !input.clear_due_at
        {
            return Err(AppError::Validation(
                "batch task update must change at least one field".into(),
            ));
        }
        let status = input
            .status
            .as_deref()
            .map(normalize_project_task_status)
            .transpose()?;
        let priority = input
            .priority
            .as_deref()
            .map(|priority| normalize_project_task_priority(Some(priority)))
            .transpose()?;
        if let Some(assignee_agent_id) = input.assignee_agent_id {
            self.ensure_active_project_member(project.id, assignee_agent_id)?;
        }
        let now = now_utc();
        let mut assignments = Vec::new();
        let mut dependency_unlock_task_ids = Vec::new();
        let mut tasks = Vec::with_capacity(task_ids.len());
        for task_id in task_ids {
            let mut task = self
                .repo
                .get_company_project_task(task_id)
                .filter(|task| task.project_id == project.id)
                .ok_or_else(|| AppError::NotFound("project task not found".into()))?;
            if let Some(status) = status.as_deref() {
                if matches!(
                    status,
                    PROJECT_TASK_STATUS_IN_PROGRESS | PROJECT_TASK_STATUS_DONE
                ) {
                    self.ensure_project_task_dependencies_resolved(project.id, task.id)?;
                    self.ensure_project_task_gates_satisfied(project.id, task.id)?;
                }
                if task.status != status {
                    task.status = status.to_string();
                    if matches!(
                        status,
                        PROJECT_TASK_STATUS_DONE
                            | PROJECT_TASK_STATUS_FAILED
                            | PROJECT_TASK_STATUS_CANCELLED
                    ) {
                        dependency_unlock_task_ids.push(task.id);
                    }
                    task.completed_at = matches!(
                        status,
                        PROJECT_TASK_STATUS_DONE | PROJECT_TASK_STATUS_FAILED
                    )
                    .then_some(now);
                }
            }
            if let Some(priority) = priority.as_deref() {
                task.priority = priority.to_string();
            }
            let previous_assignee = task.assignee_agent_id;
            if input.clear_assignee {
                task.assignee_agent_id = None;
            } else if let Some(assignee_agent_id) = input.assignee_agent_id {
                task.assignee_agent_id = Some(assignee_agent_id);
            }
            if previous_assignee != task.assignee_agent_id {
                if let Some(assignee_agent_id) = task.assignee_agent_id {
                    assignments.push((assignee_agent_id, task.id, task.title.clone()));
                }
            }
            if input.clear_due_at {
                task.due_at = None;
            } else if let Some(due_at) = input.due_at {
                task.due_at = Some(due_at);
            }
            task.updated_by_agent_id = Some(input.actor_agent_id);
            task.updated_by_human_user_id = None;
            task.updated_at = now;
            tasks.push(task);
        }
        self.repo.update_company_project_tasks(tasks.clone())?;
        self.notify_project_tasks_ready_after_changes(&project, &dependency_unlock_task_ids, now)?;
        for (assignee_agent_id, task_id, task_title) in assignments {
            let _ = self.enqueue_agent_event(
                assignee_agent_id,
                "company.project.task_assigned",
                json!({
                    "company_id": input.company_id,
                    "project_id": project.id,
                    "project_name": project.name,
                    "task_id": task_id,
                    "task_title": task_title,
                    "assigned_by_agent_id": input.actor_agent_id,
                    "assignment_mode": "batch",
                }),
                45,
            );
        }
        tasks.sort_by(|left, right| {
            right
                .updated_at
                .cmp(&left.updated_at)
                .then_with(|| left.id.cmp(&right.id))
        });
        Ok(tasks)
    }

    pub fn create_company_project_status_update(
        &self,
        input: CreateCompanyProjectStatusUpdateInput,
    ) -> AppResult<CompanyProjectStatusUpdate> {
        self.ensure_agent_can_act(input.actor_agent_id)?;
        let membership = self.ensure_company_agent_permission(
            input.company_id,
            input.actor_agent_id,
            COMPANY_PERMISSION_TASK_UPDATE,
        )?;
        let mut project = self.ensure_company_project_access(
            input.company_id,
            input.project_id,
            input.actor_agent_id,
        )?;
        self.ensure_project_not_paused(&project)?;
        let summary = input.summary.trim();
        if summary.is_empty() || summary.chars().count() > 2000 {
            return Err(AppError::Validation(
                "project status summary must contain 1 to 2000 characters".into(),
            ));
        }
        if !(0..=100).contains(&input.progress_percent) {
            return Err(AppError::Validation(
                "project progress_percent must be between 0 and 100".into(),
            ));
        }
        let blockers = normalize_project_status_items(input.blockers, "blockers")?;
        let next_steps = normalize_project_status_items(input.next_steps, "next_steps")?;
        let project_status = input
            .project_status
            .as_deref()
            .map(normalize_project_status)
            .transpose()?;
        if project_status.is_some()
            && !membership
                .permissions
                .iter()
                .any(|permission| permission == COMPANY_PERMISSION_PROJECT_MANAGE)
        {
            return Err(AppError::Unauthorized(
                "project.manage permission is required to change project status".into(),
            ));
        }
        let now = now_utc();
        if let Some(status) = project_status.as_ref() {
            project.status = status.clone();
            project.completed_at = (status == PROJECT_STATUS_COMPLETED).then_some(now);
        }
        project.updated_by_agent_id = Some(input.actor_agent_id);
        project.updated_at = now;
        self.repo.update_company_project(project.clone())?;
        let update = CompanyProjectStatusUpdate {
            id: Uuid::new_v4(),
            project_id: project.id,
            author_agent_id: input.actor_agent_id,
            summary: summary.to_string(),
            progress_percent: input.progress_percent,
            blockers,
            next_steps,
            project_status,
            created_at: now,
        };
        self.repo
            .insert_company_project_status_update(update.clone())?;
        for member in self.repo.list_company_project_members(project.id) {
            if member.left_at.is_none() && member.agent_profile_id != input.actor_agent_id {
                let _ = self.enqueue_agent_event(
                    member.agent_profile_id,
                    "company.project.status_updated",
                    json!({
                        "company_id": input.company_id,
                        "project_id": project.id,
                        "project_name": project.name,
                        "progress_percent": update.progress_percent,
                        "summary": update.summary,
                        "author_agent_id": input.actor_agent_id,
                    }),
                    30,
                );
            }
        }
        Ok(update)
    }
}
