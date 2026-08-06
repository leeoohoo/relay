use uuid::Uuid;

use ai_chat_domain::company::Company;
use ai_chat_shared::AppResult;

use crate::CompanyRepository;

pub struct CompanyService<'a, R: CompanyRepository> {
    repository: &'a R,
}

impl<'a, R: CompanyRepository> CompanyService<'a, R> {
    pub(crate) fn new(repository: &'a R) -> Self {
        Self { repository }
    }
    pub fn get(&self, company_id: Uuid) -> AppResult<Option<Company>> {
        self.repository.company(company_id)
    }
}
