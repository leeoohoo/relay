use super::*;
use ai_chat_domain::company::AGENT_TOOL_APPROVAL_MODE_ALWAYS_LOCALHOST;

impl GovernancePlatformRepository for MemoryPlatformRepository {
    fn publish_company_governance_policy_version(
        &self,
        mut version: CompanyGovernancePolicyVersion,
    ) -> AppResult<CompanyGovernancePolicyVersion> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if let Some(active) = guard
            .company_governance_policy_versions
            .values()
            .find(|existing| {
                existing.company_id == version.company_id
                    && existing.status == COMPANY_GOVERNANCE_POLICY_STATUS_ACTIVE
                    && existing.settings == version.settings
            })
            .cloned()
        {
            return Ok(active);
        }
        let next_version = guard
            .company_governance_policy_versions
            .values()
            .filter(|existing| existing.company_id == version.company_id)
            .map(|existing| existing.version)
            .max()
            .unwrap_or(0)
            + 1;
        for existing in guard.company_governance_policy_versions.values_mut() {
            if existing.company_id == version.company_id
                && existing.status == COMPANY_GOVERNANCE_POLICY_STATUS_ACTIVE
            {
                existing.status = COMPANY_GOVERNANCE_POLICY_STATUS_ARCHIVED.into();
                existing.updated_by_human_user_id = version.updated_by_human_user_id;
                existing.updated_at = version.updated_at;
            }
        }
        version.version = next_version;
        guard
            .company_governance_policy_versions
            .insert(version.id, version.clone());
        Ok(version)
    }

    fn get_active_company_governance_policy_version(
        &self,
        company_id: Uuid,
    ) -> Option<CompanyGovernancePolicyVersion> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard
            .company_governance_policy_versions
            .values()
            .find(|version| {
                version.company_id == company_id
                    && version.status == COMPANY_GOVERNANCE_POLICY_STATUS_ACTIVE
            })
            .cloned()
    }

    fn list_company_governance_policy_versions(
        &self,
        company_id: Uuid,
    ) -> Vec<CompanyGovernancePolicyVersion> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut versions = guard
            .company_governance_policy_versions
            .values()
            .filter(|version| version.company_id == company_id)
            .cloned()
            .collect::<Vec<_>>();
        versions.sort_by(|left, right| right.version.cmp(&left.version));
        versions
    }

    fn insert_agent_tool_approval_request(
        &self,
        request: AgentToolApprovalRequest,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if guard.agent_tool_approval_requests.contains_key(&request.id) {
            return Err(ai_chat_shared::AppError::Conflict(
                "agent tool approval request already exists".into(),
            ));
        }
        guard
            .agent_tool_approval_requests
            .insert(request.id, request);
        Ok(())
    }

    fn get_agent_tool_approval_request(
        &self,
        approval_request_id: Uuid,
    ) -> Option<AgentToolApprovalRequest> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard
            .agent_tool_approval_requests
            .get(&approval_request_id)
            .cloned()
    }

    fn list_company_agent_tool_approval_requests(
        &self,
        company_id: Uuid,
        status: Option<&str>,
        limit: usize,
    ) -> Vec<AgentToolApprovalRequest> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut requests = guard
            .agent_tool_approval_requests
            .values()
            .filter(|request| {
                request.company_id == company_id
                    && status.is_none_or(|status| request.status == status)
            })
            .cloned()
            .collect::<Vec<_>>();
        requests.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        requests.truncate(limit);
        requests
    }

    fn claim_agent_tool_approval_request(
        &self,
        approval_request_id: Uuid,
        human_user_id: Uuid,
        status: &str,
        review_note: &str,
        now: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<Option<AgentToolApprovalRequest>> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let Some(request) = guard
            .agent_tool_approval_requests
            .get_mut(&approval_request_id)
        else {
            return Ok(None);
        };
        if request.status != AGENT_TOOL_APPROVAL_STATUS_PENDING || request.expires_at <= now {
            return Ok(None);
        }
        request.status = status.to_string();
        request.reviewed_by_human_user_id = Some(human_user_id);
        request.review_note = review_note.to_string();
        request.reviewed_at = Some(now);
        request.updated_at = now;
        Ok(Some(request.clone()))
    }

    fn update_agent_tool_approval_request(
        &self,
        request: AgentToolApprovalRequest,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if !guard.agent_tool_approval_requests.contains_key(&request.id) {
            return Err(ai_chat_shared::AppError::NotFound(
                "agent tool approval request not found".into(),
            ));
        }
        guard
            .agent_tool_approval_requests
            .insert(request.id, request);
        Ok(())
    }

    fn count_agent_tool_approval_requests_since(
        &self,
        agent_id: Uuid,
        since: chrono::DateTime<chrono::Utc>,
    ) -> usize {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard
            .agent_tool_approval_requests
            .values()
            .filter(|request| {
                request.requested_by_agent_id == agent_id && request.created_at >= since
            })
            .count()
    }

    fn find_codex_always_allow_approval(
        &self,
        company_id: Uuid,
        agent_id: Uuid,
        tool_name: &str,
        approval_scope: &str,
        approval_target: &str,
    ) -> AppResult<Option<AgentToolApprovalRequest>> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        Ok(guard
            .agent_tool_approval_requests
            .values()
            .filter(|request| {
                request.company_id == company_id
                    && request.requested_by_agent_id == agent_id
                    && request.tool_name == tool_name
                    && request.approval_source == AGENT_TOOL_APPROVAL_SOURCE_CODEX
                    && matches!(
                        request.status.as_str(),
                        AGENT_TOOL_APPROVAL_STATUS_APPROVED | AGENT_TOOL_APPROVAL_STATUS_EXECUTED
                    )
                    && request
                        .execution_result
                        .get("approval_mode")
                        .and_then(Value::as_str)
                        .is_some_and(|mode| {
                            matches!(
                                mode,
                                AGENT_TOOL_APPROVAL_MODE_ALWAYS
                                    | AGENT_TOOL_APPROVAL_MODE_ALWAYS_LOCALHOST
                            )
                        })
                    && request
                        .execution_result
                        .get("approval_scope")
                        .and_then(Value::as_str)
                        == Some(approval_scope)
                    && request
                        .execution_result
                        .get("approval_target")
                        .and_then(Value::as_str)
                        == Some(approval_target)
            })
            .max_by_key(|request| request.reviewed_at)
            .cloned())
    }
}
