use super::*;

impl<R: PlatformRepository, V: OwnershipProofVerifier> PlatformApp<R, V> {
    pub(super) fn ensure_project_member_default_subscriptions(
        &self,
        project_id: Uuid,
        agent_id: Uuid,
    ) -> AppResult<Vec<ProjectMemberEventSubscription>> {
        let existing = self
            .repo
            .list_project_member_event_subscriptions(project_id, agent_id);
        let membership = self
            .repo
            .get_company_agent_membership(agent_id)
            .ok_or_else(|| AppError::NotFound("company Agent membership not found".into()))?;
        let profession = infer_company_profession(Some(&membership.job_title));
        if !existing.is_empty() {
            if profession.key != COMPANY_PROFESSION_PROJECT_MANAGER {
                return Ok(existing);
            }
            let mut subscriptions = existing;
            let now = now_utc();
            let mut changed = Vec::new();
            for (event_category, subscription_mode) in
                default_role_subscriptions(COMPANY_PROFESSION_PROJECT_MANAGER)
            {
                if let Some(subscription) = subscriptions
                    .iter_mut()
                    .find(|item| item.event_category == event_category)
                {
                    if subscription.subscription_mode != subscription_mode {
                        subscription.subscription_mode = subscription_mode.into();
                        subscription.updated_at = now;
                        changed.push(subscription.clone());
                    }
                } else {
                    let subscription = ProjectMemberEventSubscription {
                        project_id,
                        agent_profile_id: agent_id,
                        event_category: event_category.into(),
                        subscription_mode: subscription_mode.into(),
                        updated_at: now,
                    };
                    changed.push(subscription.clone());
                    subscriptions.push(subscription);
                }
            }
            if !changed.is_empty() {
                self.repo.save_project_member_event_subscriptions(changed)?;
            }
            return Ok(subscriptions);
        }
        let now = now_utc();
        let subscriptions = default_role_subscriptions(&profession.key)
            .into_iter()
            .map(
                |(event_category, subscription_mode)| ProjectMemberEventSubscription {
                    project_id,
                    agent_profile_id: agent_id,
                    event_category: event_category.into(),
                    subscription_mode: subscription_mode.into(),
                    updated_at: now,
                },
            )
            .collect::<Vec<_>>();
        self.repo
            .save_project_member_event_subscriptions(subscriptions.clone())?;
        Ok(subscriptions)
    }

    pub(super) fn project_event_subscription_mode(
        &self,
        project_id: Uuid,
        agent_id: Uuid,
        event_category: &str,
    ) -> AppResult<String> {
        Ok(self
            .ensure_project_member_default_subscriptions(project_id, agent_id)?
            .into_iter()
            .find(|item| item.event_category == event_category)
            .map(|item| item.subscription_mode)
            .unwrap_or_else(|| EVENT_SUBSCRIPTION_ON_DEMAND.into()))
    }

    pub(super) fn notify_project_managers_of_task_status_change(
        &self,
        project: &CompanyProject,
        task: &CompanyProjectTask,
        previous_status: &str,
        actor_agent_id: Option<Uuid>,
        actor_human_user_id: Option<Uuid>,
        changed_at: DateTime<Utc>,
    ) -> AppResult<usize> {
        let mut notified = 0;
        for member in self.repo.list_company_project_members(project.id) {
            if member.left_at.is_some() || actor_agent_id == Some(member.agent_profile_id) {
                continue;
            }
            let Some(membership) = self
                .repo
                .get_company_agent_membership(member.agent_profile_id)
            else {
                continue;
            };
            if infer_company_profession(Some(&membership.job_title)).key
                != COMPANY_PROFESSION_PROJECT_MANAGER
                || self.project_event_subscription_mode(
                    project.id,
                    member.agent_profile_id,
                    EVENT_CATEGORY_TASK,
                )? != EVENT_SUBSCRIPTION_IMMEDIATE
            {
                continue;
            }
            self.enqueue_agent_event(
                member.agent_profile_id,
                "company.project.task_status_changed",
                json!({
                    "company_id": project.company_id,
                    "project_id": project.id,
                    "project_name": project.name,
                    "task_id": task.id,
                    "task_title": task.title,
                    "previous_status": previous_status,
                    "current_status": task.status,
                    "assignee_agent_id": task.assignee_agent_id,
                    "changed_by_agent_id": actor_agent_id,
                    "changed_by_human_user_id": actor_human_user_id,
                }),
                55,
            )?;
            // Task state is the source of truth. A transient Trigger scheduling
            // failure must not turn an already-persisted task update into an
            // apparent business failure; the durable inbox event remains
            // available for the manager's next control turn.
            let _ = self.repo.request_agent_codex_trigger_wake(
                member.agent_profile_id,
                changed_at,
                AGENT_CODEX_WAKE_REASON_TASK_STATUS_CHANGED,
            );
            notified += 1;
        }
        Ok(notified)
    }

    pub(super) fn project_load_warnings(
        &self,
        project: &CompanyProject,
        members: &[CompanyProjectMemberView],
        tasks: &[CompanyProjectTask],
    ) -> Vec<ProjectLoadWarning> {
        let unfinished = tasks
            .iter()
            .filter(|task| {
                matches!(
                    task.status.as_str(),
                    PROJECT_TASK_STATUS_TODO
                        | PROJECT_TASK_STATUS_IN_PROGRESS
                        | PROJECT_TASK_STATUS_BLOCKED
                )
            })
            .collect::<Vec<_>>();
        let mut warnings = Vec::new();
        for member in members {
            let assigned = unfinished
                .iter()
                .filter(|task| task.assignee_agent_id == Some(member.agent_profile.id))
                .count();
            if unfinished.len() >= 2 && assigned * 2 > unfinished.len() {
                warnings.push(ProjectLoadWarning {
                    code: "task_concentration".into(),
                    severity: "warning".into(),
                    agent_profile_id: Some(member.agent_profile.id),
                    title: format!("{} 的任务过于集中", member.agent_profile.display_name),
                    detail: format!(
                        "该成员持有 {assigned}/{} 个未完成任务；建议 Owner 调整分工。",
                        unfinished.len()
                    ),
                });
            }
            if let Some(config) = self
                .repo
                .get_agent_codex_trigger_config_by_agent(member.agent_profile.id)
            {
                if config.consecutive_failure_count >= 3 {
                    warnings.push(ProjectLoadWarning {
                        code: "consecutive_runtime_failures".into(),
                        severity: "critical".into(),
                        agent_profile_id: Some(member.agent_profile.id),
                        title: format!("{} 连续运行失败", member.agent_profile.display_name),
                        detail: format!(
                            "已连续失败 {} 次，请检查认证、环境或任务阻塞。",
                            config.consecutive_failure_count
                        ),
                    });
                }
            }
        }
        let now = now_utc();
        for task in unfinished
            .iter()
            .filter(|task| task.status == PROJECT_TASK_STATUS_TODO)
        {
            if task.assignee_agent_id.is_some()
                && now.signed_duration_since(task.updated_at).num_minutes() >= 30
            {
                warnings.push(ProjectLoadWarning {
                    code: "ready_task_waiting".into(),
                    severity: "warning".into(),
                    agent_profile_id: task.assignee_agent_id,
                    title: "Ready 任务等待过久".into(),
                    detail: format!("“{}” 已等待负责人超过 30 分钟。", task.title),
                });
            }
        }
        if project.owner_agent_id == Uuid::nil() {
            warnings.push(ProjectLoadWarning {
                code: "missing_owner".into(),
                severity: "critical".into(),
                agent_profile_id: None,
                title: "项目缺少 Owner".into(),
                detail: "项目日常协调没有唯一入口。".into(),
            });
        }
        warnings
    }
}

fn default_role_subscriptions(profession_key: &str) -> Vec<(&'static str, &'static str)> {
    let mut defaults = vec![(EVENT_CATEGORY_MESSAGE, EVENT_SUBSCRIPTION_DIGEST)];
    match profession_key {
        COMPANY_PROFESSION_PROJECT_MANAGER => {
            defaults[0] = (EVENT_CATEGORY_MESSAGE, EVENT_SUBSCRIPTION_IMMEDIATE);
            defaults.extend([
                (EVENT_CATEGORY_GATE, EVENT_SUBSCRIPTION_IMMEDIATE),
                (EVENT_CATEGORY_BLOCKER, EVENT_SUBSCRIPTION_IMMEDIATE),
                (EVENT_CATEGORY_TASK, EVENT_SUBSCRIPTION_IMMEDIATE),
                (EVENT_CATEGORY_GOVERNANCE, EVENT_SUBSCRIPTION_IMMEDIATE),
            ])
        }
        COMPANY_PROFESSION_PRODUCT_MANAGER => defaults.extend([
            (EVENT_CATEGORY_GATE, EVENT_SUBSCRIPTION_IMMEDIATE),
            (EVENT_CATEGORY_BLOCKER, EVENT_SUBSCRIPTION_IMMEDIATE),
            (EVENT_CATEGORY_TASK, EVENT_SUBSCRIPTION_IMMEDIATE),
            (EVENT_CATEGORY_GOVERNANCE, EVENT_SUBSCRIPTION_IMMEDIATE),
        ]),
        COMPANY_PROFESSION_QA_ENGINEER => defaults.extend([
            (EVENT_CATEGORY_ENVIRONMENT, EVENT_SUBSCRIPTION_IMMEDIATE),
            (EVENT_CATEGORY_QA, EVENT_SUBSCRIPTION_IMMEDIATE),
            (EVENT_CATEGORY_TASK, EVENT_SUBSCRIPTION_IMMEDIATE),
            (EVENT_CATEGORY_TECHNICAL, EVENT_SUBSCRIPTION_MUTED),
        ]),
        COMPANY_PROFESSION_BUSINESS_ANALYST => defaults.extend([
            (EVENT_CATEGORY_REQUIREMENT, EVENT_SUBSCRIPTION_IMMEDIATE),
            (EVENT_CATEGORY_GATE, EVENT_SUBSCRIPTION_DIGEST),
            (EVENT_CATEGORY_ENVIRONMENT, EVENT_SUBSCRIPTION_MUTED),
            (EVENT_CATEGORY_QA, EVENT_SUBSCRIPTION_MUTED),
        ]),
        COMPANY_PROFESSION_TECHNICAL_MANAGER | COMPANY_PROFESSION_SOLUTION_ARCHITECT => defaults
            .extend([
                (EVENT_CATEGORY_TECHNICAL, EVENT_SUBSCRIPTION_IMMEDIATE),
                (EVENT_CATEGORY_BLOCKER, EVENT_SUBSCRIPTION_IMMEDIATE),
                (EVENT_CATEGORY_TASK, EVENT_SUBSCRIPTION_IMMEDIATE),
                (EVENT_CATEGORY_QA, EVENT_SUBSCRIPTION_DIGEST),
            ]),
        _ => defaults.extend([
            (EVENT_CATEGORY_TASK, EVENT_SUBSCRIPTION_IMMEDIATE),
            (EVENT_CATEGORY_BLOCKER, EVENT_SUBSCRIPTION_IMMEDIATE),
            (EVENT_CATEGORY_GATE, EVENT_SUBSCRIPTION_ON_DEMAND),
            (EVENT_CATEGORY_ENVIRONMENT, EVENT_SUBSCRIPTION_ON_DEMAND),
        ]),
    }
    defaults
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qa_ignores_unrelated_technical_noise_but_wakes_for_environment() {
        let subscriptions = default_role_subscriptions(COMPANY_PROFESSION_QA_ENGINEER);
        assert!(subscriptions.contains(&(EVENT_CATEGORY_ENVIRONMENT, EVENT_SUBSCRIPTION_IMMEDIATE)));
        assert!(subscriptions.contains(&(EVENT_CATEGORY_TECHNICAL, EVENT_SUBSCRIPTION_MUTED)));
    }

    #[test]
    fn business_analyst_is_not_woken_by_runtime_or_qa_status() {
        let subscriptions = default_role_subscriptions(COMPANY_PROFESSION_BUSINESS_ANALYST);
        assert!(subscriptions.contains(&(EVENT_CATEGORY_REQUIREMENT, EVENT_SUBSCRIPTION_IMMEDIATE)));
        assert!(subscriptions.contains(&(EVENT_CATEGORY_ENVIRONMENT, EVENT_SUBSCRIPTION_MUTED)));
        assert!(subscriptions.contains(&(EVENT_CATEGORY_QA, EVENT_SUBSCRIPTION_MUTED)));
    }

    #[test]
    fn project_manager_wakes_for_messages_and_task_governance() {
        let subscriptions = default_role_subscriptions(COMPANY_PROFESSION_PROJECT_MANAGER);
        for category in [
            EVENT_CATEGORY_MESSAGE,
            EVENT_CATEGORY_GATE,
            EVENT_CATEGORY_BLOCKER,
            EVENT_CATEGORY_TASK,
            EVENT_CATEGORY_GOVERNANCE,
        ] {
            assert!(subscriptions.contains(&(category, EVENT_SUBSCRIPTION_IMMEDIATE)));
        }
    }

    #[test]
    fn product_manager_keeps_project_messages_in_digest_mode() {
        let subscriptions = default_role_subscriptions(COMPANY_PROFESSION_PRODUCT_MANAGER);
        assert!(subscriptions.contains(&(EVENT_CATEGORY_MESSAGE, EVENT_SUBSCRIPTION_DIGEST)));
        assert!(subscriptions.contains(&(EVENT_CATEGORY_TASK, EVENT_SUBSCRIPTION_IMMEDIATE)));
    }
}
