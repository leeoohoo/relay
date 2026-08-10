use super::*;
use ai_chat_domain::company::AGENT_CODEX_WAKE_REASON_PROJECT_RESUMED;

impl<R: PlatformRepository, V: OwnershipProofVerifier> PlatformApp<R, V> {
    pub fn get_company_project_for_human_manager(
        &self,
        human_user_id: Uuid,
        company_id: Uuid,
        project_id: Uuid,
    ) -> AppResult<CompanyProjectView> {
        let project =
            self.ensure_company_project_for_human_manager(human_user_id, company_id, project_id)?;
        self.company_project_view(project)
    }

    pub fn create_company_project(
        &self,
        input: CreateCompanyProjectInput,
    ) -> AppResult<CompanyProjectView> {
        self.ensure_agent_can_act(input.actor_agent_id)?;
        self.ensure_company_agent_permission(
            input.company_id,
            input.actor_agent_id,
            COMPANY_PERMISSION_PROJECT_CREATE,
        )?;
        let description = normalize_optional_text(input.description).unwrap_or_default();
        let (project_type, project_type_confidence, project_type_evidence) =
            infer_company_project_type(&input.name, &description, &[]);
        self.create_company_project_record(
            input.actor_agent_id,
            input.company_id,
            input.name,
            description,
            input.member_agent_ids,
            project_type,
            PROJECT_TYPE_SOURCE_DESCRIPTION.into(),
            project_type_confidence,
            project_type_evidence,
            None,
        )
    }

    pub fn create_company_project_for_human(
        &self,
        input: CreateCompanyProjectForHumanInput,
    ) -> AppResult<CompanyProjectView> {
        let project_creation = self.prepare_company_project_for_human(input)?;
        self.complete_company_project_creation(project_creation, None, None)
            .map(|(project, _)| project)
    }

    pub(super) fn prepare_company_project_for_human(
        &self,
        input: CreateCompanyProjectForHumanInput,
    ) -> AppResult<CompanyProjectCreationBundle> {
        self.ensure_company_human_manager(input.company_id, input.human_user_id)?;
        self.ensure_agent_can_act(input.owner_agent_id)?;
        self.ensure_active_company_conversation_member(input.company_id, input.owner_agent_id)?;
        let description = normalize_optional_text(input.description).unwrap_or_default();
        let (inferred_type, inferred_confidence, inferred_evidence) =
            infer_company_project_type(&input.name, &description, &input.project_type_evidence);
        let project_type = input.project_type.unwrap_or(inferred_type);
        if company_project_type_by_key(&project_type).is_none() {
            return Err(AppError::Validation("unsupported project_type".into()));
        }
        let source = input.project_type_source.unwrap_or_else(|| {
            if input.project_type_evidence.is_empty() {
                PROJECT_TYPE_SOURCE_DESCRIPTION.into()
            } else {
                PROJECT_TYPE_SOURCE_FOLDER.into()
            }
        });
        if !matches!(
            source.as_str(),
            PROJECT_TYPE_SOURCE_HUMAN
                | PROJECT_TYPE_SOURCE_DESCRIPTION
                | PROJECT_TYPE_SOURCE_FOLDER
                | PROJECT_TYPE_SOURCE_SYSTEM
        ) {
            return Err(AppError::Validation(
                "unsupported project_type_source".into(),
            ));
        }
        self.prepare_company_project_record(
            input.owner_agent_id,
            input.company_id,
            input.name,
            description,
            input.member_agent_ids,
            project_type,
            source,
            input
                .project_type_confidence
                .unwrap_or(inferred_confidence)
                .clamp(0, 100),
            if input.project_type_evidence.is_empty() {
                inferred_evidence
            } else {
                input.project_type_evidence
            },
            input.project_id,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn create_company_project_record(
        &self,
        owner_agent_id: Uuid,
        company_id: Uuid,
        raw_name: String,
        description: String,
        requested_member_agent_ids: Vec<Uuid>,
        project_type: String,
        project_type_source: String,
        project_type_confidence: i32,
        project_type_evidence: Vec<String>,
        requested_project_id: Option<Uuid>,
    ) -> AppResult<CompanyProjectView> {
        let project_creation = self.prepare_company_project_record(
            owner_agent_id,
            company_id,
            raw_name,
            description,
            requested_member_agent_ids,
            project_type,
            project_type_source,
            project_type_confidence,
            project_type_evidence,
            requested_project_id,
        )?;
        self.complete_company_project_creation(project_creation, None, None)
            .map(|(project, _)| project)
    }

    #[allow(clippy::too_many_arguments)]
    fn prepare_company_project_record(
        &self,
        owner_agent_id: Uuid,
        company_id: Uuid,
        raw_name: String,
        description: String,
        requested_member_agent_ids: Vec<Uuid>,
        project_type: String,
        project_type_source: String,
        project_type_confidence: i32,
        project_type_evidence: Vec<String>,
        requested_project_id: Option<Uuid>,
    ) -> AppResult<CompanyProjectCreationBundle> {
        let governance = self.effective_company_governance_policy_settings(company_id);
        let active_project_count = self
            .repo
            .list_company_projects_result(company_id)?
            .into_iter()
            .filter(|project| {
                !matches!(
                    project.status.as_str(),
                    PROJECT_STATUS_COMPLETED | PROJECT_STATUS_CANCELLED
                )
            })
            .count();
        if active_project_count >= governance.max_active_projects as usize {
            return Err(AppError::RateLimited(format!(
                "company active project limit of {} has been reached",
                governance.max_active_projects
            )));
        }
        let name = raw_name.trim();
        if name.is_empty() || name.chars().count() > 120 {
            return Err(AppError::Validation(
                "project name must contain 1 to 120 characters".into(),
            ));
        }
        if description.chars().count() > 4000 {
            return Err(AppError::Validation(
                "project description must not exceed 4000 characters".into(),
            ));
        }
        let mut member_agent_ids = vec![owner_agent_id];
        for agent_id in requested_member_agent_ids {
            if !member_agent_ids.contains(&agent_id) {
                member_agent_ids.push(agent_id);
            }
        }
        if member_agent_ids.len() > governance.max_project_members as usize {
            return Err(AppError::Validation(format!(
                "project supports at most {} active members",
                governance.max_project_members
            )));
        }
        for agent_id in member_agent_ids.iter().copied() {
            self.ensure_active_company_conversation_member(company_id, agent_id)?;
        }

        let now = now_utc();
        let project_id = requested_project_id.unwrap_or_else(Uuid::new_v4);
        let conversation_id = Uuid::new_v4();
        let project = CompanyProject {
            id: project_id,
            company_id,
            name: name.to_string(),
            description,
            project_type,
            project_type_source,
            project_type_confidence,
            project_type_evidence: project_type_evidence.into_iter().take(24).collect(),
            status: PROJECT_STATUS_ACTIVE.into(),
            owner_agent_id,
            project_group_conversation_id: conversation_id,
            created_by_agent_id: owner_agent_id,
            updated_by_agent_id: None,
            due_at: None,
            created_at: now,
            updated_at: now,
            completed_at: None,
        };
        let preview = ConversationPreview {
            id: conversation_id,
            title: format!("项目 · {name}"),
            conversation_type: ConversationType::Group,
            last_message_preview: None,
            updated_at: now,
        };
        let members = member_agent_ids
            .iter()
            .copied()
            .map(|agent_id| CompanyProjectMember {
                id: Uuid::new_v4(),
                project_id,
                agent_profile_id: agent_id,
                role: if agent_id == owner_agent_id {
                    PROJECT_MEMBER_ROLE_OWNER.into()
                } else {
                    PROJECT_MEMBER_ROLE_MEMBER.into()
                },
                joined_at: now,
                left_at: None,
                added_by_agent_id: owner_agent_id,
            })
            .collect::<Vec<_>>();
        Ok(CompanyProjectCreationBundle {
            project,
            members,
            conversation_members: member_agent_ids
                .iter()
                .copied()
                .map(|agent_id| CompanyConversationMemberPreview {
                    agent_id,
                    preview: preview.clone(),
                })
                .collect(),
        })
    }

    pub(super) fn complete_company_project_creation(
        &self,
        project_creation: CompanyProjectCreationBundle,
        git_config: Option<CompanyProjectGitConfig>,
        cleanup_job_id: Option<Uuid>,
    ) -> AppResult<(CompanyProjectView, Option<CompanyProjectGitConfig>)> {
        let project = project_creation.project.clone();
        let member_agent_ids = project_creation
            .members
            .iter()
            .map(|member| member.agent_profile_id)
            .collect::<Vec<_>>();
        if let Some(git_config) = git_config.as_ref() {
            let cleanup_job_id = cleanup_job_id.ok_or_else(|| {
                AppError::Internal("managed project creation is missing its cleanup job".into())
            })?;
            self.repo.complete_managed_company_project_creation(
                ManagedCompanyProjectCreationBundle {
                    project_creation,
                    git_config: git_config.clone(),
                    cleanup_job_id,
                },
            )?;
        } else {
            self.repo
                .complete_company_project_creation(project_creation)?;
        }
        for agent_id in member_agent_ids {
            if agent_id != project.owner_agent_id {
                let name = project.name.as_str();
                let _ = self.enqueue_agent_event(
                    agent_id,
                    "company.project.member_added",
                    json!({
                        "company_id": project.company_id,
                        "project_id": project.id,
                        "project_name": name,
                        "conversation_id": project.project_group_conversation_id,
                        "added_by_agent_id": project.owner_agent_id,
                    }),
                    35,
                );
            }
        }
        Ok((self.company_project_view(project)?, git_config))
    }

    pub fn get_company_project(
        &self,
        input: GetCompanyProjectInput,
    ) -> AppResult<CompanyProjectView> {
        self.ensure_agent_can_act(input.actor_agent_id)?;
        let project = self.ensure_company_project_access(
            input.company_id,
            input.project_id,
            input.actor_agent_id,
        )?;
        self.company_project_view(project)
    }

    pub fn list_company_projects(
        &self,
        actor_agent_id: Uuid,
        company_id: Uuid,
    ) -> AppResult<Vec<CompanyProjectView>> {
        self.ensure_agent_can_act(actor_agent_id)?;
        let membership =
            self.ensure_active_company_conversation_member(company_id, actor_agent_id)?;
        let can_manage = membership
            .permissions
            .iter()
            .any(|permission| permission == COMPANY_PERMISSION_PROJECT_MANAGE);
        self.repo
            .list_company_projects_result(company_id)?
            .into_iter()
            .filter(|project| {
                can_manage
                    || self
                        .repo
                        .get_company_project_member(project.id, actor_agent_id)
                        .is_some_and(|member| member.left_at.is_none())
            })
            .map(|project| self.company_project_view(project))
            .collect()
    }

    pub fn list_company_project_tasks_for_human(
        &self,
        human_user_id: Uuid,
        company_id: Uuid,
    ) -> AppResult<Vec<CompanyProjectTask>> {
        let console = self.get_company_console(human_user_id, company_id)?;
        let mut tasks = console
            .projects
            .into_iter()
            .flat_map(|project| project.tasks)
            .collect::<Vec<_>>();
        tasks.sort_by(|left, right| {
            right
                .updated_at
                .cmp(&left.updated_at)
                .then_with(|| left.id.cmp(&right.id))
        });
        Ok(tasks)
    }

    pub fn update_company_project(
        &self,
        input: UpdateCompanyProjectInput,
    ) -> AppResult<CompanyProjectView> {
        self.ensure_agent_can_act(input.actor_agent_id)?;
        self.ensure_company_agent_permission(
            input.company_id,
            input.actor_agent_id,
            COMPANY_PERMISSION_PROJECT_MANAGE,
        )?;
        let mut project = self.ensure_company_project_access(
            input.company_id,
            input.project_id,
            input.actor_agent_id,
        )?;
        self.ensure_project_not_paused(&project)?;
        if input.name.is_none()
            && input.description.is_none()
            && input.due_at.is_none()
            && !input.clear_due_at
        {
            return Err(AppError::Validation(
                "project metadata update must change at least one field".into(),
            ));
        }
        if input.due_at.is_some() && input.clear_due_at {
            return Err(AppError::Validation(
                "project due_at and clear_due_at cannot be used together".into(),
            ));
        }
        if let Some(name) = input.name {
            let name = name.trim();
            if name.is_empty() || name.chars().count() > 120 {
                return Err(AppError::Validation(
                    "project name must contain 1 to 120 characters".into(),
                ));
            }
            project.name = name.to_string();
        }
        if let Some(description) = input.description {
            let description = description.trim().to_string();
            if description.chars().count() > 4000 {
                return Err(AppError::Validation(
                    "project description must not exceed 4000 characters".into(),
                ));
            }
            project.description = description;
        }
        if input.clear_due_at {
            project.due_at = None;
        } else if let Some(due_at) = input.due_at {
            project.due_at = Some(due_at);
        }
        project.updated_by_agent_id = Some(input.actor_agent_id);
        project.updated_at = now_utc();
        self.repo
            .update_company_project_metadata(project.clone(), format!("项目 · {}", project.name))?;
        self.company_project_view(project)
    }

    pub fn transfer_company_project_owner(
        &self,
        input: TransferCompanyProjectOwnerInput,
    ) -> AppResult<CompanyProjectView> {
        self.ensure_agent_can_act(input.actor_agent_id)?;
        self.ensure_company_agent_permission(
            input.company_id,
            input.actor_agent_id,
            COMPANY_PERMISSION_PROJECT_MANAGE,
        )?;
        let project = self.ensure_company_project_access(
            input.company_id,
            input.project_id,
            input.actor_agent_id,
        )?;
        self.transfer_company_project_owner_record(
            project,
            input.owner_agent_id,
            Some(input.actor_agent_id),
            None,
        )
    }

    pub fn transfer_company_project_owner_for_human(
        &self,
        input: TransferCompanyProjectOwnerForHumanInput,
    ) -> AppResult<CompanyProjectView> {
        let project = self.ensure_company_project_for_human_manager(
            input.human_user_id,
            input.company_id,
            input.project_id,
        )?;
        self.transfer_company_project_owner_record(
            project,
            input.owner_agent_id,
            None,
            Some(input.human_user_id),
        )
    }

    fn transfer_company_project_owner_record(
        &self,
        mut project: CompanyProject,
        owner_agent_id: Uuid,
        actor_agent_id: Option<Uuid>,
        actor_human_user_id: Option<Uuid>,
    ) -> AppResult<CompanyProjectView> {
        self.ensure_project_not_paused(&project)?;
        self.ensure_agent_can_act(owner_agent_id)?;
        self.ensure_active_company_conversation_member(project.company_id, owner_agent_id)?;
        if owner_agent_id == project.owner_agent_id {
            return self.company_project_view(project);
        }

        let existing_member = self
            .repo
            .get_company_project_member(project.id, owner_agent_id);
        let is_active_member = existing_member
            .as_ref()
            .is_some_and(|member| member.left_at.is_none());
        if !is_active_member {
            let governance = self.effective_company_governance_policy_settings(project.company_id);
            let active_member_count = self
                .repo
                .list_company_project_members(project.id)
                .into_iter()
                .filter(|member| member.left_at.is_none())
                .count();
            if active_member_count >= governance.max_project_members as usize {
                return Err(AppError::Validation(format!(
                    "project supports at most {} active members",
                    governance.max_project_members
                )));
            }
        }

        let previous_owner_agent_id = project.owner_agent_id;
        let now = now_utc();
        let new_owner_member = CompanyProjectMember {
            id: existing_member
                .as_ref()
                .map(|member| member.id)
                .unwrap_or_else(Uuid::new_v4),
            project_id: project.id,
            agent_profile_id: owner_agent_id,
            role: PROJECT_MEMBER_ROLE_OWNER.into(),
            joined_at: existing_member
                .as_ref()
                .filter(|member| member.left_at.is_none())
                .map(|member| member.joined_at)
                .unwrap_or(now),
            left_at: None,
            added_by_agent_id: existing_member
                .as_ref()
                .map(|member| member.added_by_agent_id)
                .or(actor_agent_id)
                .unwrap_or(previous_owner_agent_id),
        };
        project.owner_agent_id = owner_agent_id;
        project.updated_by_agent_id = actor_agent_id;
        project.updated_at = now;
        self.repo
            .complete_company_project_owner_transfer(CompanyProjectOwnerTransferBundle {
                project: project.clone(),
                previous_owner_agent_id,
                new_owner_member,
            })?;

        let transfer_payload = json!({
            "company_id": project.company_id,
            "project_id": project.id,
            "project_name": project.name,
            "previous_owner_agent_id": previous_owner_agent_id,
            "owner_agent_id": owner_agent_id,
            "transferred_by_agent_id": actor_agent_id,
            "transferred_by_human_user_id": actor_human_user_id,
        });
        let _ = self.enqueue_agent_event(
            previous_owner_agent_id,
            "company.project.owner_transferred",
            transfer_payload.clone(),
            50,
        );
        let _ = self.enqueue_agent_event(
            owner_agent_id,
            "company.project.owner_assigned",
            transfer_payload,
            55,
        );
        self.company_project_view(project)
    }

    pub fn pause_company_project_for_human(
        &self,
        input: SetCompanyProjectPauseForHumanInput,
    ) -> AppResult<CompanyProjectView> {
        let mut project = self.ensure_company_project_for_human_manager(
            input.human_user_id,
            input.company_id,
            input.project_id,
        )?;
        if project.status == PROJECT_STATUS_PAUSED {
            return self.company_project_view(project);
        }
        if matches!(
            project.status.as_str(),
            PROJECT_STATUS_COMPLETED | PROJECT_STATUS_CANCELLED
        ) {
            return Err(AppError::Conflict(
                "completed or cancelled projects cannot be paused".into(),
            ));
        }
        project.status = PROJECT_STATUS_PAUSED.into();
        project.updated_at = now_utc();
        self.repo.update_company_project(project.clone())?;
        self.company_project_view(project)
    }

    pub fn resume_company_project_for_human(
        &self,
        input: SetCompanyProjectPauseForHumanInput,
    ) -> AppResult<CompanyProjectView> {
        let mut project = self.ensure_company_project_for_human_manager(
            input.human_user_id,
            input.company_id,
            input.project_id,
        )?;
        if project.status != PROJECT_STATUS_PAUSED {
            return Err(AppError::Conflict("project is not paused".into()));
        }
        let now = now_utc();
        project.status = PROJECT_STATUS_ACTIVE.into();
        project.updated_at = now;
        self.repo.update_company_project(project.clone())?;
        for member in self.repo.list_company_project_members(project.id) {
            if member.left_at.is_none() {
                let _ = self.repo.request_agent_codex_trigger_wake(
                    member.agent_profile_id,
                    now,
                    AGENT_CODEX_WAKE_REASON_PROJECT_RESUMED,
                );
            }
        }
        self.company_project_view(project)
    }

    pub fn is_company_project_paused(&self, project_id: Uuid) -> AppResult<bool> {
        Ok(self
            .repo
            .get_company_project_result(project_id)?
            .is_some_and(|project| project.status == PROJECT_STATUS_PAUSED))
    }

    pub fn get_company_project_git_for_human(
        &self,
        input: GetCompanyProjectGitForHumanInput,
    ) -> AppResult<Option<CompanyProjectGitAdminView>> {
        self.ensure_company_project_for_human_manager(
            input.human_user_id,
            input.company_id,
            input.project_id,
        )?;
        Ok(self
            .repo
            .get_company_project_git_config(input.project_id)
            .map(company_project_git_admin_view))
    }

    pub fn upsert_company_project_git(
        &self,
        input: UpsertCompanyProjectGitInput,
    ) -> AppResult<CompanyProjectGitAdminView> {
        self.ensure_agent_can_act(input.actor_agent_id)?;
        let membership =
            self.ensure_active_company_conversation_member(input.company_id, input.actor_agent_id)?;
        if !membership.permissions.iter().any(|permission| {
            matches!(
                permission.as_str(),
                COMPANY_PERMISSION_PROJECT_CREATE | COMPANY_PERMISSION_PROJECT_MANAGE
            )
        }) {
            return Err(AppError::Unauthorized(
                "agent cannot configure project Git".into(),
            ));
        }
        let project = self.ensure_company_project_access(
            input.company_id,
            input.project_id,
            input.actor_agent_id,
        )?;
        self.ensure_project_not_paused(&project)?;
        let existing = self.repo.get_company_project_git_config(input.project_id);
        let now = now_utc();
        let host_local_path = resolve_project_host_local_path(
            input.project_id,
            input.host_local_path,
            existing.as_ref(),
        )?;
        let config = build_project_git_config(
            input.project_id,
            input.remote_url,
            host_local_path,
            input.default_branch.or_else(|| {
                existing
                    .as_ref()
                    .map(|config| config.default_branch.clone())
            }),
            input.auth_profile,
            input.allow_agent_push.unwrap_or_else(|| {
                existing
                    .as_ref()
                    .is_some_and(|config| config.allow_agent_push)
            }),
            input
                .branch_prefix
                .or_else(|| existing.as_ref().map(|config| config.branch_prefix.clone())),
            existing
                .as_ref()
                .and_then(|config| config.created_by_agent_id)
                .or(Some(input.actor_agent_id)),
            existing
                .as_ref()
                .and_then(|config| config.created_by_human_user_id),
            Some(input.actor_agent_id),
            None,
            existing.as_ref().map(|config| config.created_at),
            now,
        )?;
        self.repo.save_company_project_git_config(config.clone())?;
        Ok(company_project_git_admin_view(config))
    }

    pub fn upsert_company_project_git_for_human(
        &self,
        input: UpsertCompanyProjectGitForHumanInput,
    ) -> AppResult<CompanyProjectGitAdminView> {
        let project = self.ensure_company_project_for_human_manager(
            input.human_user_id,
            input.company_id,
            input.project_id,
        )?;
        self.ensure_project_not_paused(&project)?;
        let existing = self.repo.get_company_project_git_config(input.project_id);
        let now = now_utc();
        let host_local_path = resolve_project_host_local_path(
            input.project_id,
            input.host_local_path,
            existing.as_ref(),
        )?;
        let config = build_project_git_config(
            input.project_id,
            input.remote_url,
            host_local_path,
            input.default_branch.or_else(|| {
                existing
                    .as_ref()
                    .map(|config| config.default_branch.clone())
            }),
            input.auth_profile,
            input.allow_agent_push.unwrap_or_else(|| {
                existing
                    .as_ref()
                    .is_some_and(|config| config.allow_agent_push)
            }),
            input
                .branch_prefix
                .or_else(|| existing.as_ref().map(|config| config.branch_prefix.clone())),
            existing
                .as_ref()
                .and_then(|config| config.created_by_agent_id),
            existing
                .as_ref()
                .and_then(|config| config.created_by_human_user_id)
                .or(Some(input.human_user_id)),
            None,
            Some(input.human_user_id),
            existing.as_ref().map(|config| config.created_at),
            now,
        )?;
        self.repo.save_company_project_git_config(config.clone())?;
        Ok(company_project_git_admin_view(config))
    }

    pub fn configure_managed_local_project_git_for_human(
        &self,
        input: ConfigureManagedLocalProjectGitForHumanInput,
    ) -> AppResult<CompanyProjectGitAdminView> {
        let project = self.ensure_company_project_for_human_manager(
            input.human_user_id,
            input.company_id,
            input.project_id,
        )?;
        self.ensure_project_not_paused(&project)?;
        let managed_local_path = validate_host_local_path(&input.managed_local_path)?;
        let encoded_path = url::Url::from_file_path(&managed_local_path).map_err(|_| {
            AppError::Validation("managed project path cannot be represented as a file URL".into())
        })?;
        let now = now_utc();
        let existing = self.repo.get_company_project_git_config(input.project_id);
        let config = CompanyProjectGitConfig {
            project_id: input.project_id,
            remote_url: encoded_path.to_string(),
            default_branch: "main".into(),
            git_host: "relay-local".into(),
            host_local_path: managed_local_path,
            auth_profile: None,
            allow_agent_push: true,
            branch_prefix: "relay/".into(),
            created_by_agent_id: existing
                .as_ref()
                .and_then(|config| config.created_by_agent_id),
            created_by_human_user_id: existing
                .as_ref()
                .and_then(|config| config.created_by_human_user_id)
                .or(Some(input.human_user_id)),
            updated_by_agent_id: None,
            updated_by_human_user_id: Some(input.human_user_id),
            created_at: existing
                .as_ref()
                .map(|config| config.created_at)
                .unwrap_or(now),
            updated_at: now,
        };
        self.repo.save_company_project_git_config(config.clone())?;
        Ok(company_project_git_admin_view(config))
    }

    pub fn delete_company_project_git_for_human(
        &self,
        input: DeleteCompanyProjectGitForHumanInput,
    ) -> AppResult<()> {
        let project = self.ensure_company_project_for_human_manager(
            input.human_user_id,
            input.company_id,
            input.project_id,
        )?;
        self.ensure_project_not_paused(&project)?;
        self.repo
            .delete_company_project_git_config(input.project_id)
    }

    pub fn update_company_project_rule(
        &self,
        input: UpdateCompanyProjectRuleInput,
    ) -> AppResult<CompanyProjectRule> {
        self.ensure_agent_can_act(input.actor_agent_id)?;
        self.ensure_company_agent_permission(
            input.company_id,
            input.actor_agent_id,
            COMPANY_PERMISSION_PROJECT_RULES_MANAGE,
        )?;
        let project = self.ensure_company_project_access(
            input.company_id,
            input.project_id,
            input.actor_agent_id,
        )?;
        self.ensure_project_not_paused(&project)?;
        let content = normalize_project_rule_content(input.content)?;
        let existing = self.repo.get_company_project_rule(input.project_id);
        let now = now_utc();
        let rule = CompanyProjectRule {
            project_id: input.project_id,
            content,
            updated_by_agent_id: Some(input.actor_agent_id),
            updated_by_human_user_id: None,
            created_at: existing.as_ref().map_or(now, |rule| rule.created_at),
            updated_at: now,
        };
        self.repo.save_company_project_rule(rule.clone())?;
        Ok(rule)
    }

    pub fn update_company_project_rule_for_human(
        &self,
        input: UpdateCompanyProjectRuleForHumanInput,
    ) -> AppResult<CompanyProjectRule> {
        let project = self.ensure_company_project_for_human_manager(
            input.human_user_id,
            input.company_id,
            input.project_id,
        )?;
        self.ensure_project_not_paused(&project)?;
        let content = normalize_project_rule_content(input.content)?;
        let existing = self.repo.get_company_project_rule(input.project_id);
        let now = now_utc();
        let rule = CompanyProjectRule {
            project_id: input.project_id,
            content,
            updated_by_agent_id: None,
            updated_by_human_user_id: Some(input.human_user_id),
            created_at: existing.as_ref().map_or(now, |rule| rule.created_at),
            updated_at: now,
        };
        self.repo.save_company_project_rule(rule.clone())?;
        Ok(rule)
    }

    pub fn request_company_project_rule_generation_for_human(
        &self,
        input: RequestCompanyProjectRuleGenerationForHumanInput,
    ) -> AppResult<()> {
        let project = self.ensure_company_project_for_human_manager(
            input.human_user_id,
            input.company_id,
            input.project_id,
        )?;
        self.ensure_project_not_paused(&project)?;
        self.ensure_company_agent_for_human_manager(
            input.human_user_id,
            input.company_id,
            input.agent_id,
        )?;
        self.ensure_active_project_member(input.project_id, input.agent_id)?;
        self.ensure_company_agent_permission(
            input.company_id,
            input.agent_id,
            COMPANY_PERMISSION_PROJECT_RULES_MANAGE,
        )?;
        let instructions = normalize_optional_text(input.instructions);
        if instructions
            .as_ref()
            .is_some_and(|value| value.chars().count() > 2_000)
        {
            return Err(AppError::Validation(
                "project rule generation instructions must not exceed 2000 characters".into(),
            ));
        }
        self.enqueue_agent_event(
            input.agent_id,
            "company.project.rule_generation_requested",
            json!({
                "company_id": input.company_id,
                "project_id": input.project_id,
                "project_name": project.name,
                "instructions": instructions,
                "requested_by_human_user_id": input.human_user_id,
            }),
            40,
        )
    }

    pub fn replace_company_project_assets(
        &self,
        input: ReplaceCompanyProjectAssetsInput,
    ) -> AppResult<Vec<CompanyProjectAsset>> {
        self.ensure_agent_can_act(input.actor_agent_id)?;
        self.ensure_company_agent_permission(
            input.company_id,
            input.actor_agent_id,
            COMPANY_PERMISSION_PROJECT_ASSETS_MANAGE,
        )?;
        let project = self.ensure_company_project_access(
            input.company_id,
            input.project_id,
            input.actor_agent_id,
        )?;
        self.ensure_project_not_paused(&project)?;
        if input.assets.len() > 500 {
            return Err(AppError::Validation(
                "a project asset inventory supports at most 500 entries".into(),
            ));
        }
        let now = now_utc();
        let mut locators = HashSet::new();
        let mut assets = Vec::with_capacity(input.assets.len());
        for asset in input.assets {
            let name = normalize_required_project_asset_text(asset.name, 200, "asset name")?;
            let asset_type =
                normalize_required_project_asset_text(asset.asset_type, 80, "asset type")?;
            let locator =
                normalize_required_project_asset_text(asset.locator, 4_096, "asset locator")?;
            if !locators.insert(locator.clone()) {
                return Err(AppError::Validation(format!(
                    "duplicate project asset locator: {locator}"
                )));
            }
            let description = asset.description.unwrap_or_default().trim().to_string();
            if description.chars().count() > 4_000 {
                return Err(AppError::Validation(
                    "asset description must not exceed 4000 characters".into(),
                ));
            }
            let status = asset.status.unwrap_or_else(|| "active".into());
            if !matches!(
                status.as_str(),
                "active" | "missing" | "deprecated" | "unknown"
            ) {
                return Err(AppError::Validation(
                    "asset status must be active, missing, deprecated, or unknown".into(),
                ));
            }
            let metadata = asset.metadata.unwrap_or_else(|| json!({}));
            if !metadata.is_object() {
                return Err(AppError::Validation(
                    "asset metadata must be a JSON object".into(),
                ));
            }
            assets.push(CompanyProjectAsset {
                id: Uuid::new_v4(),
                project_id: input.project_id,
                name,
                asset_type,
                locator,
                description,
                status,
                metadata,
                updated_by_agent_id: Some(input.actor_agent_id),
                updated_by_human_user_id: None,
                created_at: now,
                updated_at: now,
            });
        }
        assets.sort_by(|left, right| {
            left.asset_type
                .cmp(&right.asset_type)
                .then_with(|| left.name.cmp(&right.name))
        });
        self.repo
            .replace_company_project_assets(input.project_id, assets.clone())?;
        Ok(assets)
    }
}
