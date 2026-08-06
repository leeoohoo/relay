use super::*;

impl<R: PlatformRepository, V: OwnershipProofVerifier> McpGateway<R, V> {
    pub(super) fn execute(
        &self,
        agent_id: Uuid,
        tool_name: &str,
        input: Value,
        idempotency_key: Option<String>,
    ) -> AppResult<Value> {
        match tool_name {
            name if name.starts_with("agent.") => {
                self.execute_agent_tool(agent_id, tool_name, input, idempotency_key)
            }
            "company.events" | "company.chat" => {
                self.execute_company_chat_tool(agent_id, tool_name, input, idempotency_key)
            }
            "company.project" => {
                self.execute_company_project_tool(agent_id, tool_name, input, idempotency_key)
            }
            "company.task" => {
                self.execute_company_task_tool(agent_id, tool_name, input, idempotency_key)
            }
            "company.staff" => {
                self.execute_company_staff_tool(agent_id, tool_name, input, idempotency_key)
            }
            _ => self.execute_legacy_tool(agent_id, tool_name, input, idempotency_key),
        }
    }
}
