use super::*;

impl<R: PlatformRepository, V: OwnershipProofVerifier> PlatformApp<R, V> {
    pub fn agent_control_snapshot(
        &self,
        agent_profile_id: Uuid,
        company_id: Uuid,
    ) -> AppResult<AgentControlSnapshot> {
        self.repo
            .get_company_agent_membership(agent_profile_id)
            .filter(|membership| {
                membership.company_id == company_id && membership.employment_status == "active"
            })
            .ok_or_else(|| {
                AppError::Unauthorized("Agent is not an active company member".into())
            })?;
        self.repo
            .get_company_result(company_id)?
            .filter(|company| company.status == "active")
            .ok_or_else(|| AppError::NotFound("active company not found".into()))?;

        let now = now_utc();
        let projects = self
            .repo
            .list_company_projects_result(company_id)?
            .into_iter()
            .filter(|project| {
                project.status != PROJECT_STATUS_PAUSED
                    && self
                        .repo
                        .get_company_project_member(project.id, agent_profile_id)
                        .is_some_and(|member| member.left_at.is_none())
            })
            .collect::<Vec<_>>();
        let active_project_ids = projects
            .iter()
            .map(|project| project.id)
            .collect::<HashSet<_>>();

        let mut actionable_events = Vec::new();
        for event in self.repo.list_agent_inbox_events(
            agent_profile_id,
            Some(AgentInboxEventStatus::Pending),
            1_000,
        ) {
            if !event.requires_action
                || event.available_at > now
                || event.expires_at.is_some_and(|expires_at| expires_at <= now)
            {
                continue;
            }
            if let Some(project_id) = self.event_project_id(&event)? {
                if !active_project_ids.contains(&project_id) {
                    continue;
                }
            }
            actionable_events.push(event);
        }
        actionable_events.sort_by(|left, right| {
            left.priority
                .cmp(&right.priority)
                .then_with(|| left.created_at.cmp(&right.created_at))
        });

        let mut ready_tasks = Vec::new();
        let mut waiting_tasks = Vec::new();
        for project in &projects {
            let tasks = self.repo.list_company_project_tasks_result(project.id)?;
            let dependencies = self.repo.list_company_project_task_dependencies(project.id);
            for task in tasks.iter().filter(|task| {
                task.assignee_agent_id == Some(agent_profile_id)
                    && matches!(
                        task.status.as_str(),
                        PROJECT_TASK_STATUS_TODO | PROJECT_TASK_STATUS_IN_PROGRESS
                    )
            }) {
                let ready = self.project_task_gate_requirements_satisfied(project.id, task.id)
                    && self.project_task_environment_requirements_satisfied(project.id, task.id)
                    && dependencies
                        .iter()
                        .filter(|dependency| dependency.task_id == task.id)
                        .all(|dependency| {
                            tasks
                                .iter()
                                .find(|candidate| candidate.id == dependency.depends_on_task_id)
                                .is_some_and(|dependency_task| {
                                    ai_chat_domain::company::project_task_dependency_satisfied(
                                        &dependency.dependency_condition,
                                        &dependency_task.status,
                                    )
                                })
                        });
                if ready {
                    ready_tasks.push(task.clone());
                } else {
                    waiting_tasks.push(task.clone());
                }
            }
        }
        ready_tasks.sort_by(|left, right| {
            left.created_at
                .cmp(&right.created_at)
                .then_with(|| left.id.cmp(&right.id))
        });
        waiting_tasks.sort_by(|left, right| {
            left.created_at
                .cmp(&right.created_at)
                .then_with(|| left.id.cmp(&right.id))
        });

        let mut active_intents = self.repo.list_agent_execution_intents(
            agent_profile_id,
            Some(AGENT_EXECUTION_INTENT_STATUS_PENDING),
            100,
        );
        active_intents.extend(self.repo.list_agent_execution_intents(
            agent_profile_id,
            Some(AGENT_EXECUTION_INTENT_STATUS_RUNNING),
            100,
        ));
        active_intents.retain(|intent| active_project_ids.contains(&intent.project_id));
        active_intents.sort_by(|left, right| left.created_at.cmp(&right.created_at));

        let work_sessions = self.repo.list_agent_codex_sessions(agent_profile_id, 50);
        let snapshot_version = format!(
            "{}:{}:{}:{}:{}",
            now.timestamp_millis(),
            actionable_events.len(),
            ready_tasks.len(),
            waiting_tasks.len(),
            active_intents.len()
        );
        Ok(AgentControlSnapshot {
            agent_profile_id,
            company_id,
            generated_at: now,
            snapshot_version,
            actionable_events,
            ready_tasks,
            waiting_tasks,
            active_intents,
            work_sessions,
        })
    }

    fn event_project_id(&self, event: &AgentInboxEvent) -> AppResult<Option<Uuid>> {
        let mut project_id = payload_uuid_field_optional(&event.payload_json, "project_id");
        if project_id.is_none() {
            if let Some(conversation_id) =
                payload_uuid_field_optional(&event.payload_json, "conversation_id")
            {
                project_id = self
                    .repo
                    .get_conversation_context_result(conversation_id)?
                    .and_then(|context| context.project_id);
            }
        }
        Ok(project_id)
    }
}
