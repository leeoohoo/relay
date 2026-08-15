use super::handler::parse_input;
use super::*;

impl<R: PlatformRepository, V: OwnershipProofVerifier> McpGateway<R, V> {
    pub(super) fn execute_company_gate_tool(
        &self,
        agent_id: Uuid,
        input: Value,
    ) -> AppResult<Value> {
        let input: CompanyGateToolInput = parse_input(input)?;
        let _ = input.idempotency_key;
        match input.operation {
            CompanyGateOperation::List {
                company_id,
                project_id,
            } => Ok(json!({
                "gates": self.platform.list_project_gates(ListProjectGatesInput {
                    actor_agent_id: agent_id,
                    company_id,
                    project_id,
                })?,
                "requirements": self.platform.list_project_gate_requirements(ListProjectGatesInput {
                    actor_agent_id: agent_id,
                    company_id,
                    project_id,
                })?,
            })),
            CompanyGateOperation::Create {
                company_id,
                project_id,
                gate_key,
                gate_type,
                title,
                related_task_id,
                required_evidence,
            } => Ok(json!({
                "gate": self.platform.create_project_gate(CreateProjectGateInput {
                    actor_agent_id: agent_id,
                    company_id,
                    project_id,
                    gate_key,
                    gate_type,
                    title,
                    related_task_id,
                    required_evidence,
                })?
            })),
            CompanyGateOperation::Decide {
                company_id,
                project_id,
                gate_id,
                status,
                decision_summary,
            } => Ok(json!({
                "gate": self.platform.decide_project_gate(DecideProjectGateInput {
                    actor_agent_id: agent_id,
                    company_id,
                    project_id,
                    gate_id,
                    status,
                    decision_summary,
                })?
            })),
            CompanyGateOperation::RequirementSet {
                company_id,
                project_id,
                task_id,
                gate_id,
                required_status,
            } => Ok(json!({
                "requirement": self.platform.set_project_task_gate_requirement(
                    SetProjectTaskGateRequirementInput {
                        actor_agent_id: agent_id,
                        company_id,
                        project_id,
                        task_id,
                        gate_id,
                        required_status,
                    }
                )?
            })),
        }
    }
}
