use engine::{CapabilityRef, NodeRef, WorkflowPatch, WorkflowPatchOperation};
use oh_my_dream_tauri::assistant_operations::{ApprovedEffect, RequestContext};
use oh_my_dream_tauri::reviewed_change::{
    PrepareCandidateInput, RecordReviewInput, ReviewVerdict, ReviewedChangeOperations,
};
use oh_my_dream_tauri::state::AppState;
use oh_my_dream_tauri::workflow_patch_operation::WorkflowPatchService;
use serde_json::{Map, json};
use std::sync::Arc;
use tempfile::tempdir;

fn add(alias: &str, capability: &str) -> WorkflowPatch {
    WorkflowPatch {
        operations: vec![WorkflowPatchOperation::AddNode {
            alias: alias.to_owned(),
            capability: CapabilityRef::new(capability, "1.0"),
            params: Map::new(),
            position: None,
        }],
    }
}

#[test]
fn candidate_extension_is_immutable_and_does_not_mutate_workflow_authority() {
    let root = tempdir().expect("app root");
    let state = AppState::from_asset_root(root.path()).expect("app state");
    state
        .store
        .lock()
        .expect("store")
        .create_project_with_id("project", "Project")
        .expect("project");

    let first = state
        .reviewed_change
        .prepare(PrepareCandidateInput {
            project_id: "project".to_owned(),
            session_id: "session".to_owned(),
            expected_revision: None,
            prior_candidate_id: None,
            patch: add("prompt", "TextPrompt"),
        })
        .expect("first candidate");
    let prompt_id = first.workflow().nodes[0].id.clone();
    let second = state
        .reviewed_change
        .prepare(PrepareCandidateInput {
            project_id: "project".to_owned(),
            session_id: "session".to_owned(),
            expected_revision: None,
            prior_candidate_id: Some(first.id().to_owned()),
            patch: WorkflowPatch {
                operations: vec![WorkflowPatchOperation::SetPosition {
                    node: NodeRef::Id { id: prompt_id },
                    position: [20.0, 30.0],
                }],
            },
        })
        .expect("extended candidate");

    assert_ne!(first.id(), second.id());
    assert_eq!(first.patches().len(), 1);
    assert_eq!(second.patches().len(), 2);
    assert_eq!(first.workflow().nodes[0].position, None);
    assert_eq!(second.workflow().nodes[0].position, Some([20.0, 30.0]));
    assert!(state.workflow_authority.load_head("project").expect("head").is_none());
    assert_eq!(state.reviewed_change.get(first.id()).expect("get").expect("first"), first);
}

#[test]
fn production_assistant_has_candidate_tools_without_direct_workflow_mutation() {
    let root = tempdir().expect("app root");
    let state = AppState::from_asset_root(root.path()).expect("app state");
    let ids = oh_my_dream_tauri::assistant_commands::production_operation_ids(&state)
        .expect("operation ids");

    assert!(ids.contains(&"workflow_prepare_patch".to_owned()));
    assert!(ids.contains(&"workflow_candidate_get".to_owned()));
    assert!(!ids.contains(&"workflow_apply_patch".to_owned()));
}

#[test]
fn candidate_survives_a_fresh_application_state() {
    let root = tempdir().expect("app root");
    let candidate_id = {
        let state = AppState::from_asset_root(root.path()).expect("first state");
        state
            .store
            .lock()
            .expect("store")
            .create_project_with_id("project", "Project")
            .expect("project");
        state
            .reviewed_change
            .prepare(PrepareCandidateInput {
                project_id: "project".to_owned(),
                session_id: "session".to_owned(),
                expected_revision: None,
                prior_candidate_id: None,
                patch: add("prompt", "TextPrompt"),
            })
            .expect("candidate")
            .id()
            .to_owned()
    };

    let reopened = AppState::from_asset_root(root.path()).expect("reopened state");
    let candidate = reopened
        .reviewed_change
        .get(&candidate_id)
        .expect("read candidate")
        .expect("persisted candidate");

    assert_eq!(candidate.id(), candidate_id);
    assert_eq!(candidate.patches().len(), 1);
}

#[tokio::test]
async fn approved_candidate_replays_exact_patches_into_one_workflow_revision() {
    let root = tempdir().expect("app root");
    let state = AppState::from_asset_root(root.path()).expect("state");
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
            session_id: "session".to_owned(),
            expected_revision: None,
            prior_candidate_id: None,
            patch: add("prompt", "TextPrompt"),
        })
        .expect("candidate");
    let receipt = state
        .reviewed_change
        .record_review(RecordReviewInput {
            project_id: "project".to_owned(),
            session_id: "session".to_owned(),
            candidate_id: candidate.id().to_owned(),
            candidate_digest: candidate.digest().to_owned(),
            reviewer_version: "reviewer-v1".to_owned(),
            verdict: ReviewVerdict::Pass,
            evidence_hash: "sha256:evidence".to_owned(),
        })
        .expect("receipt");
    let registrations = ReviewedChangeOperations::new(
        Arc::clone(&state.reviewed_change),
        Arc::new(WorkflowPatchService::from_state(&state)),
    )
    .registrations()
    .expect("registrations");
    let apply = &registrations[2];
    let context = RequestContext::new(
        "project",
        "session",
        "transport-request",
        apply.version(),
        Some(ApprovedEffect::new(apply.id(), apply.version(), "approval-call")),
    );

    let first = apply
        .dispatch(&context, json!({"review_receipt_id": receipt.id()}))
        .await
        .expect("approved apply");
    let replay = apply
        .dispatch(&context, json!({"review_receipt_id": receipt.id()}))
        .await
        .expect("idempotent replay");

    assert_eq!(first["workflow_head"]["revision"], 1);
    assert_eq!(first["workflow_head"]["workflow"]["nodes"].as_array().unwrap().len(), 1);
    assert_eq!(replay["deduplicated"], true);
}
