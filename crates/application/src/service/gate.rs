use super::*;

pub trait GatePlatformRepository: Send + Sync {
    fn insert_project_gate(&self, _gate: ProjectGate) -> AppResult<()> {
        Err(AppError::Validation(
            "project Gates are not supported".into(),
        ))
    }
    fn get_project_gate(&self, _gate_id: Uuid) -> Option<ProjectGate> {
        None
    }
    fn list_project_gates(&self, _project_id: Uuid) -> Vec<ProjectGate> {
        Vec::new()
    }
    fn update_project_gate(&self, _gate: ProjectGate) -> AppResult<()> {
        Err(AppError::Validation(
            "project Gates are not supported".into(),
        ))
    }
    fn save_project_task_gate_requirement(
        &self,
        _requirement: ProjectTaskGateRequirement,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "project Gates are not supported".into(),
        ))
    }
    fn list_project_task_gate_requirements(
        &self,
        _project_id: Uuid,
    ) -> Vec<ProjectTaskGateRequirement> {
        Vec::new()
    }
}
