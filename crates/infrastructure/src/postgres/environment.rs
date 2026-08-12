use super::*;

impl EnvironmentPlatformRepository for PostgresPlatformRepository {
    fn insert_project_environment(&self, environment: ProjectEnvironment) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO project_environments (
                    id, project_id, environment_key, display_name, status,
                    desired_revision, observed_revision, configuration_fingerprint,
                    health_summary_json, last_observed_at, created_at, updated_at
                ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)
                "#,
                &[
                    &environment.id,
                    &environment.project_id,
                    &environment.environment_key,
                    &environment.display_name,
                    &environment.status,
                    &environment.desired_revision,
                    &environment.observed_revision,
                    &environment.configuration_fingerprint,
                    &Json(environment.health_summary.clone()),
                    &environment.last_observed_at,
                    &environment.created_at,
                    &environment.updated_at,
                ],
            )?;
            Ok(())
        })
    }

    fn get_project_environment(&self, environment_id: Uuid) -> Option<ProjectEnvironment> {
        self.with_client(|client| {
            client.query_opt(
                "SELECT * FROM project_environments WHERE id = $1",
                &[&environment_id],
            )
        })
        .ok()
        .flatten()
        .map(map_project_environment)
    }

    fn list_project_environments(&self, project_id: Uuid) -> Vec<ProjectEnvironment> {
        self.with_client(|client| {
            client.query(
                "SELECT * FROM project_environments WHERE project_id = $1 ORDER BY created_at, id",
                &[&project_id],
            )
        })
        .unwrap_or_default()
        .into_iter()
        .map(map_project_environment)
        .collect()
    }

    fn update_project_environment(&self, environment: ProjectEnvironment) -> AppResult<()> {
        let updated = self.with_client(|client| {
            client.execute(
                r#"
            UPDATE project_environments SET environment_key=$2, display_name=$3, status=$4,
                desired_revision=$5, observed_revision=$6, configuration_fingerprint=$7,
                health_summary_json=$8, last_observed_at=$9, updated_at=$10 WHERE id=$1
            "#,
                &[
                    &environment.id,
                    &environment.environment_key,
                    &environment.display_name,
                    &environment.status,
                    &environment.desired_revision,
                    &environment.observed_revision,
                    &environment.configuration_fingerprint,
                    &Json(environment.health_summary.clone()),
                    &environment.last_observed_at,
                    &environment.updated_at,
                ],
            )
        })?;
        if updated == 0 {
            return Err(AppError::NotFound("project environment not found".into()));
        }
        Ok(())
    }

    fn replace_project_environment_services(
        &self,
        environment_id: Uuid,
        services: Vec<ProjectEnvironmentService>,
    ) -> AppResult<()> {
        self.with_transaction(|transaction| {
            transaction
                .execute(
                    "DELETE FROM project_environment_services WHERE environment_id = $1",
                    &[&environment_id],
                )
                .map_err(map_postgres_error)?;
            for service in services {
                transaction
                    .execute(
                        r#"
                    INSERT INTO project_environment_services (
                        id, environment_id, service_key, desired_revision, observed_revision,
                        image_digest, configuration_fingerprint, health_status,
                        health_details_json, observed_at
                    ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)
                    "#,
                        &[
                            &service.id,
                            &service.environment_id,
                            &service.service_key,
                            &service.desired_revision,
                            &service.observed_revision,
                            &service.image_digest,
                            &service.configuration_fingerprint,
                            &service.health_status,
                            &Json(service.health_details.clone()),
                            &service.observed_at,
                        ],
                    )
                    .map_err(map_postgres_error)?;
            }
            Ok(())
        })
    }

    fn list_project_environment_services(
        &self,
        environment_id: Uuid,
    ) -> Vec<ProjectEnvironmentService> {
        self.with_client(|client| client.query(
            "SELECT * FROM project_environment_services WHERE environment_id = $1 ORDER BY service_key",
            &[&environment_id],
        )).unwrap_or_default().into_iter().map(map_project_environment_service).collect()
    }

    fn save_project_task_environment_requirement(
        &self,
        requirement: ProjectTaskEnvironmentRequirement,
    ) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO project_task_environment_requirements (
                    task_id, environment_id, required_revision, required_services_json,
                    require_healthy, created_at
                ) VALUES ($1,$2,$3,$4,$5,$6)
                ON CONFLICT (task_id, environment_id) DO UPDATE SET
                    required_revision=EXCLUDED.required_revision,
                    required_services_json=EXCLUDED.required_services_json,
                    require_healthy=EXCLUDED.require_healthy
                "#,
                &[
                    &requirement.task_id,
                    &requirement.environment_id,
                    &requirement.required_revision,
                    &Json(requirement.required_services.clone()),
                    &requirement.require_healthy,
                    &requirement.created_at,
                ],
            )?;
            Ok(())
        })
    }

    fn list_project_task_environment_requirements(
        &self,
        project_id: Uuid,
    ) -> Vec<ProjectTaskEnvironmentRequirement> {
        self.with_client(|client| {
            client.query(
                r#"
            SELECT requirement.* FROM project_task_environment_requirements requirement
            JOIN project_environments environment ON environment.id=requirement.environment_id
            WHERE environment.project_id=$1 ORDER BY requirement.created_at, requirement.task_id
            "#,
                &[&project_id],
            )
        })
        .unwrap_or_default()
        .into_iter()
        .map(|row| ProjectTaskEnvironmentRequirement {
            task_id: row.get("task_id"),
            environment_id: row.get("environment_id"),
            required_revision: row.get("required_revision"),
            required_services: row.get::<_, Json<Vec<String>>>("required_services_json").0,
            require_healthy: row.get("require_healthy"),
            created_at: row.get("created_at"),
        })
        .collect()
    }
}

fn map_project_environment(row: Row) -> ProjectEnvironment {
    ProjectEnvironment {
        id: row.get("id"),
        project_id: row.get("project_id"),
        environment_key: row.get("environment_key"),
        display_name: row.get("display_name"),
        status: row.get("status"),
        desired_revision: row.get("desired_revision"),
        observed_revision: row.get("observed_revision"),
        configuration_fingerprint: row.get("configuration_fingerprint"),
        health_summary: row.get::<_, Json<Value>>("health_summary_json").0,
        last_observed_at: row.get("last_observed_at"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn map_project_environment_service(row: Row) -> ProjectEnvironmentService {
    ProjectEnvironmentService {
        id: row.get("id"),
        environment_id: row.get("environment_id"),
        service_key: row.get("service_key"),
        desired_revision: row.get("desired_revision"),
        observed_revision: row.get("observed_revision"),
        image_digest: row.get("image_digest"),
        configuration_fingerprint: row.get("configuration_fingerprint"),
        health_status: row.get("health_status"),
        health_details: row.get::<_, Json<Value>>("health_details_json").0,
        observed_at: row.get("observed_at"),
    }
}
