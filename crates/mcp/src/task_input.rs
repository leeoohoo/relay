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
    Artifact => "artifact",
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
            description = "Required. Execution attempt category: execution, review, qa, retest, or environment_check. Natural aliases such as implementation/work, verification/test, and delivery_recovery are accepted and normalized."
        )]
        attempt_type: AttemptTypeInput,
        #[schemars(description = "Required. Concise objective for this concrete attempt.")]
        objective: String,
    },
    AttemptFinish {
        company_id: Uuid,
        project_id: Uuid,
        task_id: Uuid,
        attempt_id: Uuid,
        #[schemars(
            description = "Required terminal Attempt status: succeeded, failed, cancelled, or interrupted. Natural aliases such as success/done/completed and blocked/stopped are accepted and normalized."
        )]
        status: AttemptTerminalStatusInput,
        #[schemars(
            description = "Required. What this attempt produced, verified, or failed to complete."
        )]
        result_summary: String,
        failure_category: Option<String>,
    },
    BlockerOpen {
        company_id: Uuid,
        project_id: Uuid,
        task_id: Uuid,
        attempt_id: Option<Uuid>,
        #[schemars(
            description = "Required blocker category: dependency, environment, approval, defect, decision, or external. Detailed values such as environment_git_permission and gate are accepted and normalized to the matching category."
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
            description = "Required evidence category: test, report, artifact, screenshot, log, runtime, design, decision, or other. Natural aliases such as document, integration_report, review, and git_commit are accepted and normalized."
        )]
        evidence_type: EvidenceTypeInput,
        #[schemars(
            description = "Required short evidence title. When omitted by an older cached client, Relay derives it from summary."
        )]
        title: String,
        #[schemars(
            description = "Required evidence summary describing what was checked or delivered and the conclusion."
        )]
        summary: String,
        #[schemars(
            description = "Required evidence conclusion: passed, failed, inconclusive, or informational. Natural aliases such as pass/success and blocked/pending are accepted and normalized. When omitted by an older cached client, Relay uses informational."
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

pub(super) fn normalize_company_task_input(input: &mut Value) {
    let Some(fields) = input.as_object_mut() else {
        return;
    };
    let Some(action) = fields
        .get("action")
        .and_then(Value::as_str)
        .map(str::to_owned)
    else {
        return;
    };

    match action.as_str() {
        "attempt_start" => {
            normalize_string_field(fields, "attempt_type", normalize_attempt_type);
            fields
                .entry("attempt_type")
                .or_insert_with(|| Value::String("execution".into()));
            fields.entry("objective").or_insert_with(|| {
                Value::String(
                    "Execute the assigned task and satisfy its acceptance criteria.".into(),
                )
            });
        }
        "attempt_finish" => {
            normalize_string_field(fields, "status", normalize_attempt_terminal_status);
        }
        "blocker_open" => {
            normalize_string_field(fields, "blocker_type", normalize_blocker_type);
        }
        "evidence_create" => normalize_evidence_input(fields),
        _ => {}
    }
}

pub(super) fn validate_company_task_input(input: &Value) -> AppResult<()> {
    let Some(fields) = input.as_object() else {
        return Err(AppError::Validation(
            "company.task input must be a JSON object".into(),
        ));
    };
    let Some(action) = fields.get("action").and_then(Value::as_str) else {
        return Err(AppError::Validation(
            "company.task requires action; inspect the tool schema and send one complete action request"
                .into(),
        ));
    };

    let contract = match action {
        "get" | "execution_get" => Some((
            &["company_id", "project_id", "task_id"][..],
            &[][..],
        )),
        "list" => Some((&["company_id", "project_id"][..], &[][..])),
        "my" => Some((&["company_id"][..], &[][..])),
        "create" => Some((&["company_id", "project_id", "title"][..], &[][..])),
        "update" => Some((
            &["company_id", "project_id", "task_id"][..],
            &[][..],
        )),
        "batch_update" => Some((
            &["company_id", "project_id", "task_ids"][..],
            &[][..],
        )),
        "dependency_add" | "dependency_remove" => Some((
            &[
                "company_id",
                "project_id",
                "task_id",
                "depends_on_task_id",
            ][..],
            &[][..],
        )),
        "attempt_start" => Some((
            &[
                "company_id",
                "project_id",
                "task_id",
                "attempt_type",
                "objective",
            ][..],
            &[(
                "attempt_type",
                &["execution", "review", "qa", "retest", "environment_check"][..],
            )][..],
        )),
        "attempt_finish" => Some((
            &[
                "company_id",
                "project_id",
                "task_id",
                "attempt_id",
                "status",
                "result_summary",
            ][..],
            &[(
                "status",
                &["succeeded", "failed", "cancelled", "interrupted"][..],
            )][..],
        )),
        "blocker_open" => Some((
            &[
                "company_id",
                "project_id",
                "task_id",
                "blocker_type",
                "summary",
                "resolution_condition",
            ][..],
            &[(
                "blocker_type",
                &[
                    "dependency",
                    "environment",
                    "approval",
                    "defect",
                    "decision",
                    "external",
                ][..],
            )][..],
        )),
        "blocker_resolve" => Some((
            &[
                "company_id",
                "project_id",
                "task_id",
                "blocker_id",
                "status",
                "resolution_summary",
            ][..],
            &[("status", &["resolved", "waived"][..])][..],
        )),
        "relation_add" => Some((
            &[
                "company_id",
                "project_id",
                "source_task_id",
                "target_task_id",
                "relation_type",
            ][..],
            &[(
                "relation_type",
                &[
                    "parent",
                    "child",
                    "retry_of",
                    "supersedes",
                    "caused_by",
                    "validates",
                    "fixes",
                ][..],
            )][..],
        )),
        "relation_remove" => Some((
            &["company_id", "project_id", "relation_id"][..],
            &[][..],
        )),
        "evidence_create" => Some((
            &[
                "company_id",
                "project_id",
                "evidence_type",
                "title",
                "summary",
                "result",
            ][..],
            &[
                (
                    "evidence_type",
                    &[
                        "test",
                        "report",
                        "artifact",
                        "screenshot",
                        "log",
                        "runtime",
                        "design",
                        "decision",
                        "other",
                    ][..],
                ),
                (
                    "result",
                    &["passed", "failed", "inconclusive", "informational"][..],
                ),
            ][..],
        )),
        _ => {
            return Err(AppError::Validation(format!(
                "unknown company.task action {action:?}; allowed actions: get, list, my, create, update, batch_update, dependency_add, dependency_remove, execution_get, attempt_start, attempt_finish, blocker_open, blocker_resolve, relation_add, relation_remove, evidence_create"
            )))
        }
    };
    let Some((required, enums)) = contract else {
        return Ok(());
    };

    let missing = required
        .iter()
        .filter(|field| {
            fields.get(**field).is_none_or(|value| {
                value.is_null()
                    || value.as_str().is_some_and(|value| value.trim().is_empty())
                    || value.as_array().is_some_and(Vec::is_empty)
            })
        })
        .copied()
        .collect::<Vec<_>>();
    let invalid = enums
        .iter()
        .filter_map(|(field, allowed)| {
            fields
                .get(*field)
                .and_then(Value::as_str)
                .filter(|value| !allowed.contains(value))
                .map(|value| format!("{field}={value:?}; allowed: {}", allowed.join(", ")))
        })
        .collect::<Vec<_>>();

    if missing.is_empty() && invalid.is_empty() {
        return Ok(());
    }

    let mut problems = Vec::new();
    if !missing.is_empty() {
        problems.push(format!("missing or empty fields: {}", missing.join(", ")));
    }
    if !invalid.is_empty() {
        problems.push(format!("invalid enum fields: {}", invalid.join("; ")));
    }
    Err(AppError::Validation(format!(
        "company.task action {action:?} is incomplete: {}. Required fields: {}. Rebuild one complete request and retry once; do not add fields one at a time",
        problems.join("; "),
        required.join(", ")
    )))
}

fn normalize_evidence_input(fields: &mut serde_json::Map<String, Value>) {
    normalize_string_field(fields, "evidence_type", normalize_evidence_type);
    normalize_string_field(fields, "result", normalize_evidence_result);

    fields
        .entry("evidence_type")
        .or_insert_with(|| Value::String("other".into()));
    fields
        .entry("result")
        .or_insert_with(|| Value::String("informational".into()));

    if !fields.contains_key("title") {
        let title = fields
            .get("summary")
            .and_then(Value::as_str)
            .map(evidence_title_from_summary)
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "Project evidence".into());
        fields.insert("title".into(), Value::String(title));
    }
    let metrics = fields
        .get("metrics")
        .and_then(normalize_object_value)
        .or_else(|| fields.get("metadata").and_then(normalize_object_value))
        .unwrap_or_else(|| Value::Object(serde_json::Map::new()));
    fields.insert("metrics".into(), metrics);
    fields.remove("metadata");

    if fields
        .get("artifact_refs")
        .is_none_or(|value| !value.is_array())
    {
        if let Some(locator) = fields.get("locator").and_then(Value::as_str) {
            fields.insert(
                "artifact_refs".into(),
                Value::Array(vec![Value::String(locator.to_owned())]),
            );
        } else {
            fields.insert("artifact_refs".into(), Value::Array(Vec::new()));
        }
    }
    fields.remove("locator");
}

fn normalize_object_value(value: &Value) -> Option<Value> {
    match value {
        Value::Object(_) => Some(value.clone()),
        Value::String(encoded) => serde_json::from_str::<Value>(encoded)
            .ok()
            .filter(Value::is_object),
        _ => None,
    }
}

fn normalize_string_field(
    fields: &mut serde_json::Map<String, Value>,
    field: &str,
    normalize: fn(&str) -> String,
) {
    let Some(value) = fields.get(field).and_then(Value::as_str) else {
        return;
    };
    fields.insert(field.into(), Value::String(normalize(value)));
}

fn normalized_key(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace([' ', '-'], "_")
}

fn normalize_attempt_type(value: &str) -> String {
    let value = normalized_key(value);
    match value.as_str() {
        "execution" | "review" | "qa" | "retest" | "environment_check" => value,
        "implementation" | "work" | "agent" | "delivery_recovery" => "execution".into(),
        "continuity_review" => "review".into(),
        "verification" | "validation" | "test" | "testing" => "qa".into(),
        "regression" | "regression_test" => "retest".into(),
        "environment" | "environment_diagnostic" | "diagnostic" => "environment_check".into(),
        value => value.into(),
    }
}

fn normalize_attempt_terminal_status(value: &str) -> String {
    match normalized_key(value).as_str() {
        "success" | "successful" | "done" | "complete" | "completed" | "passed" => {
            "succeeded".into()
        }
        "blocked" | "stopped" | "timeout" | "timed_out" | "aborted" => "interrupted".into(),
        "error" | "errored" | "unsuccessful" => "failed".into(),
        value => value.into(),
    }
}

fn normalize_blocker_type(value: &str) -> String {
    let value = normalized_key(value);
    match value.as_str() {
        "dependency" | "environment" | "approval" | "defect" | "decision" | "external" => value,
        "environment_git_permission" | "git" | "git_permission" | "repository_permission" => {
            "environment".into()
        }
        "gate" | "permission" | "approval_gate" => "approval".into(),
        "bug" | "failure" | "test_failure" => "defect".into(),
        "dependency_blocked" | "upstream_dependency" => "dependency".into(),
        value => value.into(),
    }
}

fn normalize_evidence_type(value: &str) -> String {
    let value = normalized_key(value);
    if matches!(
        value.as_str(),
        "test"
            | "report"
            | "artifact"
            | "screenshot"
            | "log"
            | "runtime"
            | "design"
            | "decision"
            | "other"
    ) {
        return value;
    }
    match value.as_str() {
        "document" | "integration_report" | "review" | "review_report" => "report".into(),
        "git_commit" | "commit" | "deliverable" | "file" | "project_output" => "artifact".into(),
        "qa" | "validation" | "test_report" => "test".into(),
        "image" | "screen_capture" => "screenshot".into(),
        "runtime_health" | "health_check" => "runtime".into(),
        "svg" | "ui" | "ux" | "ui_design" | "ux_design" => "design".into(),
        value => value.into(),
    }
}

fn normalize_evidence_result(value: &str) -> String {
    match normalized_key(value).as_str() {
        "pass" | "success" | "successful" | "succeeded" | "ok" => "passed".into(),
        "error" | "errored" | "unsuccessful" => "failed".into(),
        "blocked" | "pending" | "not_run" | "skipped" | "content_ready_git_blocked" => {
            "inconclusive".into()
        }
        "info" | "complete" | "completed" => "informational".into(),
        value => value.into(),
    }
}

fn evidence_title_from_summary(summary: &str) -> String {
    summary
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or(summary)
        .trim()
        .chars()
        .take(120)
        .collect()
}
