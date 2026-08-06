use super::*;

impl TaskPlatformRepository for MemoryPlatformRepository {
    fn insert_company_project_task(&self, task: CompanyProjectTask) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if guard.company_project_tasks.contains_key(&task.id) {
            return Err(ai_chat_shared::AppError::Conflict(
                "company project task already exists".into(),
            ));
        }
        if let Some(project) = guard.company_projects.get_mut(&task.project_id) {
            project.updated_at = task.updated_at;
            project.updated_by_agent_id = task.created_by_agent_id;
        }
        guard
            .company_project_task_status_history
            .push(task_status_history_entry(None, &task));
        guard.company_project_tasks.insert(task.id, task);
        Ok(())
    }

    fn get_company_project_task(&self, task_id: Uuid) -> Option<CompanyProjectTask> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard.company_project_tasks.get(&task_id).cloned()
    }

    fn list_company_project_tasks(&self, project_id: Uuid) -> Vec<CompanyProjectTask> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut tasks = guard
            .company_project_tasks
            .values()
            .filter(|task| task.project_id == project_id)
            .cloned()
            .collect::<Vec<_>>();
        tasks.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        tasks
    }

    fn update_company_project_task(&self, task: CompanyProjectTask) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let Some(previous) = guard.company_project_tasks.get(&task.id).cloned() else {
            return Err(ai_chat_shared::AppError::NotFound(
                "company project task not found".into(),
            ));
        };
        if let Some(project) = guard.company_projects.get_mut(&task.project_id) {
            project.updated_at = task.updated_at;
            project.updated_by_agent_id = task.updated_by_agent_id;
        }
        if previous.status != task.status {
            guard
                .company_project_task_status_history
                .push(task_status_history_entry(Some(previous.status), &task));
        }
        guard.company_project_tasks.insert(task.id, task);
        Ok(())
    }

    fn update_company_project_tasks(&self, tasks: Vec<CompanyProjectTask>) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if tasks
            .iter()
            .any(|task| !guard.company_project_tasks.contains_key(&task.id))
        {
            return Err(ai_chat_shared::AppError::NotFound(
                "company project task not found".into(),
            ));
        }
        for task in tasks {
            let previous_status = guard
                .company_project_tasks
                .get(&task.id)
                .map(|previous| previous.status.clone());
            if let Some(project) = guard.company_projects.get_mut(&task.project_id) {
                project.updated_at = task.updated_at;
                project.updated_by_agent_id = task.updated_by_agent_id;
            }
            if previous_status.as_deref() != Some(task.status.as_str()) {
                guard
                    .company_project_task_status_history
                    .push(task_status_history_entry(previous_status, &task));
            }
            guard.company_project_tasks.insert(task.id, task);
        }
        Ok(())
    }

    fn list_company_project_task_status_history(
        &self,
        project_id: Uuid,
    ) -> Vec<CompanyProjectTaskStatusHistory> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut history = guard
            .company_project_task_status_history
            .iter()
            .filter(|entry| entry.project_id == project_id)
            .cloned()
            .collect::<Vec<_>>();
        history.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        history
    }

    fn insert_company_project_task_dependency(
        &self,
        dependency: CompanyProjectTaskDependency,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if guard
            .company_project_task_dependencies
            .values()
            .any(|existing| {
                existing.task_id == dependency.task_id
                    && existing.depends_on_task_id == dependency.depends_on_task_id
            })
        {
            return Err(ai_chat_shared::AppError::Conflict(
                "company project task dependency already exists".into(),
            ));
        }
        guard
            .company_project_task_dependencies
            .insert(dependency.id, dependency);
        Ok(())
    }

    fn remove_company_project_task_dependency(
        &self,
        project_id: Uuid,
        task_id: Uuid,
        depends_on_task_id: Uuid,
        _removed_by_agent_id: Option<Uuid>,
        _removed_by_human_user_id: Option<Uuid>,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let dependency_id = guard
            .company_project_task_dependencies
            .values()
            .find(|dependency| {
                dependency.project_id == project_id
                    && dependency.task_id == task_id
                    && dependency.depends_on_task_id == depends_on_task_id
            })
            .map(|dependency| dependency.id)
            .ok_or_else(|| {
                ai_chat_shared::AppError::NotFound(
                    "company project task dependency not found".into(),
                )
            })?;
        guard
            .company_project_task_dependencies
            .remove(&dependency_id);
        Ok(())
    }

    fn list_company_project_task_dependencies(
        &self,
        project_id: Uuid,
    ) -> Vec<CompanyProjectTaskDependency> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut dependencies = guard
            .company_project_task_dependencies
            .values()
            .filter(|dependency| dependency.project_id == project_id)
            .cloned()
            .collect::<Vec<_>>();
        dependencies.sort_by(|left, right| {
            left.created_at
                .cmp(&right.created_at)
                .then_with(|| left.id.cmp(&right.id))
        });
        dependencies
    }

    fn insert_company_project_status_update(
        &self,
        update: CompanyProjectStatusUpdate,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        guard
            .company_project_status_updates
            .entry(update.project_id)
            .or_default()
            .push(update);
        Ok(())
    }

    fn list_company_project_status_updates(
        &self,
        project_id: Uuid,
    ) -> Vec<CompanyProjectStatusUpdate> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut updates = guard
            .company_project_status_updates
            .get(&project_id)
            .cloned()
            .unwrap_or_default();
        updates.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        updates
    }
}

fn task_status_history_entry(
    from_status: Option<String>,
    task: &CompanyProjectTask,
) -> CompanyProjectTaskStatusHistory {
    let has_explicit_updater =
        task.updated_by_agent_id.is_some() || task.updated_by_human_user_id.is_some();
    let changed_by_agent_id = task.updated_by_agent_id.or_else(|| {
        (!has_explicit_updater)
            .then_some(task.created_by_agent_id)
            .flatten()
    });
    let changed_by_human_user_id = task.updated_by_human_user_id.or_else(|| {
        (!has_explicit_updater)
            .then_some(task.created_by_human_user_id)
            .flatten()
    });
    let change_source = if changed_by_agent_id.is_some() {
        "agent"
    } else if changed_by_human_user_id.is_some() {
        "human"
    } else {
        "system"
    };
    CompanyProjectTaskStatusHistory {
        id: Uuid::new_v4(),
        project_id: task.project_id,
        task_id: task.id,
        from_status,
        to_status: task.status.clone(),
        changed_by_agent_id,
        changed_by_human_user_id,
        change_source: change_source.into(),
        metadata: serde_json::Value::Object(Default::default()),
        created_at: task.updated_at,
    }
}
