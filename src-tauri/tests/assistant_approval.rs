use engine::{CapabilityRef, WorkflowPatch, WorkflowPatchOperation};
use oh_my_dream_tauri::assistant_approval::{
    PendingApprovalService, PendingApprovalSqliteRepository,
};
use oh_my_dream_tauri::assistant_commands::{
    AssistantSendInput, assistant_get_pending_approval_with_state, assistant_send_with_state,
};
use oh_my_dream_tauri::assistant_runtime::AssistantWaitingApproval;
use oh_my_dream_tauri::reviewed_change::{PrepareCandidateInput, RecordReviewInput, ReviewVerdict};
use oh_my_dream_tauri::state::AppState;
use serde_json::Map;
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

#[test]
fn pending_approval_exposes_the_exact_reviewed_candidate() {
    let root = tempdir().expect("root");
    let state = AppState::from_asset_root(root.path()).expect("state");
    let candidate = state
        .reviewed_change
        .prepare(PrepareCandidateInput {
            project_id: "project".to_owned(),
            session_id: "project:project".to_owned(),
            user_intent: "Build a film".to_owned(),
            expected_revision: None,
            prior_candidate_id: None,
            patch: WorkflowPatch {
                operations: vec![WorkflowPatchOperation::AddNode {
                    alias: "prompt".to_owned(),
                    capability: CapabilityRef::new("TextPrompt", "1.0"),
                    params: Map::new(),
                    position: None,
                }],
            },
        })
        .expect("candidate");
    let receipt = state
        .reviewed_change
        .record_review(RecordReviewInput {
            project_id: "project".to_owned(),
            session_id: "project:project".to_owned(),
            candidate_id: candidate.id().to_owned(),
            candidate_digest: candidate.digest().to_owned(),
            reviewer_version: "reviewer-v1".to_owned(),
            verdict: ReviewVerdict::Pass,
            evidence_hash: "sha256:evidence".to_owned(),
            summary: "Ready to apply".to_owned(),
            findings: vec!["Matches the requested production".to_owned()],
        })
        .expect("receipt");
    let pending: AssistantWaitingApproval = serde_json::from_value(json!({
        "state": {},
        "pending": {"call_id":"call-1","operation_id":"workflow_apply_reviewed_candidate","operation_version":1,"arguments_json":format!("{{\"review_receipt_id\":\"{}\"}}", receipt.id())},
        "project_id":"project","session_id":"project:project","session_path":"/tmp/session.sqlite3"
    }))
    .expect("pending");
    state.pending_approval.save(&pending).expect("save");

    let dto = assistant_get_pending_approval_with_state("project", &state)
        .expect("lookup")
        .expect("approval");
    assert_eq!(dto.user_intent, "Build a film");
    assert_eq!(dto.candidate_digest, candidate.digest());
    assert_eq!(dto.review_summary, "Ready to apply");
    assert_eq!(dto.workflow["nodes"].as_array().expect("nodes").len(), 1);
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
