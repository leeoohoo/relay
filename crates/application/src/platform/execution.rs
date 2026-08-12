use super::*;
use ai_chat_domain::company::{
    ProjectEvidence, ProjectTaskAttempt, ProjectTaskBlocker, ProjectTaskRelation,
    TASK_ATTEMPT_STATUS_CANCELLED, TASK_ATTEMPT_STATUS_FAILED, TASK_ATTEMPT_STATUS_INTERRUPTED,
    TASK_ATTEMPT_STATUS_QUEUED, TASK_ATTEMPT_STATUS_RUNNING, TASK_ATTEMPT_STATUS_SUCCEEDED,
    TASK_BLOCKER_STATUS_OPEN, TASK_BLOCKER_STATUS_RESOLVED, TASK_BLOCKER_STATUS_WAIVED,
};

const ATTEMPT_TYPES: &[&str] = &["execution", "review", "qa", "retest", "environment_check"];
const ATTEMPT_TERMINAL_STATUSES: &[&str] = &[
    TASK_ATTEMPT_STATUS_SUCCEEDED,
    TASK_ATTEMPT_STATUS_FAILED,
    TASK_ATTEMPT_STATUS_CANCELLED,
    TASK_ATTEMPT_STATUS_INTERRUPTED,
];
const BLOCKER_TYPES: &[&str] = &[
    "dependency",
    "environment",
    "approval",
    "defect",
    "decision",
    "external",
];
const RELATION_TYPES: &[&str] = &[
    "parent",
    "child",
    "retry_of",
    "supersedes",
    "caused_by",
    "validates",
    "fixes",
];
const EVIDENCE_TYPES: &[&str] = &[
    "test",
    "report",
    "screenshot",
    "log",
    "runtime",
    "design",
    "decision",
    "other",
];
const EVIDENCE_RESULTS: &[&str] = &["passed", "failed", "inconclusive", "informational"];

impl<R: PlatformRepository, V: OwnershipProofVerifier> PlatformApp<R, V> {
    pub fn get_project_task_execution_for_human(
        &self,
        input: ProjectTaskExecutionForHumanInput,
    ) -> AppResult<ProjectTaskExecutionView> {
        self.ensure_company_project_for_human_manager(
            input.human_user_id,
            input.company_id,
            input.project_id,
        )?;
        self.task_execution_view(input.project_id, input.task_id)
    }

    pub fn get_project_task_execution(
        &self,
        actor_agent_id: Uuid,
        company_id: Uuid,
        project_id: Uuid,
        task_id: Uuid,
    ) -> AppResult<ProjectTaskExecutionView> {
        self.ensure_company_project_access(company_id, project_id, actor_agent_id)?;
        self.task_execution_view(project_id, task_id)
    }

    pub fn start_project_task_attempt(
        &self,
        input: StartProjectTaskAttemptInput,
    ) -> AppResult<ProjectTaskAttempt> {
        let project = self.ensure_company_project_access(
            input.company_id,
            input.project_id,
            input.actor_agent_id,
        )?;
        self.ensure_project_not_paused(&project)?;
        let task = self.ensure_execution_task(project.id, input.task_id)?;
        if task.assignee_agent_id != Some(input.actor_agent_id) {
            return Err(AppError::Unauthorized(
                "only the assigned Agent can start a task attempt".into(),
            ));
        }
        self.ensure_project_task_dependencies_resolved(project.id, task.id)?;
        self.ensure_project_task_gates_satisfied(project.id, task.id)?;
        self.ensure_project_task_environment_ready(project.id, task.id)?;
        let attempt_type = normalize_choice(&input.attempt_type, ATTEMPT_TYPES, "attempt_type")?;
        let objective = normalize_text(&input.objective, 1, 2000, "attempt objective")?;
        let attempts = self.repo.list_project_task_attempts(task.id);
        if attempts.iter().any(|attempt| {
            matches!(
                attempt.status.as_str(),
                TASK_ATTEMPT_STATUS_QUEUED | TASK_ATTEMPT_STATUS_RUNNING
            )
        }) {
            return Err(AppError::Conflict("task_attempt_already_running".into()));
        }
        let now = now_utc();
        let attempt = ProjectTaskAttempt {
            id: Uuid::new_v4(),
            project_id: project.id,
            task_id: task.id,
            agent_id: input.actor_agent_id,
            intent_id: input.intent_id,
            attempt_number: attempts
                .iter()
                .map(|item| item.attempt_number)
                .max()
                .unwrap_or(0)
                + 1,
            attempt_type,
            status: TASK_ATTEMPT_STATUS_RUNNING.into(),
            objective,
            result_summary: None,
            failure_category: None,
            started_at: Some(now),
            finished_at: None,
            created_at: now,
            updated_at: now,
        };
        self.repo.insert_project_task_attempt(attempt.clone())?;
        Ok(attempt)
    }

    pub fn finish_project_task_attempt(
        &self,
        input: FinishProjectTaskAttemptInput,
    ) -> AppResult<ProjectTaskAttempt> {
        self.ensure_company_project_access(
            input.company_id,
            input.project_id,
            input.actor_agent_id,
        )?;
        self.ensure_execution_task(input.project_id, input.task_id)?;
        let mut attempt = self
            .repo
            .get_project_task_attempt(input.attempt_id)
            .filter(|item| item.project_id == input.project_id && item.task_id == input.task_id)
            .ok_or_else(|| AppError::NotFound("task attempt not found".into()))?;
        if attempt.agent_id != input.actor_agent_id {
            return Err(AppError::Unauthorized(
                "only the attempt Agent can finish it".into(),
            ));
        }
        if !matches!(
            attempt.status.as_str(),
            TASK_ATTEMPT_STATUS_QUEUED | TASK_ATTEMPT_STATUS_RUNNING
        ) {
            return Err(AppError::Conflict(
                "task attempt is already terminal".into(),
            ));
        }
        attempt.status =
            normalize_choice(&input.status, ATTEMPT_TERMINAL_STATUSES, "attempt status")?;
        attempt.result_summary = Some(normalize_text(
            &input.result_summary,
            1,
            4000,
            "attempt result",
        )?);
        attempt.failure_category = normalize_optional_text(input.failure_category, 120)?;
        let now = now_utc();
        attempt.finished_at = Some(now);
        attempt.updated_at = now;
        self.repo.update_project_task_attempt(attempt.clone())?;
        Ok(attempt)
    }

    pub fn open_project_task_blocker(
        &self,
        input: OpenProjectTaskBlockerInput,
    ) -> AppResult<ProjectTaskBlocker> {
        self.ensure_company_project_access(
            input.company_id,
            input.project_id,
            input.actor_agent_id,
        )?;
        self.ensure_execution_task(input.project_id, input.task_id)?;
        self.create_blocker(
            input.project_id,
            input.task_id,
            input.attempt_id,
            input.blocker_type,
            input.summary,
            input.owner_agent_id,
            None,
            input.resolution_condition,
        )
    }

    pub fn open_project_task_blocker_for_human(
        &self,
        input: OpenProjectTaskBlockerForHumanInput,
    ) -> AppResult<ProjectTaskBlocker> {
        self.ensure_company_project_for_human_manager(
            input.human_user_id,
            input.company_id,
            input.project_id,
        )?;
        self.ensure_execution_task(input.project_id, input.task_id)?;
        self.create_blocker(
            input.project_id,
            input.task_id,
            input.attempt_id,
            input.blocker_type,
            input.summary,
            input.owner_agent_id,
            Some(input.human_user_id),
            input.resolution_condition,
        )
    }

    pub fn resolve_project_task_blocker(
        &self,
        input: ResolveProjectTaskBlockerInput,
    ) -> AppResult<ProjectTaskBlocker> {
        self.ensure_company_project_access(
            input.company_id,
            input.project_id,
            input.actor_agent_id,
        )?;
        self.resolve_blocker(
            input.project_id,
            input.task_id,
            input.blocker_id,
            input.status,
            input.resolution_summary,
        )
    }

    pub fn resolve_project_task_blocker_for_human(
        &self,
        input: ResolveProjectTaskBlockerForHumanInput,
    ) -> AppResult<ProjectTaskBlocker> {
        self.ensure_company_project_for_human_manager(
            input.human_user_id,
            input.company_id,
            input.project_id,
        )?;
        self.resolve_blocker(
            input.project_id,
            input.task_id,
            input.blocker_id,
            input.status,
            input.resolution_summary,
        )
    }

    pub fn add_project_task_relation(
        &self,
        input: AddProjectTaskRelationInput,
    ) -> AppResult<ProjectTaskRelation> {
        self.ensure_company_project_access(
            input.company_id,
            input.project_id,
            input.actor_agent_id,
        )?;
        self.create_relation(
            input.project_id,
            input.source_task_id,
            input.target_task_id,
            input.relation_type,
            Some(input.actor_agent_id),
            None,
        )
    }

    pub fn add_project_task_relation_for_human(
        &self,
        input: AddProjectTaskRelationForHumanInput,
    ) -> AppResult<ProjectTaskRelation> {
        self.ensure_company_project_for_human_manager(
            input.human_user_id,
            input.company_id,
            input.project_id,
        )?;
        self.create_relation(
            input.project_id,
            input.source_task_id,
            input.target_task_id,
            input.relation_type,
            None,
            Some(input.human_user_id),
        )
    }

    pub fn remove_project_task_relation(
        &self,
        input: RemoveProjectTaskRelationInput,
    ) -> AppResult<()> {
        self.ensure_company_project_access(
            input.company_id,
            input.project_id,
            input.actor_agent_id,
        )?;
        self.ensure_relation_project(input.project_id, input.relation_id)?;
        self.repo.remove_project_task_relation(input.relation_id)
    }

    pub fn remove_project_task_relation_for_human(
        &self,
        input: RemoveProjectTaskRelationForHumanInput,
    ) -> AppResult<()> {
        self.ensure_company_project_for_human_manager(
            input.human_user_id,
            input.company_id,
            input.project_id,
        )?;
        self.ensure_relation_project(input.project_id, input.relation_id)?;
        self.repo.remove_project_task_relation(input.relation_id)
    }

    pub fn create_project_evidence(
        &self,
        input: CreateProjectEvidenceInput,
    ) -> AppResult<ProjectEvidence> {
        self.ensure_company_project_access(
            input.company_id,
            input.project_id,
            input.actor_agent_id,
        )?;
        self.create_evidence(
            input.project_id,
            input.task_id,
            input.attempt_id,
            input.gate_id,
            input.environment_id,
            input.evidence_type,
            input.title,
            input.summary,
            input.result,
            input.artifact_refs,
            input.metrics,
            Some(input.actor_agent_id),
            None,
            input.dedupe_key,
            now_utc(),
        )
    }

    pub fn create_project_evidence_for_human(
        &self,
        input: CreateProjectEvidenceForHumanInput,
    ) -> AppResult<ProjectEvidence> {
        self.ensure_company_project_for_human_manager(
            input.human_user_id,
            input.company_id,
            input.project_id,
        )?;
        self.create_evidence(
            input.project_id,
            input.task_id,
            input.attempt_id,
            input.gate_id,
            input.environment_id,
            input.evidence_type,
            input.title,
            input.summary,
            input.result,
            input.artifact_refs,
            input.metrics,
            None,
            Some(input.human_user_id),
            input.dedupe_key,
            input.created_at.unwrap_or_else(now_utc),
        )
    }

    fn task_execution_view(
        &self,
        project_id: Uuid,
        task_id: Uuid,
    ) -> AppResult<ProjectTaskExecutionView> {
        self.ensure_execution_task(project_id, task_id)?;
        Ok(ProjectTaskExecutionView {
            attempts: self.repo.list_project_task_attempts(task_id),
            blockers: self.repo.list_project_task_blockers(task_id),
            relations: self
                .repo
                .list_project_task_relations(project_id)
                .into_iter()
                .filter(|item| item.source_task_id == task_id || item.target_task_id == task_id)
                .collect(),
            evidence: self
                .repo
                .list_project_evidence(project_id)
                .into_iter()
                .filter(|item| item.task_id == Some(task_id))
                .collect(),
        })
    }

    fn ensure_execution_task(
        &self,
        project_id: Uuid,
        task_id: Uuid,
    ) -> AppResult<CompanyProjectTask> {
        self.repo
            .get_company_project_task(task_id)
            .filter(|task| task.project_id == project_id)
            .ok_or_else(|| AppError::NotFound("project task not found".into()))
    }

    #[allow(clippy::too_many_arguments)]
    fn create_blocker(
        &self,
        project_id: Uuid,
        task_id: Uuid,
        attempt_id: Option<Uuid>,
        blocker_type: String,
        summary: String,
        owner_agent_id: Option<Uuid>,
        owner_human_user_id: Option<Uuid>,
        resolution_condition: String,
    ) -> AppResult<ProjectTaskBlocker> {
        if let Some(attempt_id) = attempt_id {
            self.repo
                .get_project_task_attempt(attempt_id)
                .filter(|item| item.task_id == task_id)
                .ok_or_else(|| AppError::NotFound("task attempt not found".into()))?;
        }
        let now = now_utc();
        let blocker = ProjectTaskBlocker {
            id: Uuid::new_v4(),
            project_id,
            task_id,
            attempt_id,
            blocker_type: normalize_choice(&blocker_type, BLOCKER_TYPES, "blocker_type")?,
            status: TASK_BLOCKER_STATUS_OPEN.into(),
            summary: normalize_text(&summary, 1, 2000, "blocker summary")?,
            owner_agent_id,
            owner_human_user_id,
            resolution_condition: normalize_text(
                &resolution_condition,
                1,
                2000,
                "resolution condition",
            )?,
            resolution_summary: None,
            resolved_at: None,
            created_at: now,
            updated_at: now,
        };
        self.repo.insert_project_task_blocker(blocker.clone())?;
        Ok(blocker)
    }

    fn resolve_blocker(
        &self,
        project_id: Uuid,
        task_id: Uuid,
        blocker_id: Uuid,
        status: String,
        summary: String,
    ) -> AppResult<ProjectTaskBlocker> {
        let mut blocker = self
            .repo
            .get_project_task_blocker(blocker_id)
            .filter(|item| item.project_id == project_id && item.task_id == task_id)
            .ok_or_else(|| AppError::NotFound("task blocker not found".into()))?;
        if blocker.status != TASK_BLOCKER_STATUS_OPEN {
            return Err(AppError::Conflict("task blocker is already closed".into()));
        }
        blocker.status = normalize_choice(
            &status,
            &[TASK_BLOCKER_STATUS_RESOLVED, TASK_BLOCKER_STATUS_WAIVED],
            "blocker status",
        )?;
        blocker.resolution_summary = Some(normalize_text(&summary, 1, 2000, "blocker resolution")?);
        let now = now_utc();
        blocker.resolved_at = Some(now);
        blocker.updated_at = now;
        self.repo.update_project_task_blocker(blocker.clone())?;
        if let Some(project) = self.repo.get_company_project(project_id) {
            self.notify_project_tasks_ready_after_changes(&project, &[task_id], now)?;
        }
        Ok(blocker)
    }

    pub(super) fn project_task_has_open_blockers(&self, task_id: Uuid) -> bool {
        self.repo
            .list_project_task_blockers(task_id)
            .iter()
            .any(|blocker| blocker.status == TASK_BLOCKER_STATUS_OPEN)
    }

    pub(super) fn ensure_project_task_has_no_open_blockers(&self, task_id: Uuid) -> AppResult<()> {
        if self.project_task_has_open_blockers(task_id) {
            Err(AppError::Conflict(
                "task_blocker_open: resolve or waive open blockers before continuing".into(),
            ))
        } else {
            Ok(())
        }
    }

    fn create_relation(
        &self,
        project_id: Uuid,
        source_task_id: Uuid,
        target_task_id: Uuid,
        relation_type: String,
        agent_id: Option<Uuid>,
        human_id: Option<Uuid>,
    ) -> AppResult<ProjectTaskRelation> {
        if source_task_id == target_task_id {
            return Err(AppError::Validation(
                "a task cannot relate to itself".into(),
            ));
        }
        self.ensure_execution_task(project_id, source_task_id)?;
        self.ensure_execution_task(project_id, target_task_id)?;
        let relation = ProjectTaskRelation {
            id: Uuid::new_v4(),
            project_id,
            source_task_id,
            target_task_id,
            relation_type: normalize_choice(&relation_type, RELATION_TYPES, "relation_type")?,
            created_by_agent_id: agent_id,
            created_by_human_user_id: human_id,
            created_at: now_utc(),
        };
        self.repo.insert_project_task_relation(relation.clone())?;
        Ok(relation)
    }

    fn ensure_relation_project(&self, project_id: Uuid, relation_id: Uuid) -> AppResult<()> {
        if self
            .repo
            .list_project_task_relations(project_id)
            .iter()
            .any(|item| item.id == relation_id)
        {
            Ok(())
        } else {
            Err(AppError::NotFound("task relation not found".into()))
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn create_evidence(
        &self,
        project_id: Uuid,
        task_id: Option<Uuid>,
        attempt_id: Option<Uuid>,
        gate_id: Option<Uuid>,
        environment_id: Option<Uuid>,
        evidence_type: String,
        title: String,
        summary: String,
        result: String,
        artifact_refs: Vec<Value>,
        metrics: Value,
        agent_id: Option<Uuid>,
        human_id: Option<Uuid>,
        dedupe_key: Option<String>,
        created_at: DateTime<Utc>,
    ) -> AppResult<ProjectEvidence> {
        if let Some(task_id) = task_id {
            self.ensure_execution_task(project_id, task_id)?;
        }
        if let Some(attempt_id) = attempt_id {
            self.repo
                .get_project_task_attempt(attempt_id)
                .filter(|item| item.project_id == project_id)
                .ok_or_else(|| AppError::NotFound("task attempt not found".into()))?;
        }
        if !metrics.is_object() {
            return Err(AppError::Validation(
                "evidence metrics must be an object".into(),
            ));
        }
        let dedupe_key = normalize_optional_text(dedupe_key, 240)?;
        if let Some(key) = dedupe_key.as_deref() {
            if let Some(existing) = self
                .repo
                .find_project_evidence_by_dedupe_key(project_id, key)
            {
                return Ok(existing);
            }
        }
        let evidence = ProjectEvidence {
            id: Uuid::new_v4(),
            project_id,
            task_id,
            attempt_id,
            gate_id,
            environment_id,
            evidence_type: normalize_choice(&evidence_type, EVIDENCE_TYPES, "evidence_type")?,
            title: normalize_text(&title, 1, 200, "evidence title")?,
            summary: normalize_text(&summary, 1, 4000, "evidence summary")?,
            result: normalize_choice(&result, EVIDENCE_RESULTS, "evidence result")?,
            artifact_refs,
            metrics,
            producer_agent_id: agent_id,
            producer_human_user_id: human_id,
            dedupe_key,
            created_at,
        };
        self.repo.insert_project_evidence(evidence.clone())?;
        Ok(evidence)
    }
}

fn normalize_choice(value: &str, allowed: &[&str], field: &str) -> AppResult<String> {
    let value = value.trim().to_ascii_lowercase();
    if allowed.contains(&value.as_str()) {
        Ok(value)
    } else {
        Err(AppError::Validation(format!("unsupported {field}")))
    }
}

fn normalize_text(value: &str, min: usize, max: usize, field: &str) -> AppResult<String> {
    let value = value.trim();
    let len = value.chars().count();
    if len < min || len > max {
        Err(AppError::Validation(format!(
            "{field} must contain {min} to {max} characters"
        )))
    } else {
        Ok(value.into())
    }
}

fn normalize_optional_text(value: Option<String>, max: usize) -> AppResult<Option<String>> {
    value
        .map(|value| {
            let value = value.trim();
            if value.is_empty() || value.chars().count() > max {
                Err(AppError::Validation(format!(
                    "value must contain 1 to {max} characters"
                )))
            } else {
                Ok(value.into())
            }
        })
        .transpose()
}
