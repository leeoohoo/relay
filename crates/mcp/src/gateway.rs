use super::tools::{
    action_status_for_error, audit_action_name, failure_target_ref, idempotency_key_from_input,
    input_without_idempotency, is_mutating_tool, is_public_tool_name, success_target_ref,
};
use super::*;

impl<R: PlatformRepository, V: OwnershipProofVerifier> McpGateway<R, V> {
    pub fn new(platform: PlatformApp<R, V>, fallback_agent_key: Option<String>) -> Self {
        Self {
            platform,
            fallback_agent_key,
            project_git_provisioner: None,
        }
    }

    pub fn with_project_git_provisioner(
        mut self,
        provisioner: Option<Arc<dyn ProjectGitProvisioner>>,
    ) -> Self {
        self.project_git_provisioner = provisioner;
        self
    }

    pub(super) fn provision_project_git(
        &self,
        agent_id: Uuid,
        company_id: Uuid,
        project: &ai_chat_application::CompanyProjectView,
    ) -> AppResult<ProvisionedProjectGit> {
        if project.project.status == PROJECT_STATUS_PAUSED {
            return Err(AppError::Conflict(
                "project is paused; resume it before provisioning Git".into(),
            ));
        }
        let provisioner = self.project_git_provisioner.as_ref().ok_or_else(|| {
            AppError::Validation("Harness project repository provisioning is not configured".into())
        })?;
        let provisioned = provisioner.provision(ProjectGitProvisionRequest {
            company_id,
            project_id: project.project.id,
            project_name: project.project.name.clone(),
            description: project.project.description.clone(),
        })?;
        self.platform
            .upsert_company_project_git(UpsertCompanyProjectGitInput {
                actor_agent_id: agent_id,
                company_id,
                project_id: project.project.id,
                remote_url: provisioned.remote_url.clone(),
                host_local_path: None,
                default_branch: Some(provisioned.default_branch.clone()),
                auth_profile: Some(provisioned.auth_profile.clone()),
                allow_agent_push: Some(true),
                branch_prefix: Some("relay/".into()),
            })?;
        Ok(provisioned)
    }

    pub fn invoke(
        &self,
        agent_key: Option<&str>,
        tool_name: &str,
        input: Value,
    ) -> AppResult<McpInvocation> {
        self.invoke_with_credentials(agent_key, None, tool_name, input)
    }

    pub fn invoke_with_credentials(
        &self,
        agent_key: Option<&str>,
        agent_run_token: Option<&str>,
        tool_name: &str,
        input: Value,
    ) -> AppResult<McpInvocation> {
        if !is_public_tool_name(tool_name) {
            return Err(AppError::NotFound(format!("unknown MCP tool: {tool_name}")));
        }
        let agent = self.authenticate_agent(agent_key, agent_run_token)?;
        let request_input = input.clone();
        let idempotency_key = idempotency_key_from_input(&request_input);
        let is_mutating = is_mutating_tool(tool_name, &request_input);
        if is_mutating {
            if let Some(key) = idempotency_key.as_deref() {
                if let Some(output) = self.platform.replay_agent_idempotency(
                    agent.id,
                    tool_name,
                    key,
                    &request_input,
                )? {
                    return Ok(McpInvocation {
                        tool: tool_name.to_string(),
                        agent_id: agent.id,
                        output,
                    });
                }
            }
            self.platform.enforce_agent_action_budget(agent.id)?;
        }
        let result = self.execute(
            agent.id,
            tool_name,
            input_without_idempotency(input),
            idempotency_key.clone(),
        );

        match result {
            Ok(output) => {
                if is_mutating {
                    let action_name = audit_action_name(tool_name, &request_input);
                    self.platform.record_agent_action(
                        agent.id,
                        action_name,
                        success_target_ref(tool_name, &request_input, &output),
                        request_input.clone(),
                        output.clone(),
                        AgentActionStatus::Success,
                    )?;
                    if let Some(key) = idempotency_key.as_deref() {
                        self.platform.store_agent_idempotency(
                            agent.id,
                            tool_name,
                            key,
                            &request_input,
                            &output,
                        )?;
                    }
                }

                Ok(McpInvocation {
                    tool: tool_name.to_string(),
                    agent_id: agent.id,
                    output,
                })
            }
            Err(error) => {
                if is_mutating {
                    let action_name = audit_action_name(tool_name, &request_input);
                    let _ = self.platform.record_agent_action(
                        agent.id,
                        action_name,
                        failure_target_ref(tool_name, &request_input),
                        request_input,
                        json!({
                            "code": error.code(),
                            "message": error.to_string(),
                        }),
                        action_status_for_error(&error),
                    );
                }
                Err(error)
            }
        }
    }

    pub(super) fn authenticate_agent(
        &self,
        agent_key: Option<&str>,
        agent_run_token: Option<&str>,
    ) -> AppResult<ai_chat_domain::agent_identity::AgentProfile> {
        if let Some(agent_key) = agent_key.filter(|value| !value.trim().is_empty()) {
            return self.platform.authenticate_agent_key(agent_key);
        }
        if let Some(run_token) = agent_run_token.filter(|value| !value.trim().is_empty()) {
            return self.platform.authenticate_agent_codex_run_token(run_token);
        }
        if let Some(agent_key) = self.fallback_agent_key.as_deref() {
            return self.platform.authenticate_agent_key(agent_key);
        }
        Err(AppError::Unauthorized(
            "missing x-agent-key, bearer token, or x-agent-run-token".into(),
        ))
    }
}
