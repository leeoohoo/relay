use super::*;

pub trait CompanyPlatformRepository: Send + Sync {
    fn insert_company_bundle(&self, _bundle: CompanyCreationBundle) -> AppResult<()> {
        Err(AppError::Validation(
            "companies are not supported by this repository".into(),
        ))
    }
    fn get_company(&self, _company_id: Uuid) -> Option<Company> {
        None
    }
    fn get_company_result(&self, company_id: Uuid) -> AppResult<Option<Company>> {
        Ok(self.get_company(company_id))
    }
    fn get_company_by_slug(&self, _slug: &str) -> Option<Company> {
        None
    }
    fn list_human_companies(&self, _human_user_id: Uuid) -> Vec<Company> {
        Vec::new()
    }
    fn get_company_human_member(
        &self,
        _company_id: Uuid,
        _human_user_id: Uuid,
    ) -> Option<CompanyHumanMember> {
        None
    }
    fn get_company_human_member_result(
        &self,
        company_id: Uuid,
        human_user_id: Uuid,
    ) -> AppResult<Option<CompanyHumanMember>> {
        Ok(self.get_company_human_member(company_id, human_user_id))
    }
    fn get_org_unit(&self, _org_unit_id: Uuid) -> Option<OrgUnit> {
        None
    }
    fn insert_org_unit(&self, _org_unit: OrgUnit) -> AppResult<()> {
        Err(AppError::Validation(
            "organization units are not supported by this repository".into(),
        ))
    }
    fn list_company_org_units(&self, _company_id: Uuid) -> Vec<OrgUnit> {
        Vec::new()
    }
    fn get_company_agent_membership(&self, _agent_id: Uuid) -> Option<CompanyAgentMembership> {
        None
    }
    fn list_company_agent_memberships(&self, _company_id: Uuid) -> Vec<CompanyAgentMembership> {
        Vec::new()
    }
    fn list_company_agent_membership_page(
        &self,
        company_id: Uuid,
        after_membership_id: Option<Uuid>,
        limit: usize,
    ) -> AppResult<CursorPage<CompanyAgentMembership>> {
        let memberships = self.list_company_agent_memberships(company_id);
        cursor_page_by_id(memberships, after_membership_id, limit, |membership| {
            membership.id
        })
    }
    fn update_company_agent_work_profile(
        &self,
        _agent_id: Uuid,
        _responsibilities: Vec<String>,
        _skills: Vec<String>,
        _current_focus: String,
        _collaboration_preference: String,
        _updated_at: DateTime<Utc>,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "company agent work profiles are not supported by this repository".into(),
        ))
    }
    fn complete_company_agent_creation_bundle(
        &self,
        _bundle: CompanyAgentCreationBundle,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "company agent creation is not supported by this repository".into(),
        ))
    }
    fn complete_company_agent_membership_update(
        &self,
        _bundle: CompanyAgentMembershipUpdateBundle,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "company agent membership updates are not supported by this repository".into(),
        ))
    }
    fn complete_agent_staffing_hire(&self, _bundle: AgentStaffingHireBundle) -> AppResult<()> {
        Err(AppError::Validation(
            "agent staffing hires are not supported by this repository".into(),
        ))
    }
    fn complete_company_agent_activation(
        &self,
        _bundle: CompanyAgentActivationBundle,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "company agent activation is not supported by this repository".into(),
        ))
    }
    fn complete_agent_staffing_status_change(
        &self,
        _bundle: AgentStaffingStatusChangeBundle,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "agent staffing status changes are not supported by this repository".into(),
        ))
    }
    fn get_agent_staffing_action(&self, _action_id: Uuid) -> Option<AgentStaffingAction> {
        None
    }
    fn list_agent_staffing_actions(&self, _company_id: Uuid) -> Vec<AgentStaffingAction> {
        Vec::new()
    }
}
