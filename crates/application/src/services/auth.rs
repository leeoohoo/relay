use ai_chat_domain::agent_identity::HumanUser;
use ai_chat_shared::AppResult;

use crate::AuthRepository;

pub struct AuthService<'a, R: AuthRepository> {
    repository: &'a R,
}

impl<'a, R: AuthRepository> AuthService<'a, R> {
    pub(crate) fn new(repository: &'a R) -> Self {
        Self { repository }
    }

    pub fn find_human_by_email(&self, email: &str) -> AppResult<Option<HumanUser>> {
        self.repository.find_human_by_email(email)
    }
}
