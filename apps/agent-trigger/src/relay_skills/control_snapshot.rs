use super::*;
use ai_chat_application::{AgentControlSnapshot, ProjectTaskWaitingReason};
use serde_json::{json, Map, Value};

const MESSAGE_BUDGET: usize = 5_000;
const ACTION_BUDGET: usize = 1_500;
const TASK_BUDGET: usize = 1_500;
const INTENT_BUDGET: usize = 900;
const SESSION_BUDGET: usize = 900;

pub(super) fn render_control_snapshot(snapshot: &AgentControlSnapshot) -> String {
    let readiness_by_task = snapshot
        .task_readiness
        .iter()
        .map(|readiness| (readiness.task_id, readiness))
        .collect::<std::collections::HashMap<_, _>>();
    let mut unread_messages = Vec::new();
    let mut unread_message_chars = 0;
    for event in snapshot.unread_messages.iter().take(50) {
        let value = json!({
            "event_id": event.id,
            "created_at": event.created_at,
            "conversation_id": event.payload_json.get("conversation_id"),
            "message_id": event.payload_json.get("message_id"),
            "sender_agent_id": event.payload_json.get("sender_agent_id"),
            "sender_human_user_id": event.payload_json.get("sender_human_user_id"),
            "content": event
                .payload_json
                .get("content")
                .and_then(Value::as_str)
                .map(|content| truncate(content, 1_200)),
            "mentioned": event.payload_json.get("mentioned"),
            "requires_action": event.requires_action,
        });
        if !push_json_with_budget(
            &mut unread_messages,
            value,
            &mut unread_message_chars,
            MESSAGE_BUDGET,
        ) {
            break;
        }
    }
    let mut actionable_events = Vec::new();
    let mut actionable_event_chars = 0;
    for event in snapshot
        .actionable_events
        .iter()
        .filter(|event| event.event_type != "message.received")
        .take(20)
    {
        let value = json!({
            "id": event.id,
            "type": event.event_type,
            "class": event.event_class,
            "priority": event.priority,
            "payload": compact_event_payload(&event.payload_json),
        });
        if !push_json_with_budget(
            &mut actionable_events,
            value,
            &mut actionable_event_chars,
            ACTION_BUDGET,
        ) {
            break;
        }
    }
    let mut ready_tasks = Vec::new();
    let mut task_chars = 0;
    for task in snapshot.ready_tasks.iter().take(20) {
        let readiness = readiness_by_task.get(&task.id);
        let value = json!({
            "id": task.id,
            "project_id": task.project_id,
            "title": truncate(&task.title, 240),
            "status": task.status,
            "priority": task.priority,
            "readiness": readiness.map(|item| json!({
                "can_start": item.can_start,
                "waiting_reasons": compact_waiting_reasons(&item.waiting_reasons, 5, 240),
                "suggested_actions": compact_strings(&item.suggested_actions, 5, 240),
            })),
        });
        if !push_json_with_budget(&mut ready_tasks, value, &mut task_chars, TASK_BUDGET) {
            break;
        }
    }
    let mut waiting_tasks = Vec::new();
    for task in snapshot.waiting_tasks.iter().take(20) {
        let readiness = readiness_by_task.get(&task.id);
        let value = json!({
            "id": task.id,
            "project_id": task.project_id,
            "title": truncate(&task.title, 240),
            "status": task.status,
            "readiness": readiness.map(|item| json!({
                "can_start": item.can_start,
                "waiting_reasons": compact_waiting_reasons(&item.waiting_reasons, 5, 240),
                "suggested_actions": compact_strings(&item.suggested_actions, 5, 240),
            })),
        });
        if !push_json_with_budget(&mut waiting_tasks, value, &mut task_chars, TASK_BUDGET) {
            break;
        }
    }
    let mut active_intents = Vec::new();
    let mut intent_chars = 0;
    for intent in snapshot.active_intents.iter().take(10) {
        let value = json!({
            "id": intent.id,
            "project_id": intent.project_id,
            "status": intent.status,
            "objective": truncate(&intent.objective, 500),
            "task_ids": intent.task_ids,
        });
        if !push_json_with_budget(&mut active_intents, value, &mut intent_chars, INTENT_BUDGET) {
            break;
        }
    }
    let mut work_sessions = Vec::new();
    let mut session_chars = 0;
    for session in snapshot.work_sessions.iter().take(10) {
        let value = json!({
            "id": session.id,
            "kind": session.session_kind,
            "project_id": session.project_id,
            "status": session.status,
            "checkpoint": truncate(&session.summary_short, 500),
        });
        if !push_json_with_budget(
            &mut work_sessions,
            value,
            &mut session_chars,
            SESSION_BUDGET,
        ) {
            break;
        }
    }
    let unread_messages_truncated = unread_messages.len() < snapshot.unread_messages.len();
    let non_message_actionable_total = snapshot
        .actionable_events
        .iter()
        .filter(|event| event.event_type != "message.received")
        .count();
    serde_json::to_string(&json!({
        "unread_messages_total": snapshot.unread_messages.len(),
        "unread_messages_in_prompt": unread_messages.len(),
        "unread_messages_truncated": unread_messages_truncated,
        "unread_messages": unread_messages,
        "actionable_events_total": snapshot.actionable_events.len(),
        "actionable_message_events_covered_by_unread_messages": snapshot
            .actionable_events
            .iter()
            .filter(|event| event.event_type == "message.received")
            .count(),
        "non_message_actionable_events_truncated": actionable_events.len() < non_message_actionable_total,
        "actionable_events": actionable_events,
        "ready_tasks_total": snapshot.ready_tasks.len(),
        "ready_tasks": ready_tasks,
        "waiting_tasks_total": snapshot.waiting_tasks.len(),
        "waiting_tasks": waiting_tasks,
        "active_intents_total": snapshot.active_intents.len(),
        "active_intents": active_intents,
        "work_sessions_total": snapshot.work_sessions.len(),
        "work_sessions": work_sessions,
    }))
    .unwrap_or_else(|_| "{}".into())
}

fn push_json_with_budget(
    values: &mut Vec<Value>,
    value: Value,
    used_characters: &mut usize,
    max_characters: usize,
) -> bool {
    let value_characters = serde_json::to_string(&value)
        .map(|serialized| serialized.chars().count())
        .unwrap_or(max_characters);
    if !values.is_empty() && used_characters.saturating_add(value_characters) > max_characters {
        return false;
    }
    *used_characters = used_characters.saturating_add(value_characters);
    values.push(value);
    true
}

fn compact_strings(values: &[String], max_items: usize, max_characters: usize) -> Vec<String> {
    values
        .iter()
        .take(max_items)
        .map(|value| truncate(value, max_characters))
        .collect()
}

fn compact_waiting_reasons(
    values: &[ProjectTaskWaitingReason],
    max_items: usize,
    max_characters: usize,
) -> Vec<Value> {
    values
        .iter()
        .take(max_items)
        .map(|reason| {
            json!({
                "kind": truncate(&reason.kind, 80),
                "code": truncate(&reason.code, 80),
                "summary": truncate(&reason.summary, max_characters),
                "related_id": reason.related_id,
            })
        })
        .collect()
}

fn compact_event_payload(payload: &Value) -> Value {
    const IMPORTANT_FIELDS: &[&str] = &[
        "project_id",
        "task_id",
        "conversation_id",
        "message_id",
        "status",
        "previous_status",
        "title",
        "reason",
        "mentioned",
        "requires_action",
    ];
    let Some(object) = payload.as_object() else {
        return compact_json_value(payload, 0);
    };
    let mut compact = Map::new();
    for field in IMPORTANT_FIELDS {
        if let Some(value) = object.get(*field) {
            compact.insert((*field).into(), compact_json_value(value, 0));
        }
    }
    Value::Object(compact)
}

fn compact_json_value(value: &Value, depth: usize) -> Value {
    if depth >= 2 {
        return Value::String("…".into());
    }
    match value {
        Value::String(value) => Value::String(truncate(value, 300)),
        Value::Array(values) => Value::Array(
            values
                .iter()
                .take(8)
                .map(|value| compact_json_value(value, depth + 1))
                .collect(),
        ),
        Value::Object(values) => Value::Object(
            values
                .iter()
                .take(12)
                .map(|(key, value)| (key.clone(), compact_json_value(value, depth + 1)))
                .collect(),
        ),
        _ => value.clone(),
    }
}

pub(crate) fn normalize_codex_prompt(value: &str) -> String {
    let mut normalized = String::with_capacity(value.len());
    let mut characters = value.chars().peekable();
    while let Some(character) = characters.next() {
        match character {
            '\r' => {
                if characters.peek() == Some(&'\n') {
                    characters.next();
                }
                normalized.push('\n');
            }
            '\n' | '\t' => normalized.push(character),
            character if character.is_control() => normalized.push(' '),
            character => normalized.push(character),
        }
    }
    normalized
}
