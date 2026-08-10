use super::mapping::*;
use super::*;

impl TaskPlatformRepository for PostgresPlatformRepository {
    fn insert_company_project_task(&self, task: CompanyProjectTask) -> AppResult<()> {
        self.with_client(|client| {
            let mut tx = client.transaction()?;
            tx.execute(
                r#"
                INSERT INTO company_project_tasks (
                    id, project_id, title, description, status, priority,
                    assignee_agent_id, created_by_agent_id, created_by_human_user_id,
                    updated_by_agent_id, updated_by_human_user_id, due_at,
                    completed_at, created_at, updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
                "#,
                &[
                    &task.id,
                    &task.project_id,
                    &task.title,
                    &task.description,
                    &task.status,
                    &task.priority,
                    &task.assignee_agent_id,
                    &task.created_by_agent_id,
                    &task.created_by_human_user_id,
                    &task.updated_by_agent_id,
                    &task.updated_by_human_user_id,
                    &task.due_at,
                    &task.completed_at,
                    &task.created_at,
                    &task.updated_at,
                ],
            )?;
            let change_source = if task.created_by_agent_id.is_some() {
                "agent"
            } else if task.created_by_human_user_id.is_some() {
                "human"
            } else {
                "system"
            };
            tx.execute(
                r#"
                INSERT INTO company_project_task_status_history (
                    id, project_id, task_id, from_status, to_status,
                    changed_by_agent_id, changed_by_human_user_id,
                    change_source, metadata, created_at
                ) VALUES ($1, $2, $3, NULL, $4, $5, $6, $7, '{}'::jsonb, $8)
                "#,
                &[
                    &Uuid::new_v4(),
                    &task.project_id,
                    &task.id,
                    &task.status,
                    &task.created_by_agent_id,
                    &task.created_by_human_user_id,
                    &change_source,
                    &task.created_at,
                ],
            )?;
            tx.execute(
                "UPDATE company_projects SET updated_at = $2, updated_by_agent_id = $3 WHERE id = $1",
                &[&task.project_id, &task.updated_at, &task.created_by_agent_id],
            )?;
            tx.commit()?;
            Ok(())
        })
    }

    fn get_company_project_task(&self, task_id: Uuid) -> Option<CompanyProjectTask> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT id, project_id, title, description, status, priority,
                       assignee_agent_id, created_by_agent_id, created_by_human_user_id,
                       updated_by_agent_id, updated_by_human_user_id,
                       due_at, completed_at, created_at, updated_at
                FROM company_project_tasks
                WHERE id = $1
                "#,
                &[&task_id],
            )
        })
        .ok()
        .flatten()
        .map(map_company_project_task)
    }

    fn list_company_project_tasks(&self, project_id: Uuid) -> Vec<CompanyProjectTask> {
        self.list_company_project_tasks_result(project_id)
            .unwrap_or_default()
    }

    fn list_company_project_tasks_result(
        &self,
        project_id: Uuid,
    ) -> AppResult<Vec<CompanyProjectTask>> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT id, project_id, title, description, status, priority,
                       assignee_agent_id, created_by_agent_id, created_by_human_user_id,
                       updated_by_agent_id, updated_by_human_user_id,
                       due_at, completed_at, created_at, updated_at
                FROM company_project_tasks
                WHERE project_id = $1
                ORDER BY updated_at DESC, created_at DESC
                "#,
                &[&project_id],
            )
        })
        .map(|rows| rows.into_iter().map(map_company_project_task).collect())
    }

    fn update_company_project_task(&self, task: CompanyProjectTask) -> AppResult<()> {
        self.with_client(|client| {
            let mut tx = client.transaction()?;
            let previous_status = tx
                .query_opt(
                    "SELECT status FROM company_project_tasks WHERE id = $1 FOR UPDATE",
                    &[&task.id],
                )?
                .map(|row| row.get::<_, String>("status"));
            if previous_status.as_deref() != Some(task.status.as_str()) {
                let change_source = if task.updated_by_agent_id.is_some() {
                    "agent"
                } else if task.updated_by_human_user_id.is_some() {
                    "human"
                } else {
                    "system"
                };
                tx.execute(
                    r#"
                    INSERT INTO company_project_task_status_history (
                        id, project_id, task_id, from_status, to_status,
                        changed_by_agent_id, changed_by_human_user_id,
                        change_source, metadata, created_at
                    ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, '{}'::jsonb, $9)
                    "#,
                    &[
                        &Uuid::new_v4(),
                        &task.project_id,
                        &task.id,
                        &previous_status,
                        &task.status,
                        &task.updated_by_agent_id,
                        &task.updated_by_human_user_id,
                        &change_source,
                        &task.updated_at,
                    ],
                )?;
            }
            tx.execute(
                r#"
                UPDATE company_project_tasks
                SET title = $2,
                    description = $3,
                    status = $4,
                    priority = $5,
                    assignee_agent_id = $6,
                    updated_by_agent_id = $7,
                    updated_by_human_user_id = $8,
                    due_at = $9,
                    completed_at = $10,
                    updated_at = $11
                WHERE id = $1
                "#,
                &[
                    &task.id,
                    &task.title,
                    &task.description,
                    &task.status,
                    &task.priority,
                    &task.assignee_agent_id,
                    &task.updated_by_agent_id,
                    &task.updated_by_human_user_id,
                    &task.due_at,
                    &task.completed_at,
                    &task.updated_at,
                ],
            )?;
            tx.execute(
                "UPDATE company_projects SET updated_at = $2, updated_by_agent_id = $3 WHERE id = $1",
                &[&task.project_id, &task.updated_at, &task.updated_by_agent_id],
            )?;
            tx.commit()?;
            Ok(())
        })
    }

    fn update_company_project_tasks(&self, tasks: Vec<CompanyProjectTask>) -> AppResult<()> {
        if tasks.is_empty() {
            return Ok(());
        }
        self.with_client(|client| {
            let mut tx = client.transaction()?;
            for task in &tasks {
                let previous_status = tx
                    .query_opt(
                        "SELECT status FROM company_project_tasks WHERE id = $1 FOR UPDATE",
                        &[&task.id],
                    )?
                    .map(|row| row.get::<_, String>("status"));
                if previous_status.as_deref() != Some(task.status.as_str()) {
                    let change_source = if task.updated_by_agent_id.is_some() {
                        "agent"
                    } else if task.updated_by_human_user_id.is_some() {
                        "human"
                    } else {
                        "system"
                    };
                    tx.execute(
                        r#"
                        INSERT INTO company_project_task_status_history (
                            id, project_id, task_id, from_status, to_status,
                            changed_by_agent_id, changed_by_human_user_id,
                            change_source, metadata, created_at
                        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, '{}'::jsonb, $9)
                        "#,
                        &[
                            &Uuid::new_v4(),
                            &task.project_id,
                            &task.id,
                            &previous_status,
                            &task.status,
                            &task.updated_by_agent_id,
                            &task.updated_by_human_user_id,
                            &change_source,
                            &task.updated_at,
                        ],
                    )?;
                }
                tx.execute(
                    r#"
                    UPDATE company_project_tasks
                    SET title = $2,
                        description = $3,
                        status = $4,
                        priority = $5,
                        assignee_agent_id = $6,
                        updated_by_agent_id = $7,
                        updated_by_human_user_id = $8,
                        due_at = $9,
                        completed_at = $10,
                        updated_at = $11
                    WHERE id = $1
                    "#,
                    &[
                        &task.id,
                        &task.title,
                        &task.description,
                        &task.status,
                        &task.priority,
                        &task.assignee_agent_id,
                        &task.updated_by_agent_id,
                        &task.updated_by_human_user_id,
                        &task.due_at,
                        &task.completed_at,
                        &task.updated_at,
                    ],
                )?;
            }
            let latest = &tasks[0];
            tx.execute(
                "UPDATE company_projects SET updated_at = $2, updated_by_agent_id = $3 WHERE id = $1",
                &[
                    &latest.project_id,
                    &latest.updated_at,
                    &latest.updated_by_agent_id,
                ],
            )?;
            tx.commit()?;
            Ok(())
        })
    }

    fn insert_company_project_task_dependency(
        &self,
        dependency: CompanyProjectTaskDependency,
    ) -> AppResult<()> {
        self.with_client(|client| {
            let mut tx = client.transaction()?;
            tx.execute(
                r#"
                INSERT INTO company_project_task_dependencies (
                    id, project_id, task_id, depends_on_task_id, dependency_condition,
                    created_by_agent_id, created_by_human_user_id, created_at
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                "#,
                &[
                    &dependency.id,
                    &dependency.project_id,
                    &dependency.task_id,
                    &dependency.depends_on_task_id,
                    &dependency.dependency_condition,
                    &dependency.created_by_agent_id,
                    &dependency.created_by_human_user_id,
                    &dependency.created_at,
                ],
            )?;
            tx.execute(
                "UPDATE company_projects SET updated_at = $2, updated_by_agent_id = $3 WHERE id = $1",
                &[
                    &dependency.project_id,
                    &dependency.created_at,
                    &dependency.created_by_agent_id,
                ],
            )?;
            tx.commit()?;
            Ok(())
        })
    }

    fn remove_company_project_task_dependency(
        &self,
        project_id: Uuid,
        task_id: Uuid,
        depends_on_task_id: Uuid,
        removed_by_agent_id: Option<Uuid>,
        removed_by_human_user_id: Option<Uuid>,
    ) -> AppResult<()> {
        self.with_client(|client| {
            let mut tx = client.transaction()?;
            tx.execute(
                r#"
                UPDATE company_project_task_dependencies
                SET created_by_agent_id = $4,
                    created_by_human_user_id = $5
                WHERE project_id = $1 AND task_id = $2 AND depends_on_task_id = $3
                "#,
                &[
                    &project_id,
                    &task_id,
                    &depends_on_task_id,
                    &removed_by_agent_id,
                    &removed_by_human_user_id,
                ],
            )?;
            tx.execute(
                r#"
                DELETE FROM company_project_task_dependencies
                WHERE project_id = $1 AND task_id = $2 AND depends_on_task_id = $3
                "#,
                &[&project_id, &task_id, &depends_on_task_id],
            )?;
            let now = Utc::now();
            tx.execute(
                "UPDATE company_projects SET updated_at = $2, updated_by_agent_id = $3 WHERE id = $1",
                &[&project_id, &now, &removed_by_agent_id],
            )?;
            tx.commit()?;
            Ok(())
        })
    }

    fn list_company_project_task_dependencies(
        &self,
        project_id: Uuid,
    ) -> Vec<CompanyProjectTaskDependency> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT id, project_id, task_id, depends_on_task_id, dependency_condition,
                       created_by_agent_id, created_by_human_user_id, created_at
                FROM company_project_task_dependencies
                WHERE project_id = $1
                ORDER BY created_at, id
                "#,
                &[&project_id],
            )
        })
        .unwrap_or_default()
        .into_iter()
        .map(map_company_project_task_dependency)
        .collect()
    }

    fn list_company_project_task_status_history(
        &self,
        project_id: Uuid,
    ) -> Vec<CompanyProjectTaskStatusHistory> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT id, project_id, task_id, from_status, to_status,
                       changed_by_agent_id, changed_by_human_user_id,
                       change_source, metadata, created_at
                FROM company_project_task_status_history
                WHERE project_id = $1
                ORDER BY created_at DESC, id DESC
                "#,
                &[&project_id],
            )
        })
        .unwrap_or_default()
        .into_iter()
        .map(map_company_project_task_status_history)
        .collect()
    }

    fn insert_company_project_status_update(
        &self,
        update: CompanyProjectStatusUpdate,
    ) -> AppResult<()> {
        self.with_client(|client| {
            let blockers = Json(&update.blockers);
            let next_steps = Json(&update.next_steps);
            client.execute(
                r#"
                INSERT INTO company_project_status_updates (
                    id, project_id, author_agent_id, summary, progress_percent,
                    blockers, next_steps, project_status, created_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                "#,
                &[
                    &update.id,
                    &update.project_id,
                    &update.author_agent_id,
                    &update.summary,
                    &update.progress_percent,
                    &blockers,
                    &next_steps,
                    &update.project_status,
                    &update.created_at,
                ],
            )?;
            Ok(())
        })
    }

    fn list_company_project_status_updates(
        &self,
        project_id: Uuid,
    ) -> Vec<CompanyProjectStatusUpdate> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT id, project_id, author_agent_id, summary, progress_percent,
                       blockers, next_steps, project_status, created_at
                FROM company_project_status_updates
                WHERE project_id = $1
                ORDER BY created_at DESC
                "#,
                &[&project_id],
            )
        })
        .unwrap_or_default()
        .into_iter()
        .map(map_company_project_status_update)
        .collect()
    }

    fn list_company_realtime_events(
        &self,
        company_id: Uuid,
        after_sequence_id: i64,
        limit: usize,
    ) -> Vec<CompanyRealtimeEvent> {
        self.list_company_realtime_events_result(company_id, after_sequence_id, limit)
            .unwrap_or_default()
    }

    fn list_company_realtime_events_result(
        &self,
        company_id: Uuid,
        after_sequence_id: i64,
        limit: usize,
    ) -> AppResult<Vec<CompanyRealtimeEvent>> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT sequence_id, id, company_id, event_type, aggregate_type,
                       aggregate_id, actor_agent_id, actor_human_user_id,
                       payload, created_at
                FROM realtime_events
                WHERE company_id = $1
                  AND sequence_id > $2
                ORDER BY sequence_id
                LIMIT $3
                "#,
                &[&company_id, &after_sequence_id, &(limit as i64)],
            )
        })
        .map(|rows| rows.into_iter().map(map_company_realtime_event).collect())
    }

    fn latest_company_realtime_sequence(&self, company_id: Uuid) -> i64 {
        self.latest_company_realtime_sequence_result(company_id)
            .unwrap_or(0)
    }

    fn latest_company_realtime_sequence_result(&self, company_id: Uuid) -> AppResult<i64> {
        self.with_client(|client| {
            client.query_one(
                "SELECT COALESCE(MAX(sequence_id), 0)::BIGINT AS sequence_id FROM realtime_events WHERE company_id = $1",
                &[&company_id],
            )
        })
        .map(|row| row.get("sequence_id"))
    }
}
