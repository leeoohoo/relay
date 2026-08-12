use super::*;

pub trait ExecutionPlatformRepository: Send + Sync {
    fn insert_project_task_attempt(&self, _attempt: ProjectTaskAttempt) -> AppResult<()> {
        Err(AppError::Validation(
            "task attempts are not supported".into(),
        ))
    }
    fn update_project_task_attempt(&self, _attempt: ProjectTaskAttempt) -> AppResult<()> {
        Err(AppError::Validation(
            "task attempts are not supported".into(),
        ))
    }
    fn get_project_task_attempt(&self, _attempt_id: Uuid) -> Option<ProjectTaskAttempt> {
        None
    }
    fn list_project_task_attempts(&self, _task_id: Uuid) -> Vec<ProjectTaskAttempt> {
        Vec::new()
    }
    fn insert_project_task_blocker(&self, _blocker: ProjectTaskBlocker) -> AppResult<()> {
        Err(AppError::Validation(
            "task blockers are not supported".into(),
        ))
    }
    fn update_project_task_blocker(&self, _blocker: ProjectTaskBlocker) -> AppResult<()> {
        Err(AppError::Validation(
            "task blockers are not supported".into(),
        ))
    }
    fn get_project_task_blocker(&self, _blocker_id: Uuid) -> Option<ProjectTaskBlocker> {
        None
    }
    fn list_project_task_blockers(&self, _task_id: Uuid) -> Vec<ProjectTaskBlocker> {
        Vec::new()
    }
    fn insert_project_task_relation(&self, _relation: ProjectTaskRelation) -> AppResult<()> {
        Err(AppError::Validation(
            "task relations are not supported".into(),
        ))
    }
    fn remove_project_task_relation(&self, _relation_id: Uuid) -> AppResult<()> {
        Err(AppError::Validation(
            "task relations are not supported".into(),
        ))
    }
    fn list_project_task_relations(&self, _project_id: Uuid) -> Vec<ProjectTaskRelation> {
        Vec::new()
    }
    fn insert_project_evidence(&self, _evidence: ProjectEvidence) -> AppResult<()> {
        Err(AppError::Validation(
            "project evidence is not supported".into(),
        ))
    }
    fn find_project_evidence_by_dedupe_key(
        &self,
        _project_id: Uuid,
        _dedupe_key: &str,
    ) -> Option<ProjectEvidence> {
        None
    }
    fn list_project_evidence(&self, _project_id: Uuid) -> Vec<ProjectEvidence> {
        Vec::new()
    }
}
