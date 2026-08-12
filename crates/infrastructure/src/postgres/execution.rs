use super::*;

fn map_project_task_attempt(row: Row) -> ProjectTaskAttempt {
    ProjectTaskAttempt {
        id: row.get("id"),
        project_id: row.get("project_id"),
        task_id: row.get("task_id"),
        agent_id: row.get("agent_id"),
        intent_id: row.get("intent_id"),
        attempt_number: row.get("attempt_number"),
        attempt_type: row.get("attempt_type"),
        status: row.get("status"),
        objective: row.get("objective"),
        result_summary: row.get("result_summary"),
        failure_category: row.get("failure_category"),
        started_at: row.get("started_at"),
        finished_at: row.get("finished_at"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn map_project_task_blocker(row: Row) -> ProjectTaskBlocker {
    ProjectTaskBlocker {
        id: row.get("id"),
        project_id: row.get("project_id"),
        task_id: row.get("task_id"),
        attempt_id: row.get("attempt_id"),
        blocker_type: row.get("blocker_type"),
        status: row.get("status"),
        summary: row.get("summary"),
        owner_agent_id: row.get("owner_agent_id"),
        owner_human_user_id: row.get("owner_human_user_id"),
        resolution_condition: row.get("resolution_condition"),
        resolution_summary: row.get("resolution_summary"),
        resolved_at: row.get("resolved_at"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn map_project_task_relation(row: Row) -> ProjectTaskRelation {
    ProjectTaskRelation {
        id: row.get("id"),
        project_id: row.get("project_id"),
        source_task_id: row.get("source_task_id"),
        target_task_id: row.get("target_task_id"),
        relation_type: row.get("relation_type"),
        created_by_agent_id: row.get("created_by_agent_id"),
        created_by_human_user_id: row.get("created_by_human_user_id"),
        created_at: row.get("created_at"),
    }
}

fn map_project_evidence(row: Row) -> ProjectEvidence {
    ProjectEvidence {
        id: row.get("id"),
        project_id: row.get("project_id"),
        task_id: row.get("task_id"),
        attempt_id: row.get("attempt_id"),
        gate_id: row.get("gate_id"),
        environment_id: row.get("environment_id"),
        evidence_type: row.get("evidence_type"),
        title: row.get("title"),
        summary: row.get("summary"),
        result: row.get("result"),
        artifact_refs: row.get::<_, Json<Vec<Value>>>("artifact_refs").0,
        metrics: row.get::<_, Json<Value>>("metrics").0,
        producer_agent_id: row.get("producer_agent_id"),
        producer_human_user_id: row.get("producer_human_user_id"),
        dedupe_key: row.get("dedupe_key"),
        created_at: row.get("created_at"),
    }
}

impl ExecutionPlatformRepository for PostgresPlatformRepository {
    fn insert_project_task_attempt(&self, attempt: ProjectTaskAttempt) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"INSERT INTO project_task_attempts (
                    id, project_id, task_id, agent_id, intent_id, attempt_number,
                    attempt_type, status, objective, result_summary, failure_category,
                    started_at, finished_at, created_at, updated_at
                ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15)"#,
                &[
                    &attempt.id,
                    &attempt.project_id,
                    &attempt.task_id,
                    &attempt.agent_id,
                    &attempt.intent_id,
                    &attempt.attempt_number,
                    &attempt.attempt_type,
                    &attempt.status,
                    &attempt.objective,
                    &attempt.result_summary,
                    &attempt.failure_category,
                    &attempt.started_at,
                    &attempt.finished_at,
                    &attempt.created_at,
                    &attempt.updated_at,
                ],
            )
        })
        .map(|_| ())
    }

    fn update_project_task_attempt(&self, attempt: ProjectTaskAttempt) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"UPDATE project_task_attempts SET status=$2, result_summary=$3,
                    failure_category=$4, started_at=$5, finished_at=$6, updated_at=$7
                    WHERE id=$1"#,
                &[
                    &attempt.id,
                    &attempt.status,
                    &attempt.result_summary,
                    &attempt.failure_category,
                    &attempt.started_at,
                    &attempt.finished_at,
                    &attempt.updated_at,
                ],
            )
        })
        .and_then(|count| {
            (count == 1)
                .then_some(())
                .ok_or_else(|| AppError::NotFound("task attempt not found".into()))
        })
    }

    fn get_project_task_attempt(&self, attempt_id: Uuid) -> Option<ProjectTaskAttempt> {
        self.with_client(|client| {
            client.query_opt(
                "SELECT * FROM project_task_attempts WHERE id=$1",
                &[&attempt_id],
            )
        })
        .ok()
        .flatten()
        .map(map_project_task_attempt)
    }

    fn list_project_task_attempts(&self, task_id: Uuid) -> Vec<ProjectTaskAttempt> {
        self.with_client(|client| {
            client.query(
                "SELECT * FROM project_task_attempts WHERE task_id=$1 ORDER BY attempt_number DESC",
                &[&task_id],
            )
        })
        .map(|rows| rows.into_iter().map(map_project_task_attempt).collect())
        .unwrap_or_default()
    }

    fn insert_project_task_blocker(&self, blocker: ProjectTaskBlocker) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"INSERT INTO project_task_blockers (
                    id, project_id, task_id, attempt_id, blocker_type, status, summary,
                    owner_agent_id, owner_human_user_id, resolution_condition,
                    resolution_summary, resolved_at, created_at, updated_at
                ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)"#,
                &[
                    &blocker.id,
                    &blocker.project_id,
                    &blocker.task_id,
                    &blocker.attempt_id,
                    &blocker.blocker_type,
                    &blocker.status,
                    &blocker.summary,
                    &blocker.owner_agent_id,
                    &blocker.owner_human_user_id,
                    &blocker.resolution_condition,
                    &blocker.resolution_summary,
                    &blocker.resolved_at,
                    &blocker.created_at,
                    &blocker.updated_at,
                ],
            )
        })
        .map(|_| ())
    }

    fn update_project_task_blocker(&self, blocker: ProjectTaskBlocker) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"UPDATE project_task_blockers SET status=$2, resolution_summary=$3,
                    resolved_at=$4, updated_at=$5 WHERE id=$1"#,
                &[
                    &blocker.id,
                    &blocker.status,
                    &blocker.resolution_summary,
                    &blocker.resolved_at,
                    &blocker.updated_at,
                ],
            )
        })
        .and_then(|count| {
            (count == 1)
                .then_some(())
                .ok_or_else(|| AppError::NotFound("task blocker not found".into()))
        })
    }

    fn get_project_task_blocker(&self, blocker_id: Uuid) -> Option<ProjectTaskBlocker> {
        self.with_client(|client| {
            client.query_opt(
                "SELECT * FROM project_task_blockers WHERE id=$1",
                &[&blocker_id],
            )
        })
        .ok()
        .flatten()
        .map(map_project_task_blocker)
    }

    fn list_project_task_blockers(&self, task_id: Uuid) -> Vec<ProjectTaskBlocker> {
        self.with_client(|client| {
            client.query(
                "SELECT * FROM project_task_blockers WHERE task_id=$1 ORDER BY created_at DESC",
                &[&task_id],
            )
        })
        .map(|rows| rows.into_iter().map(map_project_task_blocker).collect())
        .unwrap_or_default()
    }

    fn insert_project_task_relation(&self, relation: ProjectTaskRelation) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"INSERT INTO project_task_relations (
                    id, project_id, source_task_id, target_task_id, relation_type,
                    created_by_agent_id, created_by_human_user_id, created_at
                ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)"#,
                &[
                    &relation.id,
                    &relation.project_id,
                    &relation.source_task_id,
                    &relation.target_task_id,
                    &relation.relation_type,
                    &relation.created_by_agent_id,
                    &relation.created_by_human_user_id,
                    &relation.created_at,
                ],
            )
        })
        .map(|_| ())
    }

    fn remove_project_task_relation(&self, relation_id: Uuid) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                "DELETE FROM project_task_relations WHERE id=$1",
                &[&relation_id],
            )
        })
        .and_then(|count| {
            (count == 1)
                .then_some(())
                .ok_or_else(|| AppError::NotFound("task relation not found".into()))
        })
    }

    fn list_project_task_relations(&self, project_id: Uuid) -> Vec<ProjectTaskRelation> {
        self.with_client(|client| {
            client.query(
                "SELECT * FROM project_task_relations WHERE project_id=$1 ORDER BY created_at DESC",
                &[&project_id],
            )
        })
        .map(|rows| rows.into_iter().map(map_project_task_relation).collect())
        .unwrap_or_default()
    }

    fn insert_project_evidence(&self, evidence: ProjectEvidence) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"INSERT INTO project_evidence (
                    id, project_id, task_id, attempt_id, gate_id, environment_id,
                    evidence_type, title, summary, result, artifact_refs, metrics,
                    producer_agent_id, producer_human_user_id, dedupe_key, created_at
                ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16)"#,
                &[
                    &evidence.id,
                    &evidence.project_id,
                    &evidence.task_id,
                    &evidence.attempt_id,
                    &evidence.gate_id,
                    &evidence.environment_id,
                    &evidence.evidence_type,
                    &evidence.title,
                    &evidence.summary,
                    &evidence.result,
                    &Json(&evidence.artifact_refs),
                    &Json(&evidence.metrics),
                    &evidence.producer_agent_id,
                    &evidence.producer_human_user_id,
                    &evidence.dedupe_key,
                    &evidence.created_at,
                ],
            )
        })
        .map(|_| ())
    }

    fn find_project_evidence_by_dedupe_key(
        &self,
        project_id: Uuid,
        dedupe_key: &str,
    ) -> Option<ProjectEvidence> {
        self.with_client(|client| {
            client.query_opt(
                "SELECT * FROM project_evidence WHERE project_id=$1 AND dedupe_key=$2",
                &[&project_id, &dedupe_key],
            )
        })
        .ok()
        .flatten()
        .map(map_project_evidence)
    }

    fn list_project_evidence(&self, project_id: Uuid) -> Vec<ProjectEvidence> {
        self.with_client(|client| {
            client.query(
                "SELECT * FROM project_evidence WHERE project_id=$1 ORDER BY created_at DESC",
                &[&project_id],
            )
        })
        .map(|rows| rows.into_iter().map(map_project_evidence).collect())
        .unwrap_or_default()
    }
}
