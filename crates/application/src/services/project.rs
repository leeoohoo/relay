use uuid::Uuid;

use ai_chat_domain::company::CompanyProject;
use ai_chat_shared::AppResult;

use crate::ProjectRepository;

pub struct ProjectService<'a, R: ProjectRepository> {
    repository: &'a R,
}

impl<'a, R: ProjectRepository> ProjectService<'a, R> {
    pub(crate) fn new(repository: &'a R) -> Self {
        Self { repository }
    }
    pub fn get(&self, project_id: Uuid) -> AppResult<Option<CompanyProject>> {
        self.repository.project(project_id)
    }
}
