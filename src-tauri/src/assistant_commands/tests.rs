use super::*;
use crate::assistant_runtime::AssistantEventSink;
use serde_json::{Value, json};
use std::path::Path;
use std::sync::{Arc, Mutex};
use tauri::ipc::{Channel, InvokeResponseBody};
use tempfile::tempdir;

#[test]
fn production_runtime_exposes_plan_memory_without_scheduler_tools() {
    let root = tempdir().expect("create app root");
    let state = AppState::from_roots(root.path().join("assets"), root.path().join("config"))
        .expect("build app state");
    let ids = operation_registrations(&state)
        .expect("build production registrations")
        .into_iter()
        .map(|registration| registration.id().to_owned())
        .collect::<Vec<_>>();
    for expected in [
        "workflow_evaluate_patch",
        "workflow_prepare_patch",
        "workflow_candidate_get",
        "workflow_apply_reviewed_candidate",
        "production_plan_get",
        "production_plan_create",
        "production_plan_replace",
        "production_plan_update_item",
    ] {
        assert!(ids.iter().any(|id| id == expected), "missing {expected}");
    }
    assert!(ids.iter().all(|id| !id.contains("next") && !id.contains("claim")));
    assert!(!ids.iter().any(|id| id == "workflow_apply_patch"));
}

#[test]
fn channel_sink_forwards_native_response_value_unchanged() {
    let events = Arc::new(Mutex::new(Vec::<Value>::new()));
    let channel = Channel::new({
        let events = Arc::clone(&events);
        move |body| {
            let InvokeResponseBody::Json(encoded) = body else {
                panic!("assistant event should use JSON IPC");
            };
            let event = serde_json::from_str(&encoded).expect("decode assistant event");
            events.lock().expect("event lock").push(event);
            Ok(())
        }
    });
    let mut sink = ChannelAssistantSink { channel };
    let event = json!({"type":"response.output_text.delta","delta":"native","sequence_number":4});
    sink.emit(event.clone()).expect("channel should accept event");
    assert_eq!(*events.lock().expect("event lock"), vec![event]);
}

#[test]
fn project_session_is_stable_while_turn_ids_are_rust_owned() {
    let root = tempdir().expect("root");
    let first = assistant_identity(root.path(), "project-1").expect("first");
    let second = assistant_identity(root.path(), "project-1").expect("second");
    assert_eq!(first.session_id, "project:project-1");
    assert_eq!(first.session_id, second.session_id);
    assert_eq!(first.session_path, second.session_path);
    assert_ne!(first.invocation_id, second.invocation_id);
    assert_ne!(first.request_id, second.request_id);
    assert!(first.session_path.starts_with(root.path()));
    assert!(!first.session_path.starts_with(Path::new("project-1")));
}

#[test]
fn invocation_keeps_user_text_and_selection_in_trusted_scope() {
    let root = tempdir().expect("root");
    let input = AssistantSendInput {
        project_id: "project-1".to_owned(),
        workflow_present: true,
        workflow_revision: Some(4),
        selected_node_ids: vec!["node-1".to_owned()],
        selected_asset_ids: vec!["asset-1".to_owned()],
        text: "  preserve this exact text  ".to_owned(),
    };
    let (invocation, trusted) = build_invocation(&input, root.path()).expect("turn");
    assert_eq!(invocation.input(), Some("  preserve this exact text  "));
    assert_eq!(trusted.project_id(), "project-1");
    assert_eq!(trusted.selected_node_ids(), ["node-1"]);
    assert_eq!(trusted.selected_asset_ids(), ["asset-1"]);
    assert!(trusted.request_id().starts_with("assistant-request-"));
}

#[test]
fn active_session_guard_rejects_only_the_same_session() {
    let root = tempdir().expect("root");
    let state = AppState::from_asset_root(root.path()).expect("state");
    let first = ActiveAssistantSession::acquire(&state, "session-1").expect("first");
    assert_eq!(
        ActiveAssistantSession::acquire(&state, "session-1").err(),
        Some("ASSISTANT_SESSION_ACTIVE".to_owned())
    );
    let other = ActiveAssistantSession::acquire(&state, "session-2").expect("other");
    drop(first);
    ActiveAssistantSession::acquire(&state, "session-1").expect("released");
    drop(other);
}
