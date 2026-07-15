use backends::MockBackend;
use engine::{CapabilityRef, InputBinding, NodeRef, WorkflowPatch, WorkflowPatchOperation};
use oh_my_dream_tauri::assistant_operations::{ApprovedEffect, RequestContext};
use oh_my_dream_tauri::assistant_repair::{ApprovedWorkflowAction, AssistantRepairService};
use oh_my_dream_tauri::production_plan::NewPlanItem;
use oh_my_dream_tauri::reviewed_change::{
    PrepareCandidateInput, RecordReviewInput, ReviewReceipt, ReviewVerdict,
    ReviewedChangeOperations, WorkflowCandidate,
};
use oh_my_dream_tauri::state::AppState;
use oh_my_dream_tauri::workflow_patch_operation::WorkflowPatchService;
use oh_my_dream_tauri::workflow_run_dto::WorkflowRunResultDto;
use serde_json::{Map, json};
use std::sync::Arc;
use tempfile::tempdir;

#[tokio::test]
async fn assistant_strong_mvp_e2e_builds_fails_repairs_and_succeeds() {
    let root = tempdir().expect("root");
    let state = AppState::from_asset_root_with_backend(
        root.path(),
        Arc::new(MockBackend::always_fails("provider outage")),
    )
    .expect("state");
    state
        .store
        .lock()
        .expect("store")
        .create_project_with_id("project", "Project")
        .expect("project");
    let plan = state
        .production_plan
        .create("project", "Two-shot film".to_owned(), vec![item("shot-1"), item("shot-2")])
        .expect("plan");
    let plan = state.production_plan.start_item("project", plan.revision(), "shot-1").unwrap();
    let plan = state
        .production_plan
        .complete_item("project", plan.revision(), "shot-1", "built".to_owned())
        .unwrap();
    let plan = state.production_plan.start_item("project", plan.revision(), "shot-2").unwrap();
    state
        .production_plan
        .complete_item("project", plan.revision(), "shot-2", "built".to_owned())
        .unwrap();

    let first = prepare(&state, None, None, add_prompt());
    let first_rejection = review(&state, &first, ReviewVerdict::Reject);
    assert!(!is_passed(&state, &first_rejection));
    let prompt_id = first.workflow().nodes[0].id.clone();
    let built = prepare(&state, None, Some(first.id()), add_image(&prompt_id));
    let first_pass = review(&state, &built, ReviewVerdict::Pass);
    let first_apply = apply(&state, &first_pass, "first-approved-action").await;
    assert_eq!(first_apply["workflow_head"]["revision"], 1);

    let failed = AssistantRepairService::from_state(&state)
        .execute(&ApprovedWorkflowAction::new("project", first_pass.approval_scope_id(), 1))
        .expect("failed run");
    assert!(matches!(failed.outcome, WorkflowRunResultDto::Failed { .. }));
    assert_eq!(failed.activation.expect("activation").session_id, "project:project");

    let image_id = built
        .workflow()
        .nodes
        .iter()
        .find(|node| node.type_id == "Image" && node.params["mode"] == "text")
        .expect("image node")
        .id
        .clone();
    let repair = prepare(&state, Some(1), None, remove_image(&image_id));
    let repair_rejection = review(&state, &repair, ReviewVerdict::Reject);
    assert!(!is_passed(&state, &repair_rejection));
    let revised = prepare(&state, Some(1), Some(repair.id()), position_prompt(&prompt_id));
    let repair_pass = review(&state, &revised, ReviewVerdict::Pass);
    let repaired = apply(&state, &repair_pass, "second-approved-action").await;
    assert_eq!(repaired["workflow_head"]["revision"], 2);
    assert_eq!(repaired["workflow_head"]["workflow"]["nodes"].as_array().unwrap().len(), 1);

    let succeeded = AssistantRepairService::from_state(&state)
        .execute(&ApprovedWorkflowAction::new("project", repair_pass.approval_scope_id(), 2))
        .expect("successful rerun");
    assert!(matches!(succeeded.outcome, WorkflowRunResultDto::Succeeded { .. }));
}

fn item(id: &str) -> NewPlanItem {
    NewPlanItem { id: id.to_owned(), summary: format!("Build {id}") }
}

fn prepare(
    state: &AppState,
    expected_revision: Option<u64>,
    prior: Option<&str>,
    patch: WorkflowPatch,
) -> WorkflowCandidate {
    state
        .reviewed_change
        .prepare(PrepareCandidateInput {
            project_id: "project".to_owned(),
            session_id: "project:project".to_owned(),
            user_intent: "Build and repair the film".to_owned(),
            expected_revision,
            prior_candidate_id: prior.map(str::to_owned),
            patch,
        })
        .expect("candidate")
}

fn review(
    state: &AppState,
    candidate: &WorkflowCandidate,
    verdict: ReviewVerdict,
) -> ReviewReceipt {
    state
        .reviewed_change
        .record_review(RecordReviewInput {
            project_id: "project".to_owned(),
            session_id: "project:project".to_owned(),
            candidate_id: candidate.id().to_owned(),
            candidate_digest: candidate.digest().to_owned(),
            reviewer_version: "reviewer-v1".to_owned(),
            verdict,
            evidence_hash: format!("sha256:{}", candidate.id()),
            summary: "reviewed".to_owned(),
            findings: Vec::new(),
        })
        .expect("review")
}

fn is_passed(state: &AppState, receipt: &ReviewReceipt) -> bool {
    state
        .reviewed_change
        .valid_passed_receipt("project", "project:project", receipt.id())
        .expect("receipt check")
}

async fn apply(state: &AppState, receipt: &ReviewReceipt, request_id: &str) -> serde_json::Value {
    let registrations = ReviewedChangeOperations::new(
        Arc::clone(&state.reviewed_change),
        Arc::new(WorkflowPatchService::from_state(state)),
    )
    .registrations()
    .expect("registrations");
    let operation = &registrations[2];
    operation
        .dispatch(
            &RequestContext::new(
                "project",
                "project:project",
                request_id,
                operation.version(),
                Some(ApprovedEffect::new(operation.id(), operation.version(), request_id)),
            ),
            json!({"review_receipt_id": receipt.id()}),
        )
        .await
        .expect("approved apply")
}

fn add_prompt() -> WorkflowPatch {
    WorkflowPatch {
        operations: vec![WorkflowPatchOperation::AddNode {
            alias: "prompt".to_owned(),
            capability: CapabilityRef::new("TextPrompt", "1.0"),
            params: Map::new(),
            position: None,
        }],
    }
}

fn add_image(prompt_id: &str) -> WorkflowPatch {
    WorkflowPatch {
        operations: vec![
            WorkflowPatchOperation::AddNode {
                alias: "image".to_owned(),
                capability: CapabilityRef::new("TextToImage", "1.0"),
                params: Map::new(),
                position: None,
            },
            WorkflowPatchOperation::SetInput {
                node: NodeRef::Alias { alias: "image".to_owned() },
                input: "prompt".to_owned(),
                binding: InputBinding::Single { source: NodeRef::Id { id: prompt_id.to_owned() } },
            },
        ],
    }
}

fn remove_image(image_id: &str) -> WorkflowPatch {
    WorkflowPatch {
        operations: vec![WorkflowPatchOperation::RemoveNode {
            node: NodeRef::Id { id: image_id.to_owned() },
        }],
    }
}

fn position_prompt(prompt_id: &str) -> WorkflowPatch {
    WorkflowPatch {
        operations: vec![WorkflowPatchOperation::SetPosition {
            node: NodeRef::Id { id: prompt_id.to_owned() },
            position: [20.0, 30.0],
        }],
    }
}
