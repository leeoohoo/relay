use super::handler::parse_input;
use super::*;
use ai_chat_application::{
    AddProjectTaskRelationInput, CreateProjectEvidenceInput, FinishProjectTaskAttemptInput,
    OpenProjectTaskBlockerInput, RemoveProjectTaskRelationInput, ResolveProjectTaskBlockerInput,
    StartProjectTaskAttemptInput,
};

impl<R: PlatformRepository, V: OwnershipProofVerifier> McpGateway<R, V> {
    pub(super) fn execute_company_task_tool(
        &self,
        agent_id: Uuid,
        tool_name: &str,
        input: Value,
        _idempotency_key: Option<String>,
    ) -> AppResult<Value> {
        match tool_name {
            "company.task" => {
                let input: CompanyTaskToolInput = parse_input(input)?;
                let _ = input.idempotency_key;
                match input.operation {
                    CompanyTaskOperation::Get {
                        company_id,
                        project_id,
                        task_id,
                    } => {
                        let project =
                            self.platform.get_company_project(GetCompanyProjectInput {
                                actor_agent_id: agent_id,
                                company_id,
                                project_id,
                            })?;
                        let task = project
                            .tasks
                            .iter()
                            .find(|task| task.id == task_id)
                            .cloned()
                            .ok_or_else(|| AppError::NotFound("project task not found".into()))?;
                        let dependencies = project
                            .task_dependencies
                            .into_iter()
                            .filter(|dependency| {
                                dependency.task_id == task_id
                                    || dependency.depends_on_task_id == task_id
                            })
                            .collect::<Vec<_>>();
                        let status_history = project
                            .task_status_history
                            .into_iter()
                            .filter(|entry| entry.task_id == task_id)
                            .collect::<Vec<_>>();
                        Ok(json!({
                            "task": task,
                            "project": project.project,
                            "dependencies": dependencies,
                            "status_history": status_history,
                        }))
                    }
                    CompanyTaskOperation::List {
                        company_id,
                        project_id,
                        assignee_agent_id,
                        status,
                    } => {
                        let project =
                            self.platform.get_company_project(GetCompanyProjectInput {
                                actor_agent_id: agent_id,
                                company_id,
                                project_id,
                            })?;
                        let tasks = project
                            .tasks
                            .into_iter()
                            .filter(|task| {
                                assignee_agent_id
                                    .is_none_or(|assignee| task.assignee_agent_id == Some(assignee))
                                    && status
                                        .as_deref()
                                        .is_none_or(|expected| task.status == expected)
                            })
                            .collect::<Vec<_>>();
                        Ok(json!({
                            "project": project.project,
                            "tasks": tasks,
                            "task_dependencies": project.task_dependencies,
                        }))
                    }
                    CompanyTaskOperation::My { company_id, status } => {
                        let mut assignments = Vec::new();
                        let mut ready_count = 0usize;
                        let mut waiting_count = 0usize;
                        for project in self.platform.list_company_projects(agent_id, company_id)? {
                            for task in project.tasks.iter().filter(|task| {
                                task.assignee_agent_id == Some(agent_id)
                                    && status
                                        .as_deref()
                                        .is_none_or(|expected| task.status == expected)
                            }) {
                                let dependencies = project
                                    .task_dependencies
                                    .iter()
                                    .filter(|dependency| dependency.task_id == task.id)
                                    .filter_map(|dependency| {
                                        project
                                            .tasks
                                            .iter()
                                            .find(|candidate| {
                                                candidate.id == dependency.depends_on_task_id
                                            })
                                            .map(|dependency_task| {
                                                let resolved = ai_chat_domain::company::project_task_dependency_satisfied(
                                                    &dependency.dependency_condition,
                                                    &dependency_task.status,
                                                );
                                                json!({
                                                    "task_id": dependency_task.id,
                                                    "title": dependency_task.title,
                                                    "status": dependency_task.status,
                                                    "assignee_agent_id": dependency_task.assignee_agent_id,
                                                    "condition": dependency.dependency_condition,
                                                    "resolved": resolved,
                                                })
                                            })
                                    })
                                    .collect::<Vec<_>>();
                                let unresolved_dependencies = dependencies
                                    .iter()
                                    .filter(|dependency| {
                                        dependency.get("resolved").and_then(|value| value.as_bool())
                                            != Some(true)
                                    })
                                    .cloned()
                                    .collect::<Vec<_>>();
                                let can_start = unresolved_dependencies.is_empty();
                                if can_start {
                                    ready_count += 1;
                                } else {
                                    waiting_count += 1;
                                }
                                assignments.push(json!({
                                    "project_id": project.project.id,
                                    "project_name": project.project.name,
                                    "project_status": project.project.status,
                                    "task": task,
                                    "readiness": if can_start { "ready" } else { "waiting_for_dependencies" },
                                    "can_start": can_start,
                                    "dependencies": dependencies,
                                    "unresolved_dependencies": unresolved_dependencies,
                                    "guidance": if can_start {
                                        "任务前置已满足，可以按职责开始处理。"
                                    } else {
                                        "前置任务尚未完成：本轮保持任务原状态，不发送等待占位消息，结束后由下一次定时检查重新判断。"
                                    },
                                }));
                            }
                        }
                        Ok(json!({
                            "assignments": assignments,
                            "ready_count": ready_count,
                            "waiting_count": waiting_count,
                        }))
                    }
                    CompanyTaskOperation::Create {
                        company_id,
                        project_id,
                        title,
                        description,
                        priority,
                        assignee_agent_id,
                        due_at,
                    } => {
                        let task = self.platform.create_company_project_task(
                            CreateCompanyProjectTaskInput {
                                actor_agent_id: agent_id,
                                company_id,
                                project_id,
                                title,
                                description,
                                priority,
                                assignee_agent_id,
                                due_at,
                            },
                        )?;
                        Ok(json!({ "task": task }))
                    }
                    CompanyTaskOperation::Update {
                        company_id,
                        project_id,
                        task_id,
                        title,
                        description,
                        status,
                        priority,
                        assignee_agent_id,
                        due_at,
                    } => {
                        let task = self.platform.update_company_project_task(
                            UpdateCompanyProjectTaskInput {
                                actor_agent_id: agent_id,
                                company_id,
                                project_id,
                                task_id,
                                title,
                                description,
                                status,
                                priority,
                                assignee_agent_id,
                                due_at,
                            },
                        )?;
                        Ok(json!({ "task": task }))
                    }
                    CompanyTaskOperation::BatchUpdate {
                        company_id,
                        project_id,
                        task_ids,
                        status,
                        priority,
                        assignee_agent_id,
                        clear_assignee,
                        due_at,
                        clear_due_at,
                    } => {
                        let tasks = self.platform.batch_update_company_project_tasks(
                            BatchUpdateCompanyProjectTasksInput {
                                actor_agent_id: agent_id,
                                company_id,
                                project_id,
                                task_ids,
                                status,
                                priority,
                                assignee_agent_id,
                                clear_assignee,
                                due_at,
                                clear_due_at,
                            },
                        )?;
                        Ok(json!({ "tasks": tasks }))
                    }
                    CompanyTaskOperation::DependencyAdd {
                        company_id,
                        project_id,
                        task_id,
                        depends_on_task_id,
                        dependency_condition,
                    } => {
                        let dependency = self.platform.add_company_project_task_dependency(
                            ChangeCompanyProjectTaskDependencyInput {
                                actor_agent_id: agent_id,
                                company_id,
                                project_id,
                                task_id,
                                depends_on_task_id,
                                dependency_condition,
                            },
                        )?;
                        Ok(json!({ "dependency": dependency }))
                    }
                    CompanyTaskOperation::DependencyRemove {
                        company_id,
                        project_id,
                        task_id,
                        depends_on_task_id,
                    } => {
                        self.platform.remove_company_project_task_dependency(
                            ChangeCompanyProjectTaskDependencyInput {
                                actor_agent_id: agent_id,
                                company_id,
                                project_id,
                                task_id,
                                depends_on_task_id,
                                dependency_condition: None,
                            },
                        )?;
                        Ok(json!({
                            "project_id": project_id,
                            "task_id": task_id,
                            "depends_on_task_id": depends_on_task_id,
                            "removed": true
                        }))
                    }
                    CompanyTaskOperation::ExecutionGet {
                        company_id,
                        project_id,
                        task_id,
                    } => Ok(
                        json!({ "execution": self.platform.get_project_task_execution(agent_id, company_id, project_id, task_id)? }),
                    ),
                    CompanyTaskOperation::AttemptStart {
                        company_id,
                        project_id,
                        task_id,
                        intent_id,
                        attempt_type,
                        objective,
                    } => Ok(
                        json!({ "attempt": self.platform.start_project_task_attempt(StartProjectTaskAttemptInput { actor_agent_id: agent_id, company_id, project_id, task_id, intent_id, attempt_type: attempt_type.into_string(), objective })? }),
                    ),
                    CompanyTaskOperation::AttemptFinish {
                        company_id,
                        project_id,
                        task_id,
                        attempt_id,
                        status,
                        result_summary,
                        failure_category,
                    } => Ok(
                        json!({ "attempt": self.platform.finish_project_task_attempt(FinishProjectTaskAttemptInput { actor_agent_id: agent_id, company_id, project_id, task_id, attempt_id, status: status.into_string(), result_summary, failure_category })? }),
                    ),
                    CompanyTaskOperation::BlockerOpen {
                        company_id,
                        project_id,
                        task_id,
                        attempt_id,
                        blocker_type,
                        summary,
                        owner_agent_id,
                        resolution_condition,
                    } => Ok(
                        json!({ "blocker": self.platform.open_project_task_blocker(OpenProjectTaskBlockerInput { actor_agent_id: agent_id, company_id, project_id, task_id, attempt_id, blocker_type: blocker_type.into_string(), summary, owner_agent_id, resolution_condition })? }),
                    ),
                    CompanyTaskOperation::BlockerResolve {
                        company_id,
                        project_id,
                        task_id,
                        blocker_id,
                        status,
                        resolution_summary,
                    } => Ok(
                        json!({ "blocker": self.platform.resolve_project_task_blocker(ResolveProjectTaskBlockerInput { actor_agent_id: agent_id, company_id, project_id, task_id, blocker_id, status: status.into_string(), resolution_summary })? }),
                    ),
                    CompanyTaskOperation::RelationAdd {
                        company_id,
                        project_id,
                        source_task_id,
                        target_task_id,
                        relation_type,
                    } => Ok(
                        json!({ "relation": self.platform.add_project_task_relation(AddProjectTaskRelationInput { actor_agent_id: agent_id, company_id, project_id, source_task_id, target_task_id, relation_type: relation_type.into_string() })? }),
                    ),
                    CompanyTaskOperation::RelationRemove {
                        company_id,
                        project_id,
                        relation_id,
                    } => {
                        self.platform.remove_project_task_relation(
                            RemoveProjectTaskRelationInput {
                                actor_agent_id: agent_id,
                                company_id,
                                project_id,
                                relation_id,
                            },
                        )?;
                        Ok(json!({ "relation_id": relation_id, "removed": true }))
                    }
                    CompanyTaskOperation::EvidenceCreate {
                        company_id,
                        project_id,
                        task_id,
                        attempt_id,
                        gate_id,
                        environment_id,
                        evidence_type,
                        title,
                        summary,
                        result,
                        artifact_refs,
                        metrics,
                        dedupe_key,
                    } => Ok(
                        json!({ "evidence": self.platform.create_project_evidence(CreateProjectEvidenceInput { actor_agent_id: agent_id, company_id, project_id, task_id, attempt_id, gate_id, environment_id, evidence_type: evidence_type.into_string(), title, summary, result: result.into_string(), artifact_refs, metrics: Value::Object(metrics.into_iter().collect()), dedupe_key })? }),
                    ),
                }
            }

            _ => Err(AppError::NotFound(format!(
                "unknown standard MCP tool: {tool_name}"
            ))),
        }
    }
}
