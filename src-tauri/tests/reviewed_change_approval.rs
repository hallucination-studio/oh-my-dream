use engine::{CapabilityRef, WorkflowPatch, WorkflowPatchOperation};
use oh_my_dream_tauri::assistant_operations::{ApprovedEffect, RequestContext};
use oh_my_dream_tauri::reviewed_change::{
    PrepareCandidateInput, RecordReviewInput, ReviewVerdict, ReviewedChangeOperations,
};
use oh_my_dream_tauri::state::AppState;
use oh_my_dream_tauri::workflow_authority::{
    WorkflowAuthority, WorkflowAuthorityError, WorkflowCommitRequest, WorkflowCommitResult,
    WorkflowHead, WorkflowRepository,
};
use oh_my_dream_tauri::workflow_patch_operation::WorkflowPatchService;
use serde_json::{Map, json};
use std::sync::{Arc, Mutex};
use tempfile::tempdir;

#[derive(Clone, Copy)]
enum FailurePoint {
    BeforeCommit,
    AfterCommit,
}

struct FaultInjectingRepository {
    failure: FailurePoint,
    state: Mutex<RepositoryState>,
}

#[derive(Default)]
struct RepositoryState {
    failed_once: bool,
    head: Option<WorkflowHead>,
    receipt: Option<(String, String, WorkflowCommitResult)>,
}

impl FaultInjectingRepository {
    fn new(failure: FailurePoint) -> Self {
        Self { failure, state: Mutex::new(RepositoryState::default()) }
    }
}

impl WorkflowRepository for FaultInjectingRepository {
    fn load_head(&self, _project_id: &str) -> Result<Option<WorkflowHead>, WorkflowAuthorityError> {
        Ok(self.state.lock().expect("repository state").head.clone())
    }

    fn load_receipt(
        &self,
        _project_id: &str,
        request_id: &str,
        request_hash: &str,
    ) -> Result<Option<WorkflowCommitResult>, WorkflowAuthorityError> {
        let state = self.state.lock().expect("repository state");
        Ok(state.receipt.as_ref().and_then(|(stored_id, stored_hash, result)| {
            (stored_id == request_id && stored_hash == request_hash).then(|| result.clone())
        }))
    }

    fn commit(
        &self,
        request: &WorkflowCommitRequest,
    ) -> Result<WorkflowCommitResult, WorkflowAuthorityError> {
        let mut state = self.state.lock().expect("repository state");
        if !state.failed_once && matches!(self.failure, FailurePoint::BeforeCommit) {
            state.failed_once = true;
            return Err(injected_failure());
        }
        if let Some((stored_id, stored_hash, result)) = &state.receipt
            && stored_id == &request.request_id
            && stored_hash == &request.request_hash
        {
            let mut replay = result.clone();
            replay.deduplicated = true;
            return Ok(replay);
        }
        let result = committed_result(request);
        state.head = result.head.clone();
        state.receipt =
            Some((request.request_id.clone(), request.request_hash.clone(), result.clone()));
        if !state.failed_once && matches!(self.failure, FailurePoint::AfterCommit) {
            state.failed_once = true;
            return Err(injected_failure());
        }
        Ok(result)
    }
}

#[tokio::test]
async fn crash_before_canonical_commit_leaves_no_head_and_retry_applies_once() {
    let fixture = ApprovalFixture::new(FailurePoint::BeforeCommit);

    fixture.first_dispatch_fails().await;
    assert!(fixture.authority.load_head("project").expect("head lookup").is_none());

    let retry = fixture.dispatch().await.expect("retry applies candidate");
    assert_eq!(retry["workflow_head"]["revision"], 1);
    assert_eq!(retry["deduplicated"], false);
}

#[tokio::test]
async fn crash_after_canonical_commit_is_recovered_from_the_exact_receipt() {
    let fixture = ApprovalFixture::new(FailurePoint::AfterCommit);

    fixture.first_dispatch_fails().await;
    assert_eq!(
        fixture
            .authority
            .load_head("project")
            .expect("head lookup")
            .expect("committed head")
            .revision,
        1
    );

    let retry = fixture.dispatch().await.expect("retry reads committed receipt");
    assert_eq!(retry["workflow_head"]["revision"], 1);
    assert_eq!(retry["deduplicated"], true);
}

struct ApprovalFixture {
    authority: Arc<WorkflowAuthority>,
    apply: oh_my_dream_tauri::assistant_operations::OperationRegistration,
    context: RequestContext,
    receipt_id: String,
}

impl ApprovalFixture {
    fn new(failure: FailurePoint) -> Self {
        let root = tempdir().expect("app root");
        let state = AppState::from_asset_root(root.path()).expect("app state");
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
                summary: "reviewed".to_owned(),
                findings: Vec::new(),
            })
            .expect("receipt");
        let authority =
            Arc::new(WorkflowAuthority::from_repository(FaultInjectingRepository::new(failure)));
        let patch_service = Arc::new(WorkflowPatchService::new(
            Arc::clone(&state.registry),
            Arc::clone(&authority),
            Arc::clone(&state.store),
        ));
        let apply =
            ReviewedChangeOperations::new(Arc::clone(&state.reviewed_change), patch_service)
                .registrations()
                .expect("registrations")
                .remove(2);
        let context = RequestContext::new(
            "project",
            "session",
            "stable-request",
            apply.version(),
            Some(ApprovedEffect::new(apply.id(), apply.version(), "approval-call")),
        );
        Self { authority, apply, context, receipt_id: receipt.id().to_owned() }
    }

    async fn dispatch(
        &self,
    ) -> Result<serde_json::Value, oh_my_dream_tauri::assistant_operations::OperationDispatchError>
    {
        self.apply.dispatch(&self.context, json!({"review_receipt_id": self.receipt_id})).await
    }

    async fn first_dispatch_fails(&self) {
        let error = self.dispatch().await.expect_err("injected crash response");
        assert!(error.to_string().contains("injected commit failure"));
    }
}

fn committed_result(request: &WorkflowCommitRequest) -> WorkflowCommitResult {
    WorkflowCommitResult {
        head: Some(WorkflowHead {
            project_id: request.project_id.clone(),
            revision: 1,
            workflow: request.workflow.clone(),
        }),
        changed: true,
        deduplicated: false,
        undo_id: Some("workflow:project:1".to_owned()),
    }
}

fn injected_failure() -> WorkflowAuthorityError {
    WorkflowAuthorityError::Storage { message: "injected commit failure".to_owned() }
}
