use uuid::Uuid;

use ai_chat_shared::AppResult;

use crate::{ChatRepository, MessagePageView};

pub struct ChatService<'a, R: ChatRepository> {
    repository: &'a R,
}

impl<'a, R: ChatRepository> ChatService<'a, R> {
    pub(crate) fn new(repository: &'a R) -> Self {
        Self { repository }
    }
    pub fn message_page(
        &self,
        conversation_id: Uuid,
        before: Option<Uuid>,
        limit: usize,
    ) -> AppResult<MessagePageView> {
        self.repository
            .conversation_message_page(conversation_id, before, limit)
    }
}
