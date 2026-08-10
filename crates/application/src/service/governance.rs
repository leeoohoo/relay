use super::*;

pub trait GovernancePlatformRepository: Send + Sync {
    fn publish_company_governance_policy_version(
        &self,
        _version: CompanyGovernancePolicyVersion,
    ) -> AppResult<CompanyGovernancePolicyVersion> {
        Err(AppError::Validation(
            "company governance policies are not supported by this repository".into(),
        ))
    }
    fn get_active_company_governance_policy_version(
        &self,
        _company_id: Uuid,
    ) -> Option<CompanyGovernancePolicyVersion> {
        None
    }
    fn list_company_governance_policy_versions(
        &self,
        _company_id: Uuid,
    ) -> Vec<CompanyGovernancePolicyVersion> {
        Vec::new()
    }
    fn insert_agent_tool_approval_request(
        &self,
        _request: AgentToolApprovalRequest,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "agent tool approvals are not supported by this repository".into(),
        ))
    }
    fn get_agent_tool_approval_request(
        &self,
        _approval_request_id: Uuid,
    ) -> Option<AgentToolApprovalRequest> {
        None
    }
    fn list_company_agent_tool_approval_requests(
        &self,
        _company_id: Uuid,
        _status: Option<&str>,
        _limit: usize,
    ) -> Vec<AgentToolApprovalRequest> {
        Vec::new()
    }
    fn claim_agent_tool_approval_request(
        &self,
        _approval_request_id: Uuid,
        _human_user_id: Uuid,
        _status: &str,
        _review_note: &str,
        _now: DateTime<Utc>,
    ) -> AppResult<Option<AgentToolApprovalRequest>> {
        Ok(None)
    }
    fn update_agent_tool_approval_request(
        &self,
        _request: AgentToolApprovalRequest,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "agent tool approvals are not supported by this repository".into(),
        ))
    }
    fn count_agent_tool_approval_requests_since(
        &self,
        _agent_id: Uuid,
        _since: DateTime<Utc>,
    ) -> usize {
        0
    }
    fn find_codex_always_allow_approval(
        &self,
        _company_id: Uuid,
        _agent_id: Uuid,
        _tool_name: &str,
        _approval_scope: &str,
        _approval_target: &str,
    ) -> AppResult<Option<AgentToolApprovalRequest>> {
        Ok(None)
    }
}
