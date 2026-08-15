use super::*;

pub(super) const EVENT_CLASS_INFORMATIONAL: &str = "informational";
pub(super) const EVENT_CLASS_DIGESTIBLE: &str = "digestible";
pub(super) const EVENT_CLASS_ACTIONABLE: &str = "actionable";
pub(super) const EVENT_CLASS_BLOCKING: &str = "blocking";
pub(super) const EVENT_CLASS_EXECUTION_READY: &str = "execution_ready";

pub(super) const EVENT_WAKE_NEVER: &str = "never";
pub(super) const EVENT_WAKE_DEFERRED: &str = "deferred";
pub(super) const EVENT_WAKE_IMMEDIATE: &str = "immediate";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct EventRoutingDecision {
    pub(super) event_class: &'static str,
    pub(super) requires_action: bool,
    pub(super) wake_policy: &'static str,
    pub(super) dedupe_key: Option<String>,
    pub(super) coalesce_key: Option<String>,
    pub(super) causation_id: Option<Uuid>,
    pub(super) correlation_id: Option<Uuid>,
}

impl EventRoutingDecision {
    fn actionable() -> Self {
        Self {
            event_class: EVENT_CLASS_ACTIONABLE,
            requires_action: true,
            wake_policy: EVENT_WAKE_IMMEDIATE,
            dedupe_key: None,
            coalesce_key: None,
            causation_id: None,
            correlation_id: None,
        }
    }
}

pub(super) fn route_agent_event(
    event_type: &str,
    payload: &serde_json::Value,
) -> EventRoutingDecision {
    let mut decision = match event_type {
        "message.received" => route_message_event(payload),
        "company.project.task_ready" => EventRoutingDecision {
            event_class: EVENT_CLASS_EXECUTION_READY,
            requires_action: true,
            wake_policy: EVENT_WAKE_IMMEDIATE,
            dedupe_key: payload_uuid(payload, "task_id")
                .map(|task_id| format!("task-ready:{task_id}")),
            coalesce_key: None,
            causation_id: None,
            correlation_id: payload_uuid(payload, "project_id"),
        },
        "company.project.task_status_changed" => EventRoutingDecision {
            event_class: EVENT_CLASS_ACTIONABLE,
            requires_action: true,
            wake_policy: EVENT_WAKE_IMMEDIATE,
            dedupe_key: None,
            coalesce_key: None,
            causation_id: None,
            correlation_id: payload_uuid(payload, "project_id"),
        },
        "company.project.task_assigned" | "company.project.member_added" => EventRoutingDecision {
            event_class: EVENT_CLASS_INFORMATIONAL,
            requires_action: false,
            wake_policy: EVENT_WAKE_NEVER,
            dedupe_key: match event_type {
                "company.project.task_assigned" => payload_uuid(payload, "task_id")
                    .map(|task_id| format!("task-assigned:{task_id}")),
                _ => payload_uuid(payload, "project_id")
                    .map(|project_id| format!("project-member-added:{project_id}")),
            },
            coalesce_key: None,
            causation_id: None,
            correlation_id: payload_uuid(payload, "project_id"),
        },
        "company.project.status_updated" => EventRoutingDecision {
            event_class: EVENT_CLASS_DIGESTIBLE,
            requires_action: false,
            wake_policy: EVENT_WAKE_NEVER,
            dedupe_key: payload_uuid(payload, "project_id")
                .map(|project_id| format!("project-status:{project_id}")),
            coalesce_key: payload_uuid(payload, "project_id")
                .map(|project_id| format!("project-status:{project_id}")),
            causation_id: None,
            correlation_id: payload_uuid(payload, "project_id"),
        },
        value if value.contains("approval") || value.contains("failed") => EventRoutingDecision {
            event_class: EVENT_CLASS_BLOCKING,
            requires_action: true,
            wake_policy: EVENT_WAKE_IMMEDIATE,
            dedupe_key: None,
            coalesce_key: None,
            causation_id: None,
            correlation_id: payload_uuid(payload, "project_id"),
        },
        _ => EventRoutingDecision::actionable(),
    };
    decision.causation_id = payload_uuid(payload, "causation_id");
    if decision.correlation_id.is_none() {
        decision.correlation_id =
            payload_uuid(payload, "correlation_id").or_else(|| payload_uuid(payload, "project_id"));
    }
    decision
}

fn route_message_event(payload: &serde_json::Value) -> EventRoutingDecision {
    let requires_action = payload
        .get("delivery_requires_action")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true);
    let owner_followup = payload
        .get("project_owner_followup")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let project_id = payload_uuid(payload, "project_id");
    let message_id = payload_uuid(payload, "message_id");
    if requires_action {
        return EventRoutingDecision {
            event_class: EVENT_CLASS_ACTIONABLE,
            requires_action: true,
            wake_policy: EVENT_WAKE_IMMEDIATE,
            dedupe_key: message_id.map(|id| format!("message:{id}")),
            coalesce_key: None,
            causation_id: None,
            correlation_id: project_id,
        };
    }
    if owner_followup {
        let owner_followup_key = project_id.map(|id| format!("project-owner-followup:{id}"));
        return EventRoutingDecision {
            event_class: EVENT_CLASS_DIGESTIBLE,
            requires_action: false,
            wake_policy: EVENT_WAKE_DEFERRED,
            dedupe_key: owner_followup_key.clone(),
            coalesce_key: owner_followup_key,
            causation_id: None,
            correlation_id: project_id,
        };
    }
    EventRoutingDecision {
        event_class: EVENT_CLASS_INFORMATIONAL,
        requires_action: false,
        wake_policy: EVENT_WAKE_NEVER,
        dedupe_key: message_id.map(|id| format!("message:{id}")),
        coalesce_key: project_id.map(|id| format!("project-message:{id}")),
        causation_id: None,
        correlation_id: project_id,
    }
}

fn payload_uuid(payload: &serde_json::Value, field: &str) -> Option<Uuid> {
    payload
        .get(field)
        .and_then(serde_json::Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_project_message_is_informational() {
        let project_id = Uuid::new_v4();
        let decision = route_agent_event(
            "message.received",
            &json!({
                "project_id": project_id,
                "delivery_requires_action": false
            }),
        );
        assert_eq!(decision.event_class, EVENT_CLASS_INFORMATIONAL);
        assert!(!decision.requires_action);
        assert_eq!(decision.wake_policy, EVENT_WAKE_NEVER);
    }

    #[test]
    fn mentioned_message_is_actionable() {
        let decision = route_agent_event(
            "message.received",
            &json!({
                "message_id": Uuid::new_v4(),
                "delivery_requires_action": true
            }),
        );
        assert_eq!(decision.event_class, EVENT_CLASS_ACTIONABLE);
        assert!(decision.requires_action);
        assert_eq!(decision.wake_policy, EVENT_WAKE_IMMEDIATE);
    }

    #[test]
    fn task_ready_has_stable_dedupe_key() {
        let task_id = Uuid::new_v4();
        let decision =
            route_agent_event("company.project.task_ready", &json!({ "task_id": task_id }));
        assert_eq!(decision.event_class, EVENT_CLASS_EXECUTION_READY);
        assert_eq!(decision.dedupe_key, Some(format!("task-ready:{task_id}")));
    }

    #[test]
    fn task_status_change_is_actionable_for_project_management() {
        let project_id = Uuid::new_v4();
        let task_id = Uuid::new_v4();
        let decision = route_agent_event(
            "company.project.task_status_changed",
            &json!({ "project_id": project_id, "task_id": task_id }),
        );
        assert_eq!(decision.event_class, EVENT_CLASS_ACTIONABLE);
        assert!(decision.requires_action);
        assert_eq!(decision.wake_policy, EVENT_WAKE_IMMEDIATE);
        assert_eq!(decision.correlation_id, Some(project_id));
    }

    #[test]
    fn task_assignment_is_informational_until_readiness_is_confirmed() {
        let task_id = Uuid::new_v4();
        let decision = route_agent_event(
            "company.project.task_assigned",
            &json!({ "task_id": task_id }),
        );
        assert_eq!(decision.event_class, EVENT_CLASS_INFORMATIONAL);
        assert!(!decision.requires_action);
        assert_eq!(decision.wake_policy, EVENT_WAKE_NEVER);
        assert_eq!(
            decision.dedupe_key,
            Some(format!("task-assigned:{task_id}"))
        );
    }

    #[test]
    fn project_membership_notice_does_not_start_control_work() {
        let project_id = Uuid::new_v4();
        let decision = route_agent_event(
            "company.project.member_added",
            &json!({ "project_id": project_id }),
        );
        assert_eq!(decision.event_class, EVENT_CLASS_INFORMATIONAL);
        assert!(!decision.requires_action);
        assert_eq!(decision.wake_policy, EVENT_WAKE_NEVER);
    }
}
