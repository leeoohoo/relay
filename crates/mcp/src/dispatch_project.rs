use super::handler::parse_input;
use super::*;

impl<R: PlatformRepository, V: OwnershipProofVerifier> McpGateway<R, V> {
    pub(super) fn execute_company_project_tool(
        &self,
        agent_id: Uuid,
        tool_name: &str,
        input: Value,
        _idempotency_key: Option<String>,
    ) -> AppResult<Value> {
        match tool_name {
            "company.project" => {
                let input: CompanyProjectToolInput = parse_input(input)?;
                let _ = input.idempotency_key;
                match input.operation {
                    CompanyProjectOperation::Create {
                        company_id,
                        name,
                        description,
                        member_agent_ids,
                    } => {
                        let project =
                            self.platform
                                .create_company_project(CreateCompanyProjectInput {
                                    actor_agent_id: agent_id,
                                    company_id,
                                    name,
                                    description,
                                    member_agent_ids,
                                })?;
                        let git_provisioning =
                            match self.provision_project_git(agent_id, company_id, &project) {
                                Ok(git) => json!({ "status": "ready", "git": git }),
                                Err(error) => json!({
                                    "status": "failed",
                                    "code": error.code(),
                                    "message": error.to_string(),
                                    "retry_action": "git_provision"
                                }),
                            };
                        let project =
                            self.platform.get_company_project(GetCompanyProjectInput {
                                actor_agent_id: agent_id,
                                company_id,
                                project_id: project.project.id,
                            })?;
                        Ok(json!({
                            "project": project,
                            "git_provisioning": git_provisioning
                        }))
                    }
                    CompanyProjectOperation::GitProvision {
                        company_id,
                        project_id,
                    } => {
                        let project =
                            self.platform.get_company_project(GetCompanyProjectInput {
                                actor_agent_id: agent_id,
                                company_id,
                                project_id,
                            })?;
                        let git = self.provision_project_git(agent_id, company_id, &project)?;
                        let project =
                            self.platform.get_company_project(GetCompanyProjectInput {
                                actor_agent_id: agent_id,
                                company_id,
                                project_id,
                            })?;
                        Ok(json!({ "project": project, "git": git }))
                    }
                    CompanyProjectOperation::Update {
                        company_id,
                        project_id,
                        name,
                        description,
                        due_at,
                        clear_due_at,
                    } => {
                        let project =
                            self.platform
                                .update_company_project(UpdateCompanyProjectInput {
                                    actor_agent_id: agent_id,
                                    company_id,
                                    project_id,
                                    name,
                                    description,
                                    due_at,
                                    clear_due_at,
                                })?;
                        Ok(json!({ "project": project }))
                    }
                    CompanyProjectOperation::Get {
                        company_id,
                        project_id,
                    } => {
                        let project =
                            self.platform.get_company_project(GetCompanyProjectInput {
                                actor_agent_id: agent_id,
                                company_id,
                                project_id,
                            })?;
                        Ok(json!({ "project": project }))
                    }
                    CompanyProjectOperation::List { company_id } => {
                        let projects = self.platform.list_company_projects(agent_id, company_id)?;
                        Ok(json!({ "projects": projects }))
                    }
                    CompanyProjectOperation::MemberAdd {
                        company_id,
                        project_id,
                        target_agent_id,
                    } => {
                        let project = self.platform.add_company_project_member(
                            AddCompanyProjectMemberInput {
                                actor_agent_id: agent_id,
                                company_id,
                                project_id,
                                target_agent_id,
                            },
                        )?;
                        Ok(json!({ "project": project }))
                    }
                    CompanyProjectOperation::MemberRemove {
                        company_id,
                        project_id,
                        target_agent_id,
                    } => {
                        let project = self.platform.remove_company_project_member(
                            RemoveCompanyProjectMemberInput {
                                actor_agent_id: agent_id,
                                company_id,
                                project_id,
                                target_agent_id,
                            },
                        )?;
                        Ok(json!({ "project": project }))
                    }
                    CompanyProjectOperation::OwnerTransfer {
                        company_id,
                        project_id,
                        owner_agent_id,
                    } => {
                        let project = self.platform.transfer_company_project_owner(
                            TransferCompanyProjectOwnerInput {
                                actor_agent_id: agent_id,
                                company_id,
                                project_id,
                                owner_agent_id,
                            },
                        )?;
                        Ok(json!({ "project": project }))
                    }
                    CompanyProjectOperation::StatusUpdate {
                        company_id,
                        project_id,
                        summary,
                        progress_percent,
                        blockers,
                        next_steps,
                        project_status,
                    } => {
                        let status_update = self.platform.create_company_project_status_update(
                            CreateCompanyProjectStatusUpdateInput {
                                actor_agent_id: agent_id,
                                company_id,
                                project_id,
                                summary,
                                progress_percent,
                                blockers,
                                next_steps,
                                project_status,
                            },
                        )?;
                        Ok(json!({ "status_update": status_update }))
                    }
                    CompanyProjectOperation::RuleUpdate {
                        company_id,
                        project_id,
                        content,
                    } => {
                        let rule = self.platform.update_company_project_rule(
                            UpdateCompanyProjectRuleInput {
                                actor_agent_id: agent_id,
                                company_id,
                                project_id,
                                content,
                            },
                        )?;
                        Ok(json!({ "rule": rule }))
                    }
                    CompanyProjectOperation::AssetsReplace {
                        company_id,
                        project_id,
                        assets,
                    } => {
                        let assets = self.platform.replace_company_project_assets(
                            ReplaceCompanyProjectAssetsInput {
                                actor_agent_id: agent_id,
                                company_id,
                                project_id,
                                assets: assets
                                    .into_iter()
                                    .map(|asset| CompanyProjectAssetInput {
                                        name: asset.name,
                                        asset_type: asset.asset_type,
                                        locator: asset.locator,
                                        description: asset.description,
                                        status: asset
                                            .status
                                            .map(CompanyProjectAssetStatusInput::into_string),
                                        metadata: asset.metadata.map(|metadata| {
                                            Value::Object(metadata.into_iter().collect())
                                        }),
                                    })
                                    .collect(),
                            },
                        )?;
                        Ok(json!({ "assets": assets, "refresh_completed": true }))
                    }
                }
            }

            _ => Err(AppError::NotFound(format!(
                "unknown standard MCP tool: {tool_name}"
            ))),
        }
    }
}
