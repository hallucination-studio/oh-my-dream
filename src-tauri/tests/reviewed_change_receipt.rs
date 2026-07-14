use engine::{CapabilityRef, WorkflowPatch, WorkflowPatchOperation};
use oh_my_dream_tauri::reviewed_change::{
    PrepareCandidateInput, RecordReviewInput, ReviewVerdict, ReviewedChangeError,
};
use oh_my_dream_tauri::state::AppState;
use serde_json::Map;
use tempfile::tempdir;

#[test]
fn rust_persists_only_a_review_bound_to_the_exact_candidate_digest() {
    let root = tempdir().expect("root");
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
            user_intent: "Build the requested production".to_owned(),
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

    let mismatch = state.reviewed_change.record_review(RecordReviewInput {
        project_id: "project".to_owned(),
        session_id: "session".to_owned(),
        candidate_id: candidate.id().to_owned(),
        candidate_digest: "sha256:forged".to_owned(),
        reviewer_version: "reviewer-v1".to_owned(),
        verdict: ReviewVerdict::Pass,
        evidence_hash: "sha256:evidence".to_owned(),
    });
    assert!(matches!(mismatch, Err(ReviewedChangeError::CandidateScopeMismatch)));

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

    assert_eq!(receipt.candidate_id(), candidate.id());
    assert_eq!(receipt.verdict(), ReviewVerdict::Pass);
    assert!(!receipt.approval_scope_id().is_empty());
    assert_eq!(state.reviewed_change.get_receipt(receipt.id()).expect("get"), Some(receipt));
}
