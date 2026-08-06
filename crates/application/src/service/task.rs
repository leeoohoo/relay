use super::*;

pub trait TaskPlatformRepository: Send + Sync {
    fn insert_company_project_task(&self, _task: CompanyProjectTask) -> AppResult<()> {
        Err(AppError::Validation(
            "company project tasks are not supported by this repository".into(),
        ))
    }
    fn get_company_project_task(&self, _task_id: Uuid) -> Option<CompanyProjectTask> {
        None
    }
    fn list_company_project_tasks(&self, _project_id: Uuid) -> Vec<CompanyProjectTask> {
        Vec::new()
    }
    fn list_company_project_tasks_result(
        &self,
        project_id: Uuid,
    ) -> AppResult<Vec<CompanyProjectTask>> {
        Ok(self.list_company_project_tasks(project_id))
    }
    fn update_company_project_task(&self, _task: CompanyProjectTask) -> AppResult<()> {
        Err(AppError::Validation(
            "company project tasks are not supported by this repository".into(),
        ))
    }
    fn update_company_project_tasks(&self, _tasks: Vec<CompanyProjectTask>) -> AppResult<()> {
        Err(AppError::Validation(
            "company project task batch updates are not supported by this repository".into(),
        ))
    }
    fn insert_company_project_task_dependency(
        &self,
        _dependency: CompanyProjectTaskDependency,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "company project task dependencies are not supported by this repository".into(),
        ))
    }
    fn remove_company_project_task_dependency(
        &self,
        _project_id: Uuid,
        _task_id: Uuid,
        _depends_on_task_id: Uuid,
        _removed_by_agent_id: Option<Uuid>,
        _removed_by_human_user_id: Option<Uuid>,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "company project task dependencies are not supported by this repository".into(),
        ))
    }
    fn list_company_project_task_dependencies(
        &self,
        _project_id: Uuid,
    ) -> Vec<CompanyProjectTaskDependency> {
        Vec::new()
    }
    fn list_company_project_task_status_history(
        &self,
        _project_id: Uuid,
    ) -> Vec<CompanyProjectTaskStatusHistory> {
        Vec::new()
    }
    fn insert_company_project_status_update(
        &self,
        _update: CompanyProjectStatusUpdate,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "company project status updates are not supported by this repository".into(),
        ))
    }
    fn list_company_project_status_updates(
        &self,
        _project_id: Uuid,
    ) -> Vec<CompanyProjectStatusUpdate> {
        Vec::new()
    }
    fn list_company_realtime_events(
        &self,
        _company_id: Uuid,
        _after_sequence_id: i64,
        _limit: usize,
    ) -> Vec<CompanyRealtimeEvent> {
        Vec::new()
    }
    fn list_company_realtime_events_result(
        &self,
        company_id: Uuid,
        after_sequence_id: i64,
        limit: usize,
    ) -> AppResult<Vec<CompanyRealtimeEvent>> {
        Ok(self.list_company_realtime_events(company_id, after_sequence_id, limit))
    }
    fn latest_company_realtime_sequence(&self, _company_id: Uuid) -> i64 {
        0
    }
    fn latest_company_realtime_sequence_result(&self, company_id: Uuid) -> AppResult<i64> {
        Ok(self.latest_company_realtime_sequence(company_id))
    }
}
