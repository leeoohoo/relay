use super::*;

#[derive(Debug, Deserialize, JsonSchema)]
pub(super) struct CompanyTaskToolInput {
    #[serde(flatten)]
    pub(super) operation: CompanyTaskOperation,
    #[schemars(description = "Optional retry key for mutating task actions.")]
    pub(super) idempotency_key: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(tag = "action", rename_all = "snake_case")]
pub(super) enum CompanyTaskOperation {
    Get {
        company_id: Uuid,
        project_id: Uuid,
        task_id: Uuid,
    },
    List {
        company_id: Uuid,
        project_id: Uuid,
        assignee_agent_id: Option<Uuid>,
        status: Option<String>,
    },
    My {
        company_id: Uuid,
        status: Option<String>,
    },
    Create {
        company_id: Uuid,
        project_id: Uuid,
        title: String,
        description: Option<String>,
        priority: Option<String>,
        assignee_agent_id: Option<Uuid>,
        due_at: Option<DateTime<Utc>>,
    },
    Update {
        company_id: Uuid,
        project_id: Uuid,
        task_id: Uuid,
        title: Option<String>,
        description: Option<String>,
        status: Option<String>,
        priority: Option<String>,
        assignee_agent_id: Option<Uuid>,
        due_at: Option<DateTime<Utc>>,
    },
    BatchUpdate {
        company_id: Uuid,
        project_id: Uuid,
        task_ids: Vec<Uuid>,
        status: Option<String>,
        priority: Option<String>,
        assignee_agent_id: Option<Uuid>,
        #[serde(default)]
        clear_assignee: bool,
        due_at: Option<DateTime<Utc>>,
        #[serde(default)]
        clear_due_at: bool,
    },
    DependencyAdd {
        company_id: Uuid,
        project_id: Uuid,
        task_id: Uuid,
        depends_on_task_id: Uuid,
        #[schemars(
            description = "Dependency condition: success (default), completion, or failure."
        )]
        dependency_condition: Option<String>,
    },
    DependencyRemove {
        company_id: Uuid,
        project_id: Uuid,
        task_id: Uuid,
        depends_on_task_id: Uuid,
    },
    ExecutionGet {
        company_id: Uuid,
        project_id: Uuid,
        task_id: Uuid,
    },
    AttemptStart {
        company_id: Uuid,
        project_id: Uuid,
        task_id: Uuid,
        intent_id: Option<Uuid>,
        attempt_type: String,
        objective: String,
    },
    AttemptFinish {
        company_id: Uuid,
        project_id: Uuid,
        task_id: Uuid,
        attempt_id: Uuid,
        status: String,
        result_summary: String,
        failure_category: Option<String>,
    },
    BlockerOpen {
        company_id: Uuid,
        project_id: Uuid,
        task_id: Uuid,
        attempt_id: Option<Uuid>,
        blocker_type: String,
        summary: String,
        owner_agent_id: Option<Uuid>,
        resolution_condition: String,
    },
    BlockerResolve {
        company_id: Uuid,
        project_id: Uuid,
        task_id: Uuid,
        blocker_id: Uuid,
        status: String,
        resolution_summary: String,
    },
    RelationAdd {
        company_id: Uuid,
        project_id: Uuid,
        source_task_id: Uuid,
        target_task_id: Uuid,
        relation_type: String,
    },
    RelationRemove {
        company_id: Uuid,
        project_id: Uuid,
        relation_id: Uuid,
    },
    EvidenceCreate {
        company_id: Uuid,
        project_id: Uuid,
        task_id: Option<Uuid>,
        attempt_id: Option<Uuid>,
        gate_id: Option<Uuid>,
        environment_id: Option<Uuid>,
        evidence_type: String,
        title: String,
        summary: String,
        result: String,
        #[serde(default)]
        artifact_refs: Vec<Value>,
        #[serde(default)]
        metrics: Value,
        dedupe_key: Option<String>,
    },
}
