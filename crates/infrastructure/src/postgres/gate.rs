use super::*;

impl GatePlatformRepository for PostgresPlatformRepository {
    fn insert_project_gate(&self, gate: ProjectGate) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO project_gates (
                    id, company_id, project_id, gate_key, gate_type, title, status,
                    related_task_id, required_evidence_json, decision_summary,
                    decided_by_agent_id, decided_by_human_user_id, decided_at,
                    created_at, updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
                "#,
                &[
                    &gate.id,
                    &gate.company_id,
                    &gate.project_id,
                    &gate.gate_key,
                    &gate.gate_type,
                    &gate.title,
                    &gate.status,
                    &gate.related_task_id,
                    &Json(gate.required_evidence.clone()),
                    &gate.decision_summary,
                    &gate.decided_by_agent_id,
                    &gate.decided_by_human_user_id,
                    &gate.decided_at,
                    &gate.created_at,
                    &gate.updated_at,
                ],
            )?;
            Ok(())
        })
    }

    fn get_project_gate(&self, gate_id: Uuid) -> Option<ProjectGate> {
        self.with_client(|client| {
            client.query_opt("SELECT * FROM project_gates WHERE id = $1", &[&gate_id])
        })
        .ok()
        .flatten()
        .map(map_project_gate)
    }

    fn list_project_gates(&self, project_id: Uuid) -> Vec<ProjectGate> {
        self.with_client(|client| {
            client.query(
                "SELECT * FROM project_gates WHERE project_id = $1 ORDER BY created_at, id",
                &[&project_id],
            )
        })
        .unwrap_or_default()
        .into_iter()
        .map(map_project_gate)
        .collect()
    }

    fn update_project_gate(&self, gate: ProjectGate) -> AppResult<()> {
        let updated = self.with_client(|client| {
            client.execute(
                r#"
                UPDATE project_gates
                SET gate_key = $2,
                    gate_type = $3,
                    title = $4,
                    status = $5,
                    related_task_id = $6,
                    required_evidence_json = $7,
                    decision_summary = $8,
                    decided_by_agent_id = $9,
                    decided_by_human_user_id = $10,
                    decided_at = $11,
                    updated_at = $12
                WHERE id = $1
                "#,
                &[
                    &gate.id,
                    &gate.gate_key,
                    &gate.gate_type,
                    &gate.title,
                    &gate.status,
                    &gate.related_task_id,
                    &Json(gate.required_evidence.clone()),
                    &gate.decision_summary,
                    &gate.decided_by_agent_id,
                    &gate.decided_by_human_user_id,
                    &gate.decided_at,
                    &gate.updated_at,
                ],
            )
        })?;
        if updated == 0 {
            return Err(AppError::NotFound("project gate not found".into()));
        }
        Ok(())
    }

    fn save_project_task_gate_requirement(
        &self,
        requirement: ProjectTaskGateRequirement,
    ) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO project_task_gate_requirements (
                    task_id, gate_id, required_status, created_at
                )
                VALUES ($1, $2, $3, $4)
                ON CONFLICT (task_id, gate_id)
                DO UPDATE SET required_status = EXCLUDED.required_status
                "#,
                &[
                    &requirement.task_id,
                    &requirement.gate_id,
                    &requirement.required_status,
                    &requirement.created_at,
                ],
            )?;
            Ok(())
        })
    }

    fn list_project_task_gate_requirements(
        &self,
        project_id: Uuid,
    ) -> Vec<ProjectTaskGateRequirement> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT requirement.task_id, requirement.gate_id,
                       requirement.required_status, requirement.created_at
                FROM project_task_gate_requirements requirement
                JOIN project_gates gate ON gate.id = requirement.gate_id
                WHERE gate.project_id = $1
                ORDER BY requirement.created_at, requirement.task_id
                "#,
                &[&project_id],
            )
        })
        .unwrap_or_default()
        .into_iter()
        .map(|row| ProjectTaskGateRequirement {
            task_id: row.get("task_id"),
            gate_id: row.get("gate_id"),
            required_status: row.get("required_status"),
            created_at: row.get("created_at"),
        })
        .collect()
    }
}

fn map_project_gate(row: Row) -> ProjectGate {
    ProjectGate {
        id: row.get("id"),
        company_id: row.get("company_id"),
        project_id: row.get("project_id"),
        gate_key: row.get("gate_key"),
        gate_type: row.get("gate_type"),
        title: row.get("title"),
        status: row.get("status"),
        related_task_id: row.get("related_task_id"),
        required_evidence: row.get::<_, Json<Vec<String>>>("required_evidence_json").0,
        decision_summary: row.get("decision_summary"),
        decided_by_agent_id: row.get("decided_by_agent_id"),
        decided_by_human_user_id: row.get("decided_by_human_user_id"),
        decided_at: row.get("decided_at"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}
