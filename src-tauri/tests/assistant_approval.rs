use engine::{CapabilityRef, WorkflowPatch, WorkflowPatchOperation};
use oh_my_dream_tauri::assistant_approval::{
    PendingApprovalService, PendingApprovalSqliteRepository,
};
use oh_my_dream_tauri::assistant_commands::{
    AssistantApprovalDecisionInput, AssistantSendInput, assistant_decide_approval_with_state,
    assistant_get_pending_approval_with_state, assistant_send_with_state,
};
use oh_my_dream_tauri::assistant_runtime::{AssistantSidecarCommand, AssistantWaitingApproval};
use oh_my_dream_tauri::reviewed_change::{PrepareCandidateInput, RecordReviewInput, ReviewVerdict};
use oh_my_dream_tauri::state::AppState;
use oh_my_dream_tauri::workflow_authority::WorkflowCommitRequest;
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

#[tokio::test]
async fn approval_command_applies_once_replays_and_rejection_has_no_effect() {
    let root = tempdir().expect("root");
    let (mut state, receipt_id) = reviewed_state(root.path());
    state.assistant_sidecar_command = approval_sidecar(&receipt_id);
    let channel = Channel::new(|_body: InvokeResponseBody| Ok(()));

    state.pending_approval.save(&waiting_for(&receipt_id, root.path())).expect("pending");
    let stale = assistant_decide_approval_with_state(
        AssistantApprovalDecisionInput {
            project_id: "project".to_owned(),
            approval_scope_id: "stale-scope".to_owned(),
            candidate_digest: "sha256:stale".to_owned(),
            approved: true,
        },
        channel.clone(),
        &state,
    )
    .await
    .expect_err("stale decision");
    assert_eq!(stale, "ASSISTANT_APPROVAL_STALE");
    let first = assistant_decide_approval_with_state(
        AssistantApprovalDecisionInput {
            project_id: "project".to_owned(),
            approval_scope_id: pending_scope(&state),
            candidate_digest: pending_digest(&state),
            approved: true,
        },
        channel.clone(),
        &state,
    )
    .await
    .expect("approve")
    .expect("head");
    assert_eq!(first.revision, 1);

    state.pending_approval.save(&waiting_for(&receipt_id, root.path())).expect("replay pending");
    let replay = assistant_decide_approval_with_state(
        AssistantApprovalDecisionInput {
            project_id: "project".to_owned(),
            approval_scope_id: pending_scope(&state),
            candidate_digest: pending_digest(&state),
            approved: true,
        },
        channel.clone(),
        &state,
    )
    .await
    .expect("replay")
    .expect("head");
    assert_eq!(replay.revision, 1);

    state.pending_approval.save(&waiting_for(&receipt_id, root.path())).expect("reject pending");
    let rejected = assistant_decide_approval_with_state(
        AssistantApprovalDecisionInput {
            project_id: "project".to_owned(),
            approval_scope_id: pending_scope(&state),
            candidate_digest: pending_digest(&state),
            approved: false,
        },
        channel,
        &state,
    )
    .await
    .expect("reject");
    assert!(rejected.is_none());
    assert_eq!(state.workflow_authority.load_head("project").expect("head").unwrap().revision, 1);
}

#[tokio::test]
async fn approval_command_rejects_a_candidate_with_a_stale_base() {
    let root = tempdir().expect("root");
    let (mut state, receipt_id) = reviewed_state(root.path());
    state.assistant_sidecar_command = approval_sidecar(&receipt_id);
    state
        .workflow_authority
        .apply(WorkflowCommitRequest::new(
            "project",
            None,
            "manual-edit",
            "sha256:manual-edit",
            serde_json::from_value(json!({
                "version":"1.0","project_id":"project","nodes":[{
                    "id":"manual","type":"TextPrompt","contract_version":"1.0",
                    "params":{},"inputs":{},"position":null
                }]
            }))
            .expect("workflow"),
        ))
        .expect("manual edit");
    state.pending_approval.save(&waiting_for(&receipt_id, root.path())).expect("pending");

    let error = assistant_decide_approval_with_state(
        AssistantApprovalDecisionInput {
            project_id: "project".to_owned(),
            approval_scope_id: pending_scope(&state),
            candidate_digest: pending_digest(&state),
            approved: true,
        },
        Channel::new(|_body: InvokeResponseBody| Ok(())),
        &state,
    )
    .await
    .expect_err("stale base");

    assert_eq!(error, "reviewed Workflow apply did not commit");
    assert_eq!(state.workflow_authority.load_head("project").expect("head").unwrap().revision, 1);
}

fn reviewed_state(root: &std::path::Path) -> (AppState, String) {
    let state = AppState::from_asset_root(root).expect("state");
    state
        .store
        .lock()
        .expect("store")
        .create_project_with_id("project", "Project")
        .expect("project");
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
            summary: "Ready".to_owned(),
            findings: Vec::new(),
        })
        .expect("receipt");
    (state, receipt.id().to_owned())
}

fn waiting_for(receipt_id: &str, root: &std::path::Path) -> AssistantWaitingApproval {
    serde_json::from_value(json!({
        "state": {},
        "pending": {"call_id":"call-1","operation_id":"workflow_apply_reviewed_candidate","operation_version":1,"arguments_json":format!("{{\"review_receipt_id\":\"{receipt_id}\"}}")},
        "project_id":"project","session_id":"project:project","session_path":root.join("session.sqlite3")
    }))
    .expect("waiting")
}

fn approval_sidecar(receipt_id: &str) -> AssistantSidecarCommand {
    let script = format!(
        r#"import json,sys
invoke=json.loads(sys.stdin.readline())['payload']
approval=json.loads(sys.stdin.readline())['payload']
inv=invoke['invocation_id']
if approval['approved']:
 request={{'protocol_version':1,'sequence':0,'kind':'tool_request','payload':{{'invocation_id':inv,'operation_id':'workflow_apply_reviewed_candidate','call_id':'call-1','arguments_json':'{{"review_receipt_id":"{receipt_id}"}}'}}}}
 print(json.dumps(request),flush=True); sys.stdin.readline(); start=1
else: start=0
frames=[('snapshot',{{'invocation_id':inv,'session_id':'project:project','status':'completed','state':None}}),('completed',{{'invocation_id':inv,'final_output':'done'}})]
[print(json.dumps({{'protocol_version':1,'sequence':i+start,'kind':kind,'payload':payload}}),flush=True) for i,(kind,payload) in enumerate(frames)]
sys.stdin.read()"#
    );
    AssistantSidecarCommand::new("python").args(["-c", &script])
}

fn pending_digest(state: &AppState) -> String {
    assistant_get_pending_approval_with_state("project", state)
        .expect("pending lookup")
        .expect("pending approval")
        .candidate_digest
}

fn pending_scope(state: &AppState) -> String {
    assistant_get_pending_approval_with_state("project", state)
        .expect("pending lookup")
        .expect("pending approval")
        .approval_scope_id
}
