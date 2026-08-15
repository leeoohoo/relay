use super::*;

pub trait ChatPlatformRepository: Send + Sync {
    fn ensure_agent_conversation_bucket(&self, agent_id: Uuid) -> AppResult<()>;
    fn insert_conversation_preview(
        &self,
        agent_id: Uuid,
        preview: ConversationPreview,
    ) -> AppResult<()>;
    fn complete_company_conversation_creation(
        &self,
        _bundle: CompanyConversationCreationBundle,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "company conversations are not supported by this repository".into(),
        ))
    }
    fn complete_human_company_direct_conversation_creation(
        &self,
        _bundle: HumanCompanyDirectConversationCreationBundle,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "human company conversations are not supported by this repository".into(),
        ))
    }
    fn find_company_direct_conversation(
        &self,
        _company_id: Uuid,
        _left_agent_id: Uuid,
        _right_agent_id: Uuid,
    ) -> Option<ConversationPreview> {
        None
    }
    fn find_human_company_direct_conversation(
        &self,
        _company_id: Uuid,
        _human_user_id: Uuid,
        _target_agent_id: Uuid,
    ) -> Option<ConversationPreview> {
        None
    }
    fn get_conversation_context(&self, _conversation_id: Uuid) -> Option<ConversationContext> {
        None
    }
    fn get_conversation_context_result(
        &self,
        conversation_id: Uuid,
    ) -> AppResult<Option<ConversationContext>> {
        Ok(self.get_conversation_context(conversation_id))
    }
    fn list_conversation_member_ids(&self, _conversation_id: Uuid) -> Vec<Uuid> {
        Vec::new()
    }
    fn list_owner_agents(&self, human_user_id: Uuid) -> Vec<AgentProfile>;
    fn agent_exists(&self, agent_id: Uuid) -> bool;
    fn get_agent_profile(&self, agent_id: Uuid) -> Option<AgentProfile>;
    fn list_agent_conversations(&self, agent_id: Uuid) -> Vec<ConversationPreview>;
    fn list_company_conversations(&self, _company_id: Uuid) -> Vec<ConversationPreview> {
        Vec::new()
    }
    fn list_company_conversations_result(
        &self,
        company_id: Uuid,
    ) -> AppResult<Vec<ConversationPreview>> {
        Ok(self.list_company_conversations(company_id))
    }
    fn list_company_conversation_page(
        &self,
        company_id: Uuid,
        after_conversation_id: Option<Uuid>,
        limit: usize,
    ) -> AppResult<CursorPage<ConversationPreview>> {
        cursor_page_by_id(
            self.list_company_conversations_result(company_id)?,
            after_conversation_id,
            limit,
            |conversation| conversation.id,
        )
    }
    fn get_conversation_messages(&self, conversation_id: Uuid) -> Vec<MessageView>;
    fn get_conversation_messages_result(
        &self,
        conversation_id: Uuid,
    ) -> AppResult<Vec<MessageView>> {
        Ok(self.get_conversation_messages(conversation_id))
    }
    fn get_conversation_message_page(
        &self,
        conversation_id: Uuid,
        before_message_id: Option<Uuid>,
        limit: usize,
    ) -> AppResult<MessagePageView> {
        crate::pagination::conversation_message_page(
            self.get_conversation_messages_result(conversation_id)?,
            before_message_id,
            limit,
        )
    }
    fn append_message(&self, message: MessageView) -> AppResult<()>;
    fn append_message_with_metadata(
        &self,
        message: MessageView,
        _content_json: serde_json::Value,
    ) -> AppResult<()> {
        self.append_message(message)
    }
    fn get_project_discussion_thread(
        &self,
        _project_id: Uuid,
        _scope_type: &str,
        _subject_id: Uuid,
    ) -> Option<ProjectDiscussionThread> {
        None
    }
    fn complete_project_discussion_thread_creation(
        &self,
        _bundle: ProjectDiscussionThreadCreationBundle,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "project discussion threads are not supported by this repository".into(),
        ))
    }
    fn conversation_exists(&self, conversation_id: Uuid) -> bool;
    fn list_agent_action_logs(&self, agent_id: Uuid, limit: usize) -> Vec<AgentActionLog>;
}
