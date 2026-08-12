use super::*;
use ai_chat_domain::company::{
    project_environment_requirement_satisfied, PROJECT_ENVIRONMENT_SERVICE_HEALTH_HEALTHY,
    PROJECT_ENVIRONMENT_SERVICE_HEALTH_UNHEALTHY, PROJECT_ENVIRONMENT_SERVICE_HEALTH_UNKNOWN,
    PROJECT_ENVIRONMENT_STATUS_DEGRADED, PROJECT_ENVIRONMENT_STATUS_OFFLINE,
    PROJECT_ENVIRONMENT_STATUS_PROVISIONING, PROJECT_ENVIRONMENT_STATUS_READY,
    PROJECT_ENVIRONMENT_STATUS_UNKNOWN,
};

const ENVIRONMENT_STATUSES: &[&str] = &[
    PROJECT_ENVIRONMENT_STATUS_UNKNOWN,
    PROJECT_ENVIRONMENT_STATUS_PROVISIONING,
    PROJECT_ENVIRONMENT_STATUS_READY,
    PROJECT_ENVIRONMENT_STATUS_DEGRADED,
    PROJECT_ENVIRONMENT_STATUS_OFFLINE,
];

const SERVICE_HEALTH_STATUSES: &[&str] = &[
    PROJECT_ENVIRONMENT_SERVICE_HEALTH_UNKNOWN,
    PROJECT_ENVIRONMENT_SERVICE_HEALTH_HEALTHY,
    PROJECT_ENVIRONMENT_SERVICE_HEALTH_UNHEALTHY,
];

impl<R: PlatformRepository, V: OwnershipProofVerifier> PlatformApp<R, V> {
    pub fn list_project_environments_for_human(
        &self,
        input: ListProjectEnvironmentsForHumanInput,
    ) -> AppResult<Vec<ProjectEnvironment>> {
        self.ensure_company_project_for_human_manager(
            input.human_user_id,
            input.company_id,
            input.project_id,
        )?;
        Ok(self.repo.list_project_environments(input.project_id))
    }

    pub fn list_project_environment_requirements_for_human(
        &self,
        human_user_id: Uuid,
        company_id: Uuid,
        project_id: Uuid,
    ) -> AppResult<Vec<ProjectTaskEnvironmentRequirement>> {
        self.ensure_company_project_for_human_manager(human_user_id, company_id, project_id)?;
        Ok(self
            .repo
            .list_project_task_environment_requirements(project_id))
    }

    pub fn list_environment_services_for_human(
        &self,
        human_user_id: Uuid,
        company_id: Uuid,
        project_id: Uuid,
    ) -> AppResult<Vec<ProjectEnvironmentService>> {
        self.ensure_company_project_for_human_manager(human_user_id, company_id, project_id)?;
        Ok(self.list_project_environment_services(project_id))
    }

    pub fn create_project_environment_for_human(
        &self,
        input: CreateProjectEnvironmentForHumanInput,
    ) -> AppResult<ProjectEnvironment> {
        let project = self.ensure_company_project_for_human_manager(
            input.human_user_id,
            input.company_id,
            input.project_id,
        )?;
        self.ensure_project_not_paused(&project)?;
        self.create_project_environment_record(
            project.id,
            input.environment_key,
            input.display_name,
            input.desired_revision,
        )
    }

    pub fn observe_project_environment_for_human(
        &self,
        input: ObserveProjectEnvironmentForHumanInput,
    ) -> AppResult<ProjectEnvironment> {
        let project = self.ensure_company_project_for_human_manager(
            input.human_user_id,
            input.company_id,
            input.project_id,
        )?;
        self.ensure_project_not_paused(&project)?;
        self.observe_project_environment_record(
            &project,
            input.environment_id,
            input.status,
            input.desired_revision,
            input.observed_revision,
            input.configuration_fingerprint,
            input.health_summary,
            input.observed_at,
            input.services,
        )
    }

    pub fn set_project_task_environment_requirement_for_human(
        &self,
        input: SetProjectTaskEnvironmentRequirementForHumanInput,
    ) -> AppResult<ProjectTaskEnvironmentRequirement> {
        let project = self.ensure_company_project_for_human_manager(
            input.human_user_id,
            input.company_id,
            input.project_id,
        )?;
        self.ensure_project_not_paused(&project)?;
        self.set_project_task_environment_requirement_record(
            &project,
            input.task_id,
            input.environment_id,
            input.required_revision,
            input.required_services,
            input.require_healthy,
        )
    }

    pub fn list_project_environments(
        &self,
        input: ListProjectEnvironmentsInput,
    ) -> AppResult<Vec<ProjectEnvironment>> {
        self.ensure_company_project_access(
            input.company_id,
            input.project_id,
            input.actor_agent_id,
        )?;
        Ok(self.repo.list_project_environments(input.project_id))
    }

    pub fn create_project_environment(
        &self,
        input: CreateProjectEnvironmentInput,
    ) -> AppResult<ProjectEnvironment> {
        let project = self.ensure_company_project_access(
            input.company_id,
            input.project_id,
            input.actor_agent_id,
        )?;
        self.ensure_company_agent_permission(
            input.company_id,
            input.actor_agent_id,
            COMPANY_PERMISSION_PROJECT_MANAGE,
        )?;
        self.ensure_project_not_paused(&project)?;
        self.create_project_environment_record(
            project.id,
            input.environment_key,
            input.display_name,
            input.desired_revision,
        )
    }

    pub fn observe_project_environment(
        &self,
        input: ObserveProjectEnvironmentInput,
    ) -> AppResult<ProjectEnvironment> {
        let project = self.ensure_company_project_access(
            input.company_id,
            input.project_id,
            input.actor_agent_id,
        )?;
        self.ensure_company_agent_permission(
            input.company_id,
            input.actor_agent_id,
            COMPANY_PERMISSION_PROJECT_MANAGE,
        )?;
        self.ensure_project_not_paused(&project)?;
        self.observe_project_environment_record(
            &project,
            input.environment_id,
            input.status,
            input.desired_revision,
            input.observed_revision,
            input.configuration_fingerprint,
            input.health_summary,
            input.observed_at,
            input.services,
        )
    }

    pub fn set_project_task_environment_requirement(
        &self,
        input: SetProjectTaskEnvironmentRequirementInput,
    ) -> AppResult<ProjectTaskEnvironmentRequirement> {
        let project = self.ensure_company_project_access(
            input.company_id,
            input.project_id,
            input.actor_agent_id,
        )?;
        self.ensure_company_agent_permission(
            input.company_id,
            input.actor_agent_id,
            COMPANY_PERMISSION_TASK_ASSIGN,
        )?;
        self.ensure_project_not_paused(&project)?;
        self.set_project_task_environment_requirement_record(
            &project,
            input.task_id,
            input.environment_id,
            input.required_revision,
            input.required_services,
            input.require_healthy,
        )
    }

    pub(super) fn project_task_environment_requirements_satisfied(
        &self,
        project_id: Uuid,
        task_id: Uuid,
    ) -> bool {
        let environments = self
            .repo
            .list_project_environments(project_id)
            .into_iter()
            .map(|environment| (environment.id, environment))
            .collect::<HashMap<_, _>>();
        self.repo
            .list_project_task_environment_requirements(project_id)
            .into_iter()
            .filter(|requirement| requirement.task_id == task_id)
            .all(|requirement| {
                environments
                    .get(&requirement.environment_id)
                    .is_some_and(|environment| {
                        let services = self.repo.list_project_environment_services(environment.id);
                        project_environment_requirement_satisfied(
                            &requirement,
                            environment,
                            &services,
                        )
                    })
            })
    }

    pub(super) fn ensure_project_task_environment_ready(
        &self,
        project_id: Uuid,
        task_id: Uuid,
    ) -> AppResult<()> {
        if self.project_task_environment_requirements_satisfied(project_id, task_id) {
            Ok(())
        } else {
            Err(AppError::Conflict(
                "task_environment_not_ready: project task environment requirements are not satisfied".into(),
            ))
        }
    }

    fn create_project_environment_record(
        &self,
        project_id: Uuid,
        environment_key: String,
        display_name: String,
        desired_revision: Option<String>,
    ) -> AppResult<ProjectEnvironment> {
        let now = now_utc();
        let environment = ProjectEnvironment {
            id: Uuid::new_v4(),
            project_id,
            environment_key: normalize_environment_key(&environment_key)?,
            display_name: normalize_environment_name(&display_name)?,
            status: PROJECT_ENVIRONMENT_STATUS_UNKNOWN.into(),
            desired_revision: normalize_optional_revision(desired_revision)?,
            observed_revision: None,
            configuration_fingerprint: None,
            health_summary: json!({}),
            last_observed_at: None,
            created_at: now,
            updated_at: now,
        };
        self.repo.insert_project_environment(environment.clone())?;
        Ok(environment)
    }

    #[allow(clippy::too_many_arguments)]
    fn observe_project_environment_record(
        &self,
        project: &CompanyProject,
        environment_id: Uuid,
        status: String,
        desired_revision: Option<String>,
        observed_revision: Option<String>,
        configuration_fingerprint: Option<String>,
        health_summary: Value,
        observed_at: Option<DateTime<Utc>>,
        service_inputs: Vec<ProjectEnvironmentServiceObservationInput>,
    ) -> AppResult<ProjectEnvironment> {
        let mut environment = self
            .repo
            .get_project_environment(environment_id)
            .filter(|environment| environment.project_id == project.id)
            .ok_or_else(|| AppError::NotFound("project environment not found".into()))?;
        let status = status.trim().to_ascii_lowercase();
        if !ENVIRONMENT_STATUSES.contains(&status.as_str()) {
            return Err(AppError::Validation(
                "unsupported project environment status".into(),
            ));
        }
        if !health_summary.is_object() {
            return Err(AppError::Validation(
                "environment health_summary must be an object".into(),
            ));
        }
        let observed_at = observed_at.unwrap_or_else(now_utc);
        let previous_fingerprint = environment.configuration_fingerprint.clone();
        let previous_status = environment.status.clone();
        let previous_revision = environment.observed_revision.clone();
        let previous_services = self.repo.list_project_environment_services(environment.id);
        environment.status = status;
        environment.desired_revision = normalize_optional_revision(desired_revision)?;
        environment.observed_revision = normalize_optional_revision(observed_revision)?;
        environment.configuration_fingerprint =
            normalize_optional_revision(configuration_fingerprint)?;
        environment.health_summary = health_summary;
        environment.last_observed_at = Some(observed_at);
        environment.updated_at = now_utc();
        let services = normalize_environment_services(environment.id, observed_at, service_inputs)?;
        let services_changed =
            environment_service_state(&previous_services) != environment_service_state(&services);
        self.repo.update_project_environment(environment.clone())?;
        self.repo
            .replace_project_environment_services(environment.id, services)?;
        if previous_status != environment.status
            || previous_revision != environment.observed_revision
            || previous_fingerprint != environment.configuration_fingerprint
            || services_changed
        {
            self.notify_project_tasks_ready_after_environment(
                project,
                environment.id,
                observed_at,
            )?;
        }
        Ok(environment)
    }

    fn set_project_task_environment_requirement_record(
        &self,
        project: &CompanyProject,
        task_id: Uuid,
        environment_id: Uuid,
        required_revision: Option<String>,
        required_services: Vec<String>,
        require_healthy: bool,
    ) -> AppResult<ProjectTaskEnvironmentRequirement> {
        self.repo
            .get_company_project_task(task_id)
            .filter(|task| task.project_id == project.id)
            .ok_or_else(|| AppError::NotFound("project task not found".into()))?;
        self.repo
            .get_project_environment(environment_id)
            .filter(|environment| environment.project_id == project.id)
            .ok_or_else(|| AppError::NotFound("project environment not found".into()))?;
        let requirement = ProjectTaskEnvironmentRequirement {
            task_id,
            environment_id,
            required_revision: normalize_optional_revision(required_revision)?,
            required_services: normalize_service_keys(required_services)?,
            require_healthy,
            created_at: now_utc(),
        };
        self.repo
            .save_project_task_environment_requirement(requirement.clone())?;
        Ok(requirement)
    }

    fn list_project_environment_services(
        &self,
        project_id: Uuid,
    ) -> Vec<ProjectEnvironmentService> {
        self.repo
            .list_project_environments(project_id)
            .into_iter()
            .flat_map(|environment| self.repo.list_project_environment_services(environment.id))
            .collect()
    }

    fn notify_project_tasks_ready_after_environment(
        &self,
        project: &CompanyProject,
        environment_id: Uuid,
        ready_at: DateTime<Utc>,
    ) -> AppResult<usize> {
        let requirements = self
            .repo
            .list_project_task_environment_requirements(project.id);
        let tasks = self.repo.list_company_project_tasks_result(project.id)?;
        let dependencies = self.repo.list_company_project_task_dependencies(project.id);
        let mut notified = 0;
        for task in tasks.iter().filter(|task| {
            task.status == PROJECT_TASK_STATUS_TODO
                && requirements.iter().any(|requirement| {
                    requirement.task_id == task.id && requirement.environment_id == environment_id
                })
        }) {
            if !self.project_task_gate_requirements_satisfied(project.id, task.id)
                || !self.project_task_environment_requirements_satisfied(project.id, task.id)
                || self.project_task_has_open_blockers(task.id)
                || dependencies
                    .iter()
                    .filter(|dependency| dependency.task_id == task.id)
                    .any(|dependency| {
                        tasks
                            .iter()
                            .find(|candidate| candidate.id == dependency.depends_on_task_id)
                            .is_none_or(|dependency_task| {
                                !ai_chat_domain::company::project_task_dependency_satisfied(
                                    &dependency.dependency_condition,
                                    &dependency_task.status,
                                )
                            })
                    })
            {
                continue;
            }
            let Some(assignee_agent_id) = task.assignee_agent_id else {
                continue;
            };
            self.enqueue_agent_event(
                assignee_agent_id,
                "company.project.task_ready",
                json!({
                    "company_id": project.company_id,
                    "project_id": project.id,
                    "project_name": project.name,
                    "task_id": task.id,
                    "task_title": task.title,
                    "unlocked_by_environment_id": environment_id,
                }),
                50,
            )?;
            self.repo.request_agent_codex_trigger_wake(
                assignee_agent_id,
                ready_at,
                ai_chat_domain::company::AGENT_CODEX_WAKE_REASON_TASK_READY,
            )?;
            notified += 1;
        }
        Ok(notified)
    }
}

fn environment_service_state(
    services: &[ProjectEnvironmentService],
) -> Vec<(
    &str,
    Option<&str>,
    Option<&str>,
    Option<&str>,
    Option<&str>,
    &str,
    &Value,
)> {
    let mut state = services
        .iter()
        .map(|service| {
            (
                service.service_key.as_str(),
                service.desired_revision.as_deref(),
                service.observed_revision.as_deref(),
                service.image_digest.as_deref(),
                service.configuration_fingerprint.as_deref(),
                service.health_status.as_str(),
                &service.health_details,
            )
        })
        .collect::<Vec<_>>();
    state.sort_by(|left, right| left.0.cmp(right.0));
    state
}

fn normalize_environment_key(value: &str) -> AppResult<String> {
    let value = value.trim().to_ascii_lowercase();
    if value.is_empty()
        || value.chars().count() > 80
        || !value.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '_' | '-')
        })
    {
        return Err(AppError::Validation(
            "environment_key must use 1 to 80 lowercase letters, digits, dashes, or underscores"
                .into(),
        ));
    }
    Ok(value)
}

fn normalize_environment_name(value: &str) -> AppResult<String> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > 120 {
        return Err(AppError::Validation(
            "environment display_name must contain 1 to 120 characters".into(),
        ));
    }
    Ok(value.into())
}

fn normalize_optional_revision(value: Option<String>) -> AppResult<Option<String>> {
    value
        .map(|value| {
            let value = value.trim();
            if value.is_empty() || value.chars().count() > 240 {
                Err(AppError::Validation(
                    "revision and fingerprint values must contain 1 to 240 characters".into(),
                ))
            } else {
                Ok(value.into())
            }
        })
        .transpose()
}

fn normalize_service_keys(values: Vec<String>) -> AppResult<Vec<String>> {
    let mut normalized = Vec::new();
    for value in values {
        let key = normalize_environment_key(&value)?;
        if normalized.len() >= 50 {
            return Err(AppError::Validation(
                "an environment requirement supports at most 50 services".into(),
            ));
        }
        if !normalized.contains(&key) {
            normalized.push(key);
        }
    }
    Ok(normalized)
}

fn normalize_environment_services(
    environment_id: Uuid,
    observed_at: DateTime<Utc>,
    values: Vec<ProjectEnvironmentServiceObservationInput>,
) -> AppResult<Vec<ProjectEnvironmentService>> {
    let mut keys = HashSet::new();
    let mut services = Vec::new();
    for value in values {
        if services.len() >= 100 {
            return Err(AppError::Validation(
                "an environment observation supports at most 100 services".into(),
            ));
        }
        let service_key = normalize_environment_key(&value.service_key)?;
        if !keys.insert(service_key.clone()) {
            continue;
        }
        let health_status = value.health_status.trim().to_ascii_lowercase();
        if !SERVICE_HEALTH_STATUSES.contains(&health_status.as_str())
            || !value.health_details.is_object()
        {
            return Err(AppError::Validation(
                "service health_status or health_details is invalid".into(),
            ));
        }
        services.push(ProjectEnvironmentService {
            id: Uuid::new_v4(),
            environment_id,
            service_key,
            desired_revision: normalize_optional_revision(value.desired_revision)?,
            observed_revision: normalize_optional_revision(value.observed_revision)?,
            image_digest: normalize_optional_revision(value.image_digest)?,
            configuration_fingerprint: normalize_optional_revision(
                value.configuration_fingerprint,
            )?,
            health_status,
            health_details: value.health_details,
            observed_at,
        });
    }
    Ok(services)
}
