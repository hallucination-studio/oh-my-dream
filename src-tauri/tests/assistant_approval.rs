use oh_my_dream_tauri::assistant_approval::{
    PendingApprovalService, PendingApprovalSqliteRepository,
};
use oh_my_dream_tauri::assistant_commands::{AssistantSendInput, assistant_send_with_state};
use oh_my_dream_tauri::assistant_runtime::AssistantWaitingApproval;
use oh_my_dream_tauri::state::AppState;
use serde_json::json;
use std::sync::Arc;
use tauri::ipc::{Channel, InvokeResponseBody};
use tempfile::tempdir;

fn waiting(session_path: &str) -> AssistantWaitingApproval {
    serde_json::from_value(json!({
        "state": {"opaque": true},
        "pending": {
            "call_id": "call-1",
            "operation_id": "workflow_apply_reviewed_candidate",
            "operation_version": 1,
            "arguments_json": "{\"review_receipt_id\":\"receipt-1\"}"
        },
        "project_id": "project",
        "session_id": "project:project",
        "session_path": session_path
    }))
    .expect("waiting approval")
}

#[test]
fn pending_run_state_survives_a_fresh_service_and_deletes_after_decision() {
    let root = tempdir().expect("root");
    let path = PendingApprovalSqliteRepository::path(root.path());
    let first = PendingApprovalService::new(Arc::new(
        PendingApprovalSqliteRepository::open(&path).expect("repository"),
    ));
    first.save(&waiting("/tmp/session.sqlite3")).expect("save");
    drop(first);

    let reopened = PendingApprovalService::new(Arc::new(
        PendingApprovalSqliteRepository::open(&path).expect("reopen"),
    ));
    let restored = reopened.load("project:project").expect("load").expect("pending");
    assert_eq!(restored.pending().call_id(), "call-1");
    assert_eq!(restored.state(), &json!({"opaque": true}));
    reopened.delete("project:project").expect("delete");
    assert!(reopened.load("project:project").expect("load deleted").is_none());
}

#[tokio::test]
async fn new_user_turn_is_rejected_while_the_same_session_waits_for_approval() {
    let root = tempdir().expect("root");
    let state = AppState::from_asset_root(root.path()).expect("state");
    state.pending_approval.save(&waiting("/tmp/session.sqlite3")).expect("save");
    let channel = Channel::new(|_body: InvokeResponseBody| Ok(()));

    let error = assistant_send_with_state(
        AssistantSendInput {
            project_id: "project".to_owned(),
            workflow_present: false,
            workflow_revision: None,
            selected_node_ids: Vec::new(),
            selected_asset_ids: Vec::new(),
            text: "new turn".to_owned(),
        },
        channel,
        &state,
    )
    .await
    .expect_err("pending session must reject a new turn");

    assert_eq!(error, "ASSISTANT_APPROVAL_PENDING");
}
