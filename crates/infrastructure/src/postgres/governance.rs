use super::mapping::*;
use super::*;

impl GovernancePlatformRepository for PostgresPlatformRepository {
    fn publish_company_governance_policy_version(
        &self,
        mut version: CompanyGovernancePolicyVersion,
    ) -> AppResult<CompanyGovernancePolicyVersion> {
        self.with_client(|client| {
            let mut tx = client.transaction()?;
            tx.query_one(
                "SELECT id FROM companies WHERE id = $1 FOR UPDATE",
                &[&version.company_id],
            )?;
            if let Some(row) = tx.query_opt(
                r#"
                SELECT id, company_id, version, status, settings, notes,
                       created_by_human_user_id, updated_by_human_user_id,
                       created_at, updated_at
                FROM company_governance_policy_versions
                WHERE company_id = $1 AND status = 'active'
                "#,
                &[&version.company_id],
            )? {
                let active = map_company_governance_policy_version(row);
                if active.settings == version.settings {
                    tx.commit()?;
                    return Ok(active);
                }
            }
            let next_version: i32 = tx
                .query_one(
                    r#"
                    SELECT COALESCE(MAX(version), 0)::INTEGER + 1 AS next_version
                    FROM company_governance_policy_versions
                    WHERE company_id = $1
                    "#,
                    &[&version.company_id],
                )?
                .get("next_version");
            tx.execute(
                r#"
                UPDATE company_governance_policy_versions
                SET status = 'archived',
                    updated_by_human_user_id = $2,
                    updated_at = $3
                WHERE company_id = $1 AND status = 'active'
                "#,
                &[
                    &version.company_id,
                    &version.updated_by_human_user_id,
                    &version.updated_at,
                ],
            )?;
            version.version = next_version;
            tx.execute(
                r#"
                INSERT INTO company_governance_policy_versions (
                    id, company_id, version, status, settings, notes,
                    created_by_human_user_id, updated_by_human_user_id,
                    created_at, updated_at
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                "#,
                &[
                    &version.id,
                    &version.company_id,
                    &version.version,
                    &version.status,
                    &Json(version.settings.clone()),
                    &version.notes,
                    &version.created_by_human_user_id,
                    &version.updated_by_human_user_id,
                    &version.created_at,
                    &version.updated_at,
                ],
            )?;
            tx.commit()?;
            Ok(version)
        })
    }

    fn get_active_company_governance_policy_version(
        &self,
        company_id: Uuid,
    ) -> Option<CompanyGovernancePolicyVersion> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT id, company_id, version, status, settings, notes,
                       created_by_human_user_id, updated_by_human_user_id,
                       created_at, updated_at
                FROM company_governance_policy_versions
                WHERE company_id = $1 AND status = 'active'
                "#,
                &[&company_id],
            )
        })
        .ok()
        .flatten()
        .map(map_company_governance_policy_version)
    }

    fn list_company_governance_policy_versions(
        &self,
        company_id: Uuid,
    ) -> Vec<CompanyGovernancePolicyVersion> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT id, company_id, version, status, settings, notes,
                       created_by_human_user_id, updated_by_human_user_id,
                       created_at, updated_at
                FROM company_governance_policy_versions
                WHERE company_id = $1
                ORDER BY version DESC
                "#,
                &[&company_id],
            )
        })
        .unwrap_or_default()
        .into_iter()
        .map(map_company_governance_policy_version)
        .collect()
    }

    fn insert_agent_tool_approval_request(
        &self,
        request: AgentToolApprovalRequest,
    ) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO agent_tool_approval_requests (
                    id, company_id, approval_source, runtime_config_id, runtime_run_id,
                    codex_trigger_run_id,
                    requested_by_agent_id, tool_name, risk_level, reason, arguments,
                    status, expires_at, reviewed_by_human_user_id, review_note,
                    reviewed_at, execution_result, error_message, created_at, updated_at
                ) VALUES (
                    $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11,
                    $12, $13, $14, $15, $16, $17, $18, $19, $20
                )
                "#,
                &[
                    &request.id,
                    &request.company_id,
                    &request.approval_source,
                    &request.runtime_config_id,
                    &request.runtime_run_id,
                    &request.codex_trigger_run_id,
                    &request.requested_by_agent_id,
                    &request.tool_name,
                    &request.risk_level,
                    &request.reason,
                    &Json(request.arguments.clone()),
                    &request.status,
                    &request.expires_at,
                    &request.reviewed_by_human_user_id,
                    &request.review_note,
                    &request.reviewed_at,
                    &Json(request.execution_result.clone()),
                    &request.error_message,
                    &request.created_at,
                    &request.updated_at,
                ],
            )?;
            Ok(())
        })
    }

    fn get_agent_tool_approval_request(
        &self,
        approval_request_id: Uuid,
    ) -> Option<AgentToolApprovalRequest> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT id, company_id, approval_source, runtime_config_id, runtime_run_id,
                       codex_trigger_run_id,
                       requested_by_agent_id, tool_name, risk_level, reason, arguments,
                       status, expires_at, reviewed_by_human_user_id, review_note,
                       reviewed_at, execution_result, error_message, created_at, updated_at
                FROM agent_tool_approval_requests
                WHERE id = $1
                "#,
                &[&approval_request_id],
            )
        })
        .ok()
        .flatten()
        .map(map_agent_tool_approval_request)
    }

    fn list_company_agent_tool_approval_requests(
        &self,
        company_id: Uuid,
        status: Option<&str>,
        limit: usize,
    ) -> Vec<AgentToolApprovalRequest> {
        let status = status.map(str::to_string);
        self.with_client(|client| {
            client.query(
                r#"
                SELECT id, company_id, approval_source, runtime_config_id, runtime_run_id,
                       codex_trigger_run_id,
                       requested_by_agent_id, tool_name, risk_level, reason, arguments,
                       status, expires_at, reviewed_by_human_user_id, review_note,
                       reviewed_at, execution_result, error_message, created_at, updated_at
                FROM agent_tool_approval_requests
                WHERE company_id = $1
                  AND ($2::TEXT IS NULL OR status = $2)
                ORDER BY created_at DESC
                LIMIT $3
                "#,
                &[&company_id, &status, &(limit as i64)],
            )
        })
        .unwrap_or_default()
        .into_iter()
        .map(map_agent_tool_approval_request)
        .collect()
    }

    fn claim_agent_tool_approval_request(
        &self,
        approval_request_id: Uuid,
        human_user_id: Uuid,
        status: &str,
        review_note: &str,
        now: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<Option<AgentToolApprovalRequest>> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                UPDATE agent_tool_approval_requests
                SET status = $3,
                    reviewed_by_human_user_id = $2,
                    review_note = $4,
                    reviewed_at = $5,
                    updated_at = $5
                WHERE id = $1
                  AND status = 'pending'
                  AND expires_at > $5
                RETURNING id, company_id, approval_source, runtime_config_id, runtime_run_id,
                          codex_trigger_run_id,
                          requested_by_agent_id, tool_name, risk_level, reason, arguments,
                          status, expires_at, reviewed_by_human_user_id, review_note,
                          reviewed_at, execution_result, error_message, created_at, updated_at
                "#,
                &[
                    &approval_request_id,
                    &human_user_id,
                    &status,
                    &review_note,
                    &now,
                ],
            )
        })
        .map(|row| row.map(map_agent_tool_approval_request))
    }

    fn update_agent_tool_approval_request(
        &self,
        request: AgentToolApprovalRequest,
    ) -> AppResult<()> {
        let updated = self.with_client(|client| {
            client.execute(
                r#"
                UPDATE agent_tool_approval_requests
                SET status = $2,
                    reviewed_by_human_user_id = $3,
                    review_note = $4,
                    reviewed_at = $5,
                    execution_result = $6,
                    error_message = $7,
                    updated_at = $8
                WHERE id = $1
                "#,
                &[
                    &request.id,
                    &request.status,
                    &request.reviewed_by_human_user_id,
                    &request.review_note,
                    &request.reviewed_at,
                    &Json(request.execution_result.clone()),
                    &request.error_message,
                    &request.updated_at,
                ],
            )
        })?;
        if updated == 0 {
            return Err(AppError::NotFound(
                "agent tool approval request not found".into(),
            ));
        }
        Ok(())
    }

    fn count_agent_tool_approval_requests_since(
        &self,
        agent_id: Uuid,
        since: chrono::DateTime<chrono::Utc>,
    ) -> usize {
        self.with_client(|client| {
            client.query_one(
                "SELECT COUNT(*)::BIGINT AS count FROM agent_tool_approval_requests WHERE requested_by_agent_id = $1 AND created_at >= $2",
                &[&agent_id, &since],
            )
        })
        .map(|row| row.get::<_, i64>("count").max(0) as usize)
        .unwrap_or(0)
    }

    fn find_codex_always_allow_approval(
        &self,
        company_id: Uuid,
        agent_id: Uuid,
        tool_name: &str,
        approval_scope: &str,
        approval_target: &str,
    ) -> AppResult<Option<AgentToolApprovalRequest>> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT id, company_id, approval_source, runtime_config_id, runtime_run_id,
                       codex_trigger_run_id,
                       requested_by_agent_id, tool_name, risk_level, reason, arguments,
                       status, expires_at, reviewed_by_human_user_id, review_note,
                       reviewed_at, execution_result, error_message, created_at, updated_at
                FROM agent_tool_approval_requests
                WHERE company_id = $1
                  AND requested_by_agent_id = $2
                  AND tool_name = $3
                  AND approval_source = 'codex'
                  AND status IN ('approved', 'executed')
                  AND execution_result ->> 'approval_mode' = 'always'
                  AND execution_result ->> 'approval_scope' = $4
                  AND execution_result ->> 'approval_target' = $5
                ORDER BY reviewed_at DESC NULLS LAST
                LIMIT 1
                "#,
                &[
                    &company_id,
                    &agent_id,
                    &tool_name,
                    &approval_scope,
                    &approval_target,
                ],
            )
        })
        .map(|row| row.map(map_agent_tool_approval_request))
    }
}
