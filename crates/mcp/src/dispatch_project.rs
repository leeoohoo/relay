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
                let mut input = input;
                self.normalize_company_project_input(agent_id, &mut input)?;
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

    fn normalize_company_project_input(&self, agent_id: Uuid, input: &mut Value) -> AppResult<()> {
        let Some(fields) = input.as_object_mut() else {
            return Err(AppError::Validation(
                "company.project input must be a JSON object".into(),
            ));
        };
        let action = fields
            .get("action")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                AppError::Validation(
                    "company.project requires action; inspect the tool schema and send one complete request"
                        .into(),
                )
            })?;
        let required = match action {
            "create" => &["company_id", "name", "member_agent_ids"][..],
            "list" => &["company_id"][..],
            "git_provision" | "update" | "get" => &["company_id", "project_id"][..],
            "member_add" | "member_remove" => {
                &["company_id", "project_id", "target_agent_id"][..]
            }
            "owner_transfer" => &["company_id", "project_id", "owner_agent_id"][..],
            "status_update" => &[
                "company_id",
                "project_id",
                "summary",
                "progress_percent",
            ][..],
            "rule_update" => &["company_id", "project_id", "content"][..],
            "assets_replace" => &["company_id", "project_id", "assets"][..],
            _ => {
                return Err(AppError::Validation(format!(
                    "unknown company.project action {action:?}; inspect the current tool schema for allowed actions"
                )))
            }
        };
        let missing = required
            .iter()
            .filter(|field| {
                fields.get(**field).is_none_or(|value| {
                    value.is_null() || value.as_str().is_some_and(|value| value.trim().is_empty())
                })
            })
            .copied()
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            return Err(AppError::Validation(format!(
                "company.project action {action:?} is incomplete; missing or empty fields: {}. Required fields: {}. Rebuild one complete request and retry once",
                missing.join(", "),
                required.join(", ")
            )));
        }

        let company_id = parse_uuid_field(fields, "company_id", action)?;
        if !required.contains(&"project_id") {
            return Ok(());
        }
        let project_reference = fields
            .get("project_id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                AppError::Validation(format!(
                    "company.project action {action:?} requires project_id as a full UUID from agent.bootstrap or company.project list"
                ))
            })?;
        if Uuid::parse_str(project_reference).is_ok() {
            return Ok(());
        }

        let projects = self.platform.list_company_projects(agent_id, company_id)?;
        let exact_name_matches = projects
            .iter()
            .filter(|project| {
                project
                    .project
                    .name
                    .trim()
                    .eq_ignore_ascii_case(project_reference)
            })
            .collect::<Vec<_>>();
        let resolved = if action == "get" && exact_name_matches.len() == 1 {
            Some(exact_name_matches[0].project.id)
        } else if action == "get" && projects.len() == 1 {
            Some(projects[0].project.id)
        } else {
            None
        };
        if let Some(project_id) = resolved {
            fields.insert("project_id".into(), Value::String(project_id.to_string()));
            return Ok(());
        }

        let candidates = projects
            .iter()
            .take(20)
            .map(|project| format!("{} ({})", project.project.name, project.project.id))
            .collect::<Vec<_>>();
        Err(AppError::Validation(format!(
            "company.project action {action:?} received invalid project_id {project_reference:?}. Use the full project UUID from agent.bootstrap or company.project list; do not use a list position, shortened ID, task ID, or Git commit. Visible projects: {}",
            if candidates.is_empty() {
                "none".into()
            } else {
                candidates.join(", ")
            }
        )))
    }
}

fn parse_uuid_field(
    fields: &serde_json::Map<String, Value>,
    field: &str,
    action: &str,
) -> AppResult<Uuid> {
    let value = fields
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::Validation(format!(
                "company.project action {action:?} requires {field} as a full UUID"
            ))
        })?;
    Uuid::parse_str(value).map_err(|_| {
        AppError::Validation(format!(
            "company.project action {action:?} received invalid {field} {value:?}; use the full UUID returned by agent.bootstrap"
        ))
    })
}
