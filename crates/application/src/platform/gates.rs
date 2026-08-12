use super::*;
use ai_chat_domain::company::{
    project_gate_requirement_satisfied, ProjectGate, ProjectTaskGateRequirement,
    AGENT_CODEX_WAKE_REASON_TASK_READY, PROJECT_GATE_STATUS_CANCELLED,
    PROJECT_GATE_STATUS_EVALUATING, PROJECT_GATE_STATUS_FAILED, PROJECT_GATE_STATUS_PASSED,
    PROJECT_GATE_STATUS_PENDING, PROJECT_GATE_STATUS_WAIVED,
};

const PROJECT_GATE_TYPES: &[&str] = &[
    "design",
    "technical",
    "qa",
    "pm",
    "environment",
    "approval",
    "release",
    "custom",
];

impl<R: PlatformRepository, V: OwnershipProofVerifier> PlatformApp<R, V> {
    pub fn list_project_gates_for_human(
        &self,
        input: ListProjectGatesForHumanInput,
    ) -> AppResult<Vec<ProjectGate>> {
        self.ensure_company_project_for_human_manager(
            input.human_user_id,
            input.company_id,
            input.project_id,
        )?;
        Ok(self.repo.list_project_gates(input.project_id))
    }

    pub fn list_project_gate_requirements_for_human(
        &self,
        human_user_id: Uuid,
        company_id: Uuid,
        project_id: Uuid,
    ) -> AppResult<Vec<ProjectTaskGateRequirement>> {
        self.ensure_company_project_for_human_manager(human_user_id, company_id, project_id)?;
        Ok(self.repo.list_project_task_gate_requirements(project_id))
    }

    pub fn create_project_gate_for_human(
        &self,
        input: CreateProjectGateForHumanInput,
    ) -> AppResult<ProjectGate> {
        let project = self.ensure_company_project_for_human_manager(
            input.human_user_id,
            input.company_id,
            input.project_id,
        )?;
        self.ensure_project_not_paused(&project)?;
        let gate_key = normalize_gate_key(&input.gate_key)?;
        let gate_type = input.gate_type.trim().to_ascii_lowercase();
        if !PROJECT_GATE_TYPES.contains(&gate_type.as_str()) {
            return Err(AppError::Validation("unsupported project gate type".into()));
        }
        let title = input.title.trim();
        if title.is_empty() || title.chars().count() > 160 {
            return Err(AppError::Validation(
                "project gate title must contain 1 to 160 characters".into(),
            ));
        }
        if let Some(task_id) = input.related_task_id {
            self.repo
                .get_company_project_task(task_id)
                .filter(|task| task.project_id == project.id)
                .ok_or_else(|| AppError::NotFound("related project task not found".into()))?;
        }
        let now = now_utc();
        let gate = ProjectGate {
            id: Uuid::new_v4(),
            company_id: input.company_id,
            project_id: project.id,
            gate_key,
            gate_type,
            title: title.into(),
            status: PROJECT_GATE_STATUS_PENDING.into(),
            related_task_id: input.related_task_id,
            required_evidence: normalize_gate_evidence(input.required_evidence)?,
            decision_summary: String::new(),
            decided_by_agent_id: None,
            decided_by_human_user_id: None,
            decided_at: None,
            created_at: now,
            updated_at: now,
        };
        self.repo.insert_project_gate(gate.clone())?;
        Ok(gate)
    }

    pub fn decide_project_gate_for_human(
        &self,
        input: DecideProjectGateForHumanInput,
    ) -> AppResult<ProjectGate> {
        let project = self.ensure_company_project_for_human_manager(
            input.human_user_id,
            input.company_id,
            input.project_id,
        )?;
        self.ensure_project_not_paused(&project)?;
        let mut gate = self
            .repo
            .get_project_gate(input.gate_id)
            .filter(|gate| gate.project_id == project.id)
            .ok_or_else(|| AppError::NotFound("project gate not found".into()))?;
        let status = normalize_gate_decision(&input.status, &input.decision_summary)?;
        gate.status = status;
        gate.decision_summary = input.decision_summary.trim().chars().take(2_000).collect();
        gate.decided_by_agent_id = None;
        gate.decided_by_human_user_id = Some(input.human_user_id);
        gate.decided_at =
            (!matches!(gate.status.as_str(), PROJECT_GATE_STATUS_EVALUATING)).then_some(now_utc());
        gate.updated_at = now_utc();
        self.repo.update_project_gate(gate.clone())?;
        if matches!(
            gate.status.as_str(),
            PROJECT_GATE_STATUS_PASSED | PROJECT_GATE_STATUS_WAIVED
        ) {
            self.notify_project_tasks_ready_after_gate(&project, gate.id, gate.updated_at)?;
        }
        Ok(gate)
    }

    pub fn set_project_task_gate_requirement_for_human(
        &self,
        input: SetProjectTaskGateRequirementForHumanInput,
    ) -> AppResult<ProjectTaskGateRequirement> {
        let project = self.ensure_company_project_for_human_manager(
            input.human_user_id,
            input.company_id,
            input.project_id,
        )?;
        self.ensure_project_not_paused(&project)?;
        self.repo
            .get_company_project_task(input.task_id)
            .filter(|task| task.project_id == project.id)
            .ok_or_else(|| AppError::NotFound("project task not found".into()))?;
        self.repo
            .get_project_gate(input.gate_id)
            .filter(|gate| gate.project_id == project.id)
            .ok_or_else(|| AppError::NotFound("project gate not found".into()))?;
        let required_status = input.required_status.trim().to_ascii_lowercase();
        if !matches!(
            required_status.as_str(),
            PROJECT_GATE_STATUS_PASSED | PROJECT_GATE_STATUS_WAIVED
        ) {
            return Err(AppError::Validation(
                "required gate status must be passed or waived".into(),
            ));
        }
        let requirement = ProjectTaskGateRequirement {
            task_id: input.task_id,
            gate_id: input.gate_id,
            required_status,
            created_at: now_utc(),
        };
        self.repo
            .save_project_task_gate_requirement(requirement.clone())?;
        Ok(requirement)
    }

    pub fn list_project_gates(&self, input: ListProjectGatesInput) -> AppResult<Vec<ProjectGate>> {
        self.ensure_company_project_access(
            input.company_id,
            input.project_id,
            input.actor_agent_id,
        )?;
        Ok(self.repo.list_project_gates(input.project_id))
    }

    pub fn create_project_gate(&self, input: CreateProjectGateInput) -> AppResult<ProjectGate> {
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
        let gate_key = normalize_gate_key(&input.gate_key)?;
        let gate_type = input.gate_type.trim().to_ascii_lowercase();
        if !PROJECT_GATE_TYPES.contains(&gate_type.as_str()) {
            return Err(AppError::Validation("unsupported project gate type".into()));
        }
        let title = input.title.trim();
        if title.is_empty() || title.chars().count() > 160 {
            return Err(AppError::Validation(
                "project gate title must contain 1 to 160 characters".into(),
            ));
        }
        if let Some(task_id) = input.related_task_id {
            self.repo
                .get_company_project_task(task_id)
                .filter(|task| task.project_id == project.id)
                .ok_or_else(|| AppError::NotFound("related project task not found".into()))?;
        }
        let required_evidence = normalize_gate_evidence(input.required_evidence)?;
        let now = now_utc();
        let gate = ProjectGate {
            id: Uuid::new_v4(),
            company_id: input.company_id,
            project_id: project.id,
            gate_key,
            gate_type,
            title: title.into(),
            status: PROJECT_GATE_STATUS_PENDING.into(),
            related_task_id: input.related_task_id,
            required_evidence,
            decision_summary: String::new(),
            decided_by_agent_id: None,
            decided_by_human_user_id: None,
            decided_at: None,
            created_at: now,
            updated_at: now,
        };
        self.repo.insert_project_gate(gate.clone())?;
        Ok(gate)
    }

    pub fn decide_project_gate(&self, input: DecideProjectGateInput) -> AppResult<ProjectGate> {
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
        let mut gate = self
            .repo
            .get_project_gate(input.gate_id)
            .filter(|gate| gate.project_id == project.id)
            .ok_or_else(|| AppError::NotFound("project gate not found".into()))?;
        let status = normalize_gate_decision(&input.status, &input.decision_summary)?;
        let decision_summary = input.decision_summary.trim();
        gate.status = status.clone();
        gate.decision_summary = decision_summary.chars().take(2_000).collect();
        gate.decided_by_agent_id = Some(input.actor_agent_id);
        gate.decided_by_human_user_id = None;
        gate.decided_at =
            (!matches!(status.as_str(), PROJECT_GATE_STATUS_EVALUATING)).then_some(now_utc());
        gate.updated_at = now_utc();
        self.repo.update_project_gate(gate.clone())?;
        if matches!(
            gate.status.as_str(),
            PROJECT_GATE_STATUS_PASSED | PROJECT_GATE_STATUS_WAIVED
        ) {
            self.notify_project_tasks_ready_after_gate(&project, gate.id, gate.updated_at)?;
        }
        Ok(gate)
    }

    pub fn set_project_task_gate_requirement(
        &self,
        input: SetProjectTaskGateRequirementInput,
    ) -> AppResult<ProjectTaskGateRequirement> {
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
        self.repo
            .get_company_project_task(input.task_id)
            .filter(|task| task.project_id == project.id)
            .ok_or_else(|| AppError::NotFound("project task not found".into()))?;
        self.repo
            .get_project_gate(input.gate_id)
            .filter(|gate| gate.project_id == project.id)
            .ok_or_else(|| AppError::NotFound("project gate not found".into()))?;
        let required_status = input.required_status.trim().to_ascii_lowercase();
        if !matches!(
            required_status.as_str(),
            PROJECT_GATE_STATUS_PASSED | PROJECT_GATE_STATUS_WAIVED
        ) {
            return Err(AppError::Validation(
                "required gate status must be passed or waived".into(),
            ));
        }
        let requirement = ProjectTaskGateRequirement {
            task_id: input.task_id,
            gate_id: input.gate_id,
            required_status,
            created_at: now_utc(),
        };
        self.repo
            .save_project_task_gate_requirement(requirement.clone())?;
        Ok(requirement)
    }

    pub(super) fn project_task_gate_requirements_satisfied(
        &self,
        project_id: Uuid,
        task_id: Uuid,
    ) -> bool {
        let gates = self
            .repo
            .list_project_gates(project_id)
            .into_iter()
            .map(|gate| (gate.id, gate))
            .collect::<HashMap<_, _>>();
        self.repo
            .list_project_task_gate_requirements(project_id)
            .into_iter()
            .filter(|requirement| requirement.task_id == task_id)
            .all(|requirement| {
                gates
                    .get(&requirement.gate_id)
                    .is_some_and(|gate| project_gate_requirement_satisfied(&requirement, gate))
            })
    }

    pub(super) fn ensure_project_task_gates_satisfied(
        &self,
        project_id: Uuid,
        task_id: Uuid,
    ) -> AppResult<()> {
        if self.project_task_gate_requirements_satisfied(project_id, task_id) {
            Ok(())
        } else {
            Err(AppError::Conflict(
                "task_gate_unresolved: project task has unresolved Gate requirements".into(),
            ))
        }
    }

    fn notify_project_tasks_ready_after_gate(
        &self,
        project: &CompanyProject,
        gate_id: Uuid,
        ready_at: DateTime<Utc>,
    ) -> AppResult<usize> {
        let requirements = self.repo.list_project_task_gate_requirements(project.id);
        let tasks = self.repo.list_company_project_tasks_result(project.id)?;
        let dependencies = self.repo.list_company_project_task_dependencies(project.id);
        let mut notified = 0;
        for task in tasks.iter().filter(|task| {
            task.status == PROJECT_TASK_STATUS_TODO
                && requirements.iter().any(|requirement| {
                    requirement.task_id == task.id && requirement.gate_id == gate_id
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
                    "unlocked_by_gate_id": gate_id,
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
}

fn normalize_gate_key(value: &str) -> AppResult<String> {
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
            "gate_key must use 1 to 80 lowercase letters, digits, dashes, or underscores".into(),
        ));
    }
    Ok(value)
}

fn normalize_gate_evidence(values: Vec<String>) -> AppResult<Vec<String>> {
    let mut normalized = Vec::new();
    for value in values {
        let value = value.trim();
        if value.is_empty() {
            continue;
        }
        if value.chars().count() > 240 || normalized.len() >= 20 {
            return Err(AppError::Validation(
                "Gate evidence supports at most 20 items of 240 characters".into(),
            ));
        }
        if !normalized.iter().any(|existing| existing == value) {
            normalized.push(value.to_string());
        }
    }
    Ok(normalized)
}

fn normalize_gate_decision(status: &str, summary: &str) -> AppResult<String> {
    let status = status.trim().to_ascii_lowercase();
    if !matches!(
        status.as_str(),
        PROJECT_GATE_STATUS_EVALUATING
            | PROJECT_GATE_STATUS_PASSED
            | PROJECT_GATE_STATUS_FAILED
            | PROJECT_GATE_STATUS_WAIVED
            | PROJECT_GATE_STATUS_CANCELLED
    ) {
        return Err(AppError::Validation(
            "unsupported project gate status".into(),
        ));
    }
    if matches!(
        status.as_str(),
        PROJECT_GATE_STATUS_PASSED | PROJECT_GATE_STATUS_FAILED | PROJECT_GATE_STATUS_WAIVED
    ) && summary.trim().is_empty()
    {
        return Err(AppError::Validation(
            "a terminal gate decision requires a summary".into(),
        ));
    }
    Ok(status)
}
