use uuid::Uuid;

use ai_chat_domain::company::CompanyProjectTask;
use ai_chat_shared::AppResult;

use crate::TaskRepository;

pub struct TaskService<'a, R: TaskRepository> {
    repository: &'a R,
}

impl<'a, R: TaskRepository> TaskService<'a, R> {
    pub(crate) fn new(repository: &'a R) -> Self {
        Self { repository }
    }
    pub fn list_for_project(&self, project_id: Uuid) -> AppResult<Vec<CompanyProjectTask>> {
        self.repository.project_tasks(project_id)
    }
}
