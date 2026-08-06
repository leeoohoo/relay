use super::handler::parse_input;
use super::*;

impl<R: PlatformRepository, V: OwnershipProofVerifier> McpGateway<R, V> {
    pub(super) fn execute_company_staff_tool(
        &self,
        agent_id: Uuid,
        tool_name: &str,
        input: Value,
        idempotency_key: Option<String>,
    ) -> AppResult<Value> {
        match tool_name {
            "company.staff" => {
                let input: CompanyStaffToolInput = parse_input(input)?;
                let _ = input.idempotency_key;
                match input.operation {
                    CompanyStaffOperation::Hire {
                        company_id,
                        display_name,
                        handle,
                        persona,
                        org_unit_id,
                        profession_key,
                        reports_to_membership_id,
                        reason,
                    } => {
                        let profession = company_profession_by_key(profession_key.as_str())
                            .ok_or_else(|| {
                                AppError::Validation("unsupported profession_key".into())
                            })?;
                        let result = self.platform.hire_company_agent(AgentStaffingHireInput {
                            actor_agent_id: agent_id,
                            company_id,
                            display_name,
                            handle,
                            persona,
                            org_unit_id,
                            job_title: Some(profession.label),
                            reports_to_membership_id,
                            reason,
                            idempotency_key,
                        })?;
                        Ok(json!({ "result": result }))
                    }
                    CompanyStaffOperation::Suspend {
                        company_id,
                        target_agent_id,
                        reason,
                        handoff_plan,
                        handoff_agent_id,
                    } => {
                        let result = self.platform.suspend_company_agent_as_agent(
                            AgentStaffingStatusInput {
                                actor_agent_id: agent_id,
                                company_id,
                                target_agent_id,
                                reason,
                                handoff_plan,
                                handoff_agent_id,
                                idempotency_key,
                            },
                        )?;
                        Ok(json!({ "result": result }))
                    }
                    CompanyStaffOperation::Terminate {
                        company_id,
                        target_agent_id,
                        reason,
                        handoff_plan,
                        handoff_agent_id,
                    } => {
                        let result = self.platform.terminate_company_agent_as_agent(
                            AgentStaffingStatusInput {
                                actor_agent_id: agent_id,
                                company_id,
                                target_agent_id,
                                reason,
                                handoff_plan,
                                handoff_agent_id,
                                idempotency_key,
                            },
                        )?;
                        Ok(json!({ "result": result }))
                    }
                    CompanyStaffOperation::ActionList { company_id } => {
                        let staffing_actions = self
                            .platform
                            .list_company_staffing_actions_for_agent(agent_id, company_id)?;
                        Ok(json!({ "staffing_actions": staffing_actions }))
                    }
                    CompanyStaffOperation::ActionGet {
                        company_id,
                        action_id,
                    } => {
                        let staffing_action = self.platform.get_company_staffing_action_for_agent(
                            agent_id, company_id, action_id,
                        )?;
                        Ok(json!({ "staffing_action": staffing_action }))
                    }
                }
            }

            _ => Err(AppError::NotFound(format!(
                "unknown standard MCP tool: {tool_name}"
            ))),
        }
    }
}
