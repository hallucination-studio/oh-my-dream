use backends::MockBackendImpl;
use engine::{NodeRef, Workflow, WorkflowPatch, WorkflowPatchOperation};
use oh_my_dream_tauri::assistant_commands::{
    AssistantApprovalDecisionInput, assistant_decide_approval_with_state,
    assistant_get_pending_approval_with_state,
};
use oh_my_dream_tauri::assistant_operations::{ApprovedEffect, RequestContext};
use oh_my_dream_tauri::assistant_repair::{ApprovedWorkflowAction, AssistantRepairService};
use oh_my_dream_tauri::assistant_runtime::{AssistantSidecarCommand, AssistantWaitingApproval};
use oh_my_dream_tauri::reviewed_change::{
    PrepareCandidateInput, RecordReviewInput, ReviewVerdict, ReviewedChangeOperations,
};
use oh_my_dream_tauri::state::AppState;
use oh_my_dream_tauri::workflow_authority::WorkflowCommitRequest;
use oh_my_dream_tauri::workflow_patch_operation::WorkflowPatchService;
use oh_my_dream_tauri::workflow_run_dto::WorkflowRunResultDto;
use serde_json::json;
use std::sync::{Arc, Mutex};
use tauri::ipc::{Channel, InvokeResponseBody};
use tempfile::tempdir;

const WORKFLOW_JSON: &str = r#"{
  "version": "1.0",
  "project_id": "project",
  "nodes": [
    { "id": "prompt", "type": "TextPrompt", "params": { "text": "a red fox" }, "inputs": {} },
    { "id": "image", "type": "TextToImage", "params": {}, "inputs": { "prompt": ["prompt", "text"] } }
  ]
}"#;

#[test]
fn assistant_repair_injected_failure_creates_only_a_factual_same_session_activation() {
    let state = state_with_backend(Arc::new(MockBackendImpl::always_fails("provider outage")));
    let service = AssistantRepairService::from_state(&state);
    let action = ApprovedWorkflowAction::new("project", "approval-scope-1", 1);

    let first = service.execute(&action).expect("mock run outcome");
    let second = service.execute(&action).expect("stable action retry");

    assert_eq!(first.run_id, second.run_id);
    assert!(matches!(first.outcome, WorkflowRunResultDto::Failed { .. }));
    let activation = first.activation.expect("failure activation");
    assert_eq!(activation.session_id, "project:project");
    assert_eq!(activation.project_id, "project");
    assert_eq!(activation.workflow_revision, 1);
    assert_eq!(activation.run_id, first.run_id);
    assert!(activation.reason.contains("provider outage"));
    let fact = serde_json::to_string(&activation).expect("activation JSON");
    assert!(!fact.contains("patch"));
    assert!(!fact.contains("candidate"));
    assert!(!fact.contains("repair_step"));
}

#[test]
fn assistant_repair_reviewed_action_uses_a_new_stable_run_and_can_succeed() {
    let state = state_with_backend(Arc::new(MockBackendImpl::new()));
    let service = AssistantRepairService::from_state(&state);
    let repaired = ApprovedWorkflowAction::new("project", "approval-scope-2", 1);

    let result = service.execute(&repaired).expect("repaired mock run");

    assert!(matches!(result.outcome, WorkflowRunResultDto::Succeeded { .. }));
    assert!(result.activation.is_none());
    assert_eq!(result.run_id, AssistantRepairService::run_id(&repaired));
    assert_ne!(
        result.run_id,
        AssistantRepairService::run_id(&ApprovedWorkflowAction::new(
            "project",
            "approval-scope-1",
            1,
        ))
    );
}

#[tokio::test]
async fn assistant_repair_rejection_revision_second_approval_applies_exact_repair_and_reruns() {
    let state = state_with_backend(Arc::new(MockBackendImpl::always_fails("provider outage")));
    let rejected = state
        .reviewed_change
        .prepare(PrepareCandidateInput {
            project_id: "project".to_owned(),
            session_id: "project:project".to_owned(),
            user_intent: "Repair the failed run".to_owned(),
            expected_revision: Some(1),
            prior_candidate_id: None,
            patch: WorkflowPatch {
                operations: vec![WorkflowPatchOperation::RemoveNode {
                    node: NodeRef::Id { id: "image".to_owned() },
                }],
            },
        })
        .expect("first repair candidate");
    let rejected_receipt = review(&state, &rejected, ReviewVerdict::Reject);
    assert!(
        !state
            .reviewed_change
            .valid_passed_receipt("project", "project:project", rejected_receipt.id())
            .expect("rejected receipt check")
    );
    let revised = state
        .reviewed_change
        .prepare(PrepareCandidateInput {
            project_id: "project".to_owned(),
            session_id: "project:project".to_owned(),
            user_intent: "Repair the failed run".to_owned(),
            expected_revision: Some(1),
            prior_candidate_id: Some(rejected.id().to_owned()),
            patch: WorkflowPatch {
                operations: vec![WorkflowPatchOperation::SetPosition {
                    node: NodeRef::Id { id: "prompt".to_owned() },
                    position: [40.0, 20.0],
                }],
            },
        })
        .expect("revised repair candidate");
    let passed_receipt = review(&state, &revised, ReviewVerdict::Pass);
    let registrations = ReviewedChangeOperations::new(
        Arc::clone(&state.reviewed_change),
        Arc::new(WorkflowPatchService::from_state(&state)),
    )
    .registrations()
    .expect("registrations");
    let apply = &registrations[2];
    let context = RequestContext::new(
        "project",
        "project:project",
        "second-approved-action",
        apply.version(),
        Some(ApprovedEffect::new(apply.id(), apply.version(), "second-approval")),
    );

    let applied = apply
        .dispatch(&context, json!({"review_receipt_id": passed_receipt.id()}))
        .await
        .expect("second approval applies exact repair");
    let run = AssistantRepairService::from_state(&state)
        .execute(&ApprovedWorkflowAction::new("project", passed_receipt.approval_scope_id(), 2))
        .expect("repaired rerun");

    assert_eq!(applied["workflow_head"]["revision"], 2);
    assert_eq!(applied["workflow_head"]["workflow"]["nodes"].as_array().unwrap().len(), 1);
    assert_eq!(applied["workflow_head"]["workflow"]["nodes"][0]["id"], "prompt");
    assert!(matches!(run.outcome, WorkflowRunResultDto::Succeeded { .. }));
    assert!(run.activation.is_none());
}

#[tokio::test]
async fn assistant_repair_approved_apply_runs_and_failure_invokes_the_same_session() {
    let root = tempdir().expect("asset root");
    let activation_path = root.path().join("activation.json");
    let mut state =
        state_at_root(root.path(), Arc::new(MockBackendImpl::always_fails("provider outage")));
    let candidate = state
        .reviewed_change
        .prepare(PrepareCandidateInput {
            project_id: "project".to_owned(),
            session_id: "project:project".to_owned(),
            user_intent: "Position the approved workflow".to_owned(),
            expected_revision: Some(1),
            prior_candidate_id: None,
            patch: WorkflowPatch {
                operations: vec![WorkflowPatchOperation::SetPosition {
                    node: NodeRef::Id { id: "prompt".to_owned() },
                    position: [20.0, 30.0],
                }],
            },
        })
        .expect("approved candidate");
    let receipt = review(&state, &candidate, ReviewVerdict::Pass);
    state.assistant_sidecar_command = repair_activation_sidecar(receipt.id(), &activation_path);
    state.pending_approval.save(&waiting_for(receipt.id(), root.path())).expect("pending approval");
    let pending = assistant_get_pending_approval_with_state("project", &state)
        .expect("pending lookup")
        .expect("pending approval");

    let events = Arc::new(Mutex::new(Vec::<serde_json::Value>::new()));
    let channel = Channel::new({
        let events = Arc::clone(&events);
        move |body: InvokeResponseBody| {
            let InvokeResponseBody::Json(json) = body else { return Ok(()) };
            events.lock().expect("events").push(serde_json::from_str(&json).expect("event JSON"));
            Ok(())
        }
    });
    let head = assistant_decide_approval_with_state(
        AssistantApprovalDecisionInput {
            project_id: "project".to_owned(),
            approval_scope_id: pending.approval_scope_id,
            candidate_digest: pending.candidate_digest,
            approved: true,
        },
        channel,
        &state,
    )
    .await
    .expect("approved apply and lifecycle turn")
    .expect("workflow head");

    assert_eq!(head.revision, 2);
    assert!(state.backend.submitted_task_count() > 0);
    let activation: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&activation_path).expect("captured activation"),
    )
    .expect("activation JSON");
    assert_eq!(activation["kind"], "workflow_run_failed");
    assert_eq!(activation["session_id"], "project:project");
    assert_eq!(activation["workflow_revision"], 2);
    assert!(activation["reason"].as_str().unwrap().contains("provider outage"));
    let captured = events.lock().expect("events");
    let event_types =
        captured.iter().filter_map(|event| event["type"].as_str()).collect::<Vec<_>>();
    assert!(event_types.contains(&"assistant.workflow_run.started"));
    assert!(event_types.contains(&"assistant.workflow_run.progress"));
    assert!(event_types.contains(&"assistant.workflow_run.failed"));
}

fn review(
    state: &AppState,
    candidate: &oh_my_dream_tauri::reviewed_change::WorkflowCandidate,
    verdict: ReviewVerdict,
) -> oh_my_dream_tauri::reviewed_change::ReviewReceipt {
    state
        .reviewed_change
        .record_review(RecordReviewInput {
            project_id: "project".to_owned(),
            session_id: "project:project".to_owned(),
            candidate_id: candidate.id().to_owned(),
            candidate_digest: candidate.digest().to_owned(),
            reviewer_version: "reviewer-v1".to_owned(),
            verdict,
            evidence_hash: format!("sha256:evidence-{}", candidate.id()),
            summary: "review result".to_owned(),
            findings: Vec::new(),
        })
        .expect("review receipt")
}

fn state_with_backend(backend: Arc<MockBackendImpl>) -> AppState {
    let root = tempdir().expect("asset root").keep();
    state_at_root(&root, backend)
}

fn state_at_root(root: &std::path::Path, backend: Arc<MockBackendImpl>) -> AppState {
    let state = AppState::from_asset_root_with_backend(root, backend).expect("app state");
    state
        .store
        .lock()
        .expect("store")
        .create_project_with_id("project", "Project")
        .expect("project");
    let workflow: Workflow = serde_json::from_str(WORKFLOW_JSON).expect("workflow");
    state
        .workflow_authority
        .apply(WorkflowCommitRequest::new(
            "project",
            None,
            "initial-workflow",
            "sha256:initial-workflow",
            workflow,
        ))
        .expect("workflow head");
    state
}

fn waiting_for(receipt_id: &str, root: &std::path::Path) -> AssistantWaitingApproval {
    serde_json::from_value(json!({
        "state": {},
        "pending": {
            "call_id": "call-1",
            "operation_id": "workflow_apply_reviewed_candidate",
            "operation_version": 2,
            "arguments_json": format!("{{\"review_receipt_id\":\"{receipt_id}\"}}")
        },
        "project_id": "project",
        "session_id": "project:project",
        "session_path": root.join("session.sqlite3")
    }))
    .expect("waiting approval")
}

fn repair_activation_sidecar(
    receipt_id: &str,
    activation_path: &std::path::Path,
) -> AssistantSidecarCommand {
    let path = serde_json::to_string(&activation_path.to_string_lossy()).expect("path literal");
    let script = format!(
        r#"import json,sys
invoke=json.loads(sys.stdin.readline())['payload']; inv=invoke['invocation_id']
if invoke.get('input') is not None:
 open({path},'w').write(invoke['input'])
 start=0
else:
 approval=json.loads(sys.stdin.readline())['payload']
 request={{'protocol_version':1,'sequence':0,'kind':'tool_request','payload':{{'invocation_id':inv,'operation_id':'workflow_apply_reviewed_candidate','call_id':'call-1','arguments_json':'{{"review_receipt_id":"{receipt_id}"}}'}}}}
 print(json.dumps(request),flush=True); sys.stdin.readline(); start=1
frames=[('snapshot',{{'invocation_id':inv,'session_id':'project:project','status':'completed','state':None}}),('completed',{{'invocation_id':inv,'final_output':'done'}})]
[print(json.dumps({{'protocol_version':1,'sequence':i+start,'kind':kind,'payload':payload}}),flush=True) for i,(kind,payload) in enumerate(frames)]
sys.stdin.read()"#
    );
    AssistantSidecarCommand::new("python").args(["-c", &script])
}
