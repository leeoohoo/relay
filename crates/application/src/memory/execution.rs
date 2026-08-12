use super::*;

impl ExecutionPlatformRepository for MemoryPlatformRepository {
    fn insert_project_task_attempt(&self, attempt: ProjectTaskAttempt) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if guard.project_task_attempts.values().any(|existing| {
            existing.task_id == attempt.task_id
                && matches!(existing.status.as_str(), "queued" | "running")
        }) {
            return Err(AppError::Conflict("task_attempt_already_running".into()));
        }
        guard.project_task_attempts.insert(attempt.id, attempt);
        Ok(())
    }

    fn update_project_task_attempt(&self, attempt: ProjectTaskAttempt) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if !guard.project_task_attempts.contains_key(&attempt.id) {
            return Err(AppError::NotFound("task attempt not found".into()));
        }
        guard.project_task_attempts.insert(attempt.id, attempt);
        Ok(())
    }

    fn get_project_task_attempt(&self, attempt_id: Uuid) -> Option<ProjectTaskAttempt> {
        self.inner
            .read()
            .expect("memory repo lock poisoned")
            .project_task_attempts
            .get(&attempt_id)
            .cloned()
    }

    fn list_project_task_attempts(&self, task_id: Uuid) -> Vec<ProjectTaskAttempt> {
        let mut items = self
            .inner
            .read()
            .expect("memory repo lock poisoned")
            .project_task_attempts
            .values()
            .filter(|item| item.task_id == task_id)
            .cloned()
            .collect::<Vec<_>>();
        items.sort_by(|left, right| right.attempt_number.cmp(&left.attempt_number));
        items
    }

    fn insert_project_task_blocker(&self, blocker: ProjectTaskBlocker) -> AppResult<()> {
        self.inner
            .write()
            .expect("memory repo lock poisoned")
            .project_task_blockers
            .insert(blocker.id, blocker);
        Ok(())
    }

    fn update_project_task_blocker(&self, blocker: ProjectTaskBlocker) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if !guard.project_task_blockers.contains_key(&blocker.id) {
            return Err(AppError::NotFound("task blocker not found".into()));
        }
        guard.project_task_blockers.insert(blocker.id, blocker);
        Ok(())
    }

    fn get_project_task_blocker(&self, blocker_id: Uuid) -> Option<ProjectTaskBlocker> {
        self.inner
            .read()
            .expect("memory repo lock poisoned")
            .project_task_blockers
            .get(&blocker_id)
            .cloned()
    }

    fn list_project_task_blockers(&self, task_id: Uuid) -> Vec<ProjectTaskBlocker> {
        let mut items = self
            .inner
            .read()
            .expect("memory repo lock poisoned")
            .project_task_blockers
            .values()
            .filter(|item| item.task_id == task_id)
            .cloned()
            .collect::<Vec<_>>();
        items.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        items
    }

    fn insert_project_task_relation(&self, relation: ProjectTaskRelation) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if guard.project_task_relations.values().any(|existing| {
            existing.source_task_id == relation.source_task_id
                && existing.target_task_id == relation.target_task_id
                && existing.relation_type == relation.relation_type
        }) {
            return Err(AppError::Conflict("task relation already exists".into()));
        }
        guard.project_task_relations.insert(relation.id, relation);
        Ok(())
    }

    fn remove_project_task_relation(&self, relation_id: Uuid) -> AppResult<()> {
        self.inner
            .write()
            .expect("memory repo lock poisoned")
            .project_task_relations
            .remove(&relation_id)
            .map(|_| ())
            .ok_or_else(|| AppError::NotFound("task relation not found".into()))
    }

    fn list_project_task_relations(&self, project_id: Uuid) -> Vec<ProjectTaskRelation> {
        let mut items = self
            .inner
            .read()
            .expect("memory repo lock poisoned")
            .project_task_relations
            .values()
            .filter(|item| item.project_id == project_id)
            .cloned()
            .collect::<Vec<_>>();
        items.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        items
    }

    fn insert_project_evidence(&self, evidence: ProjectEvidence) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if let Some(key) = evidence.dedupe_key.as_deref() {
            if guard.project_evidence.values().any(|existing| {
                existing.project_id == evidence.project_id
                    && existing.dedupe_key.as_deref() == Some(key)
            }) {
                return Err(AppError::Conflict("project evidence already exists".into()));
            }
        }
        guard.project_evidence.insert(evidence.id, evidence);
        Ok(())
    }

    fn find_project_evidence_by_dedupe_key(
        &self,
        project_id: Uuid,
        dedupe_key: &str,
    ) -> Option<ProjectEvidence> {
        self.inner
            .read()
            .expect("memory repo lock poisoned")
            .project_evidence
            .values()
            .find(|item| {
                item.project_id == project_id && item.dedupe_key.as_deref() == Some(dedupe_key)
            })
            .cloned()
    }

    fn list_project_evidence(&self, project_id: Uuid) -> Vec<ProjectEvidence> {
        let mut items = self
            .inner
            .read()
            .expect("memory repo lock poisoned")
            .project_evidence
            .values()
            .filter(|item| item.project_id == project_id)
            .cloned()
            .collect::<Vec<_>>();
        items.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        items
    }
}
