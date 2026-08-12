use super::*;

pub trait EnvironmentPlatformRepository: Send + Sync {
    fn insert_project_environment(&self, _environment: ProjectEnvironment) -> AppResult<()> {
        Err(AppError::Validation(
            "project environments are not supported".into(),
        ))
    }
    fn get_project_environment(&self, _environment_id: Uuid) -> Option<ProjectEnvironment> {
        None
    }
    fn list_project_environments(&self, _project_id: Uuid) -> Vec<ProjectEnvironment> {
        Vec::new()
    }
    fn update_project_environment(&self, _environment: ProjectEnvironment) -> AppResult<()> {
        Err(AppError::Validation(
            "project environments are not supported".into(),
        ))
    }
    fn replace_project_environment_services(
        &self,
        _environment_id: Uuid,
        _services: Vec<ProjectEnvironmentService>,
    ) -> AppResult<()> {
        Ok(())
    }
    fn list_project_environment_services(
        &self,
        _environment_id: Uuid,
    ) -> Vec<ProjectEnvironmentService> {
        Vec::new()
    }
    fn save_project_task_environment_requirement(
        &self,
        _requirement: ProjectTaskEnvironmentRequirement,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "project environment requirements are not supported".into(),
        ))
    }
    fn list_project_task_environment_requirements(
        &self,
        _project_id: Uuid,
    ) -> Vec<ProjectTaskEnvironmentRequirement> {
        Vec::new()
    }
}
