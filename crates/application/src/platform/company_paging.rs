use super::*;

impl<R: PlatformRepository, V: OwnershipProofVerifier> PlatformApp<R, V> {
    pub fn list_company_console_agent_page_for_human(
        &self,
        human_user_id: Uuid,
        company_id: Uuid,
        after_agent_membership_id: Option<Uuid>,
        limit: usize,
    ) -> AppResult<CursorPage<CompanyConsoleAgentView>> {
        self.company_console_identity(human_user_id, company_id)?;
        let page = self.repo.list_company_agent_membership_page(
            company_id,
            after_agent_membership_id,
            limit.clamp(1, 100),
        )?;
        Ok(CursorPage {
            items: page
                .items
                .into_iter()
                .map(|membership| self.company_console_agent(membership))
                .collect::<AppResult<Vec<_>>>()?,
            next_cursor: page.next_cursor,
            has_more: page.has_more,
        })
    }

    pub fn list_company_console_conversation_page_for_human(
        &self,
        human_user_id: Uuid,
        company_id: Uuid,
        after_conversation_id: Option<Uuid>,
        limit: usize,
    ) -> AppResult<CursorPage<CompanyConversationView>> {
        self.company_console_identity(human_user_id, company_id)?;
        let page = self.repo.list_company_conversation_page(
            company_id,
            after_conversation_id,
            limit.clamp(1, 100),
        )?;
        Ok(CursorPage {
            items: page
                .items
                .into_iter()
                .map(|preview| self.company_conversation_view(company_id, preview))
                .collect::<AppResult<Vec<_>>>()?,
            next_cursor: page.next_cursor,
            has_more: page.has_more,
        })
    }

    pub fn list_company_console_project_page_for_human(
        &self,
        human_user_id: Uuid,
        company_id: Uuid,
        after_project_id: Option<Uuid>,
        limit: usize,
    ) -> AppResult<CursorPage<CompanyProjectView>> {
        self.company_console_identity(human_user_id, company_id)?;
        let page = self.repo.list_company_project_page(
            company_id,
            after_project_id,
            limit.clamp(1, 100),
        )?;
        Ok(CursorPage {
            items: page
                .items
                .into_iter()
                .map(|project| self.company_project_view(project))
                .collect::<AppResult<Vec<_>>>()?,
            next_cursor: page.next_cursor,
            has_more: page.has_more,
        })
    }
}
