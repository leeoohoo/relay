use uuid::Uuid;

use ai_chat_domain::social::MessageView;
use ai_chat_shared::{AppError, AppResult};

use crate::contracts::MessagePageView;
use crate::validation::normalize_message_page_limit;

pub(crate) fn conversation_message_page(
    mut messages: Vec<MessageView>,
    before_message_id: Option<Uuid>,
    limit: usize,
) -> AppResult<MessagePageView> {
    let limit = normalize_message_page_limit(limit);
    messages.sort_by(|left, right| {
        left.created_at
            .cmp(&right.created_at)
            .then_with(|| left.id.cmp(&right.id))
    });
    let end = match before_message_id {
        Some(cursor) => messages
            .iter()
            .position(|message| message.id == cursor)
            .ok_or_else(|| {
                AppError::Validation("message cursor does not belong to the conversation".into())
            })?,
        None => messages.len(),
    };
    let start = end.saturating_sub(limit);
    let page_messages = messages[start..end].to_vec();
    let has_more = start > 0;
    Ok(MessagePageView {
        next_cursor: has_more.then(|| page_messages[0].id),
        messages: page_messages,
        has_more,
    })
}
