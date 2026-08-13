use super::*;
use std::collections::BTreeMap;

macro_rules! string_enum {
    ($name:ident { $($variant:ident => $value:literal),+ $(,)? }) => {
        #[derive(Debug, Deserialize, JsonSchema)]
        #[serde(rename_all = "snake_case")]
        pub(super) enum $name {
            $($variant),+
        }

        impl $name {
            pub(super) const fn as_str(&self) -> &'static str {
                match self {
                    $(Self::$variant => $value),+
                }
            }

            pub(super) fn into_string(self) -> String {
                self.as_str().to_owned()
            }
        }
    };
}

string_enum!(AttemptTypeInput {
    Execution => "execution",
    Review => "review",
    Qa => "qa",
    Retest => "retest",
    EnvironmentCheck => "environment_check",
});

string_enum!(AttemptTerminalStatusInput {
    Succeeded => "succeeded",
    Failed => "failed",
    Cancelled => "cancelled",
    Interrupted => "interrupted",
});

string_enum!(BlockerTypeInput {
    Dependency => "dependency",
    Environment => "environment",
    Approval => "approval",
    Defect => "defect",
    Decision => "decision",
    External => "external",
});

string_enum!(BlockerResolutionStatusInput {
    Resolved => "resolved",
    Waived => "waived",
});

string_enum!(TaskRelationTypeInput {
    Parent => "parent",
    Child => "child",
    RetryOf => "retry_of",
    Supersedes => "supersedes",
    CausedBy => "caused_by",
    Validates => "validates",
    Fixes => "fixes",
});

string_enum!(EvidenceTypeInput {
    Test => "test",
    Report => "report",
    Screenshot => "screenshot",
    Log => "log",
    Runtime => "runtime",
    Design => "design",
    Decision => "decision",
    Other => "other",
});

string_enum!(EvidenceResultInput {
    Passed => "passed",
    Failed => "failed",
    Inconclusive => "inconclusive",
    Informational => "informational",
});

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
        #[schemars(
            description = "Execution attempt category. Use execution for implementation/work, review for content or code review, qa for initial verification, retest after a fix, and environment_check for environment diagnostics."
        )]
        attempt_type: AttemptTypeInput,
        objective: String,
    },
    AttemptFinish {
        company_id: Uuid,
        project_id: Uuid,
        task_id: Uuid,
        attempt_id: Uuid,
        #[schemars(
            description = "Terminal Attempt status, not the Task status. Use succeeded when acceptance criteria pass; failed when work cannot be delivered; cancelled when intentionally abandoned; interrupted when execution stopped before a result. Never use done, completed, success, or blocked."
        )]
        status: AttemptTerminalStatusInput,
        result_summary: String,
        failure_category: Option<String>,
    },
    BlockerOpen {
        company_id: Uuid,
        project_id: Uuid,
        task_id: Uuid,
        attempt_id: Option<Uuid>,
        #[schemars(
            description = "Blocker category. Put detailed subtypes such as environment_git_permission in summary or resolution_condition, while blocker_type remains environment."
        )]
        blocker_type: BlockerTypeInput,
        summary: String,
        owner_agent_id: Option<Uuid>,
        resolution_condition: String,
    },
    BlockerResolve {
        company_id: Uuid,
        project_id: Uuid,
        task_id: Uuid,
        blocker_id: Uuid,
        #[schemars(description = "How the open blocker was closed: resolved or waived.")]
        status: BlockerResolutionStatusInput,
        resolution_summary: String,
    },
    RelationAdd {
        company_id: Uuid,
        project_id: Uuid,
        source_task_id: Uuid,
        target_task_id: Uuid,
        relation_type: TaskRelationTypeInput,
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
        #[schemars(
            description = "Evidence category. Use report for review documents and structured written findings; use other only when no specific category applies."
        )]
        evidence_type: EvidenceTypeInput,
        title: String,
        summary: String,
        #[schemars(
            description = "Evidence conclusion: passed, failed, inconclusive, or informational. Put delivery/blocker details in summary and metrics instead of inventing a result value."
        )]
        result: EvidenceResultInput,
        #[serde(default)]
        artifact_refs: Vec<Value>,
        #[serde(default)]
        #[schemars(
            description = "Optional structured metrics object. Omit it or send {} when there are no metrics."
        )]
        metrics: BTreeMap<String, Value>,
        dedupe_key: Option<String>,
    },
}
