use super::*;
use std::collections::HashSet;

impl EnvironmentPlatformRepository for MemoryPlatformRepository {
    fn insert_project_environment(&self, environment: ProjectEnvironment) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if guard.project_environments.values().any(|existing| {
            existing.project_id == environment.project_id
                && existing.environment_key == environment.environment_key
        }) {
            return Err(AppError::Conflict(
                "project environment key already exists".into(),
            ));
        }
        guard
            .project_environments
            .insert(environment.id, environment);
        Ok(())
    }

    fn get_project_environment(&self, environment_id: Uuid) -> Option<ProjectEnvironment> {
        self.inner
            .read()
            .expect("memory repo lock poisoned")
            .project_environments
            .get(&environment_id)
            .cloned()
    }

    fn list_project_environments(&self, project_id: Uuid) -> Vec<ProjectEnvironment> {
        let mut environments = self
            .inner
            .read()
            .expect("memory repo lock poisoned")
            .project_environments
            .values()
            .filter(|environment| environment.project_id == project_id)
            .cloned()
            .collect::<Vec<_>>();
        environments.sort_by(|left, right| left.created_at.cmp(&right.created_at));
        environments
    }

    fn update_project_environment(&self, environment: ProjectEnvironment) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if !guard.project_environments.contains_key(&environment.id) {
            return Err(AppError::NotFound("project environment not found".into()));
        }
        guard
            .project_environments
            .insert(environment.id, environment);
        Ok(())
    }

    fn replace_project_environment_services(
        &self,
        environment_id: Uuid,
        services: Vec<ProjectEnvironmentService>,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        guard
            .project_environment_services
            .retain(|_, service| service.environment_id != environment_id);
        for service in services {
            guard
                .project_environment_services
                .insert(service.id, service);
        }
        Ok(())
    }

    fn list_project_environment_services(
        &self,
        environment_id: Uuid,
    ) -> Vec<ProjectEnvironmentService> {
        let mut services = self
            .inner
            .read()
            .expect("memory repo lock poisoned")
            .project_environment_services
            .values()
            .filter(|service| service.environment_id == environment_id)
            .cloned()
            .collect::<Vec<_>>();
        services.sort_by(|left, right| left.service_key.cmp(&right.service_key));
        services
    }

    fn save_project_task_environment_requirement(
        &self,
        requirement: ProjectTaskEnvironmentRequirement,
    ) -> AppResult<()> {
        self.inner
            .write()
            .expect("memory repo lock poisoned")
            .project_task_environment_requirements
            .insert(
                (requirement.task_id, requirement.environment_id),
                requirement,
            );
        Ok(())
    }

    fn list_project_task_environment_requirements(
        &self,
        project_id: Uuid,
    ) -> Vec<ProjectTaskEnvironmentRequirement> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let environment_ids = guard
            .project_environments
            .values()
            .filter(|environment| environment.project_id == project_id)
            .map(|environment| environment.id)
            .collect::<HashSet<_>>();
        guard
            .project_task_environment_requirements
            .values()
            .filter(|requirement| environment_ids.contains(&requirement.environment_id))
            .cloned()
            .collect()
    }
}
