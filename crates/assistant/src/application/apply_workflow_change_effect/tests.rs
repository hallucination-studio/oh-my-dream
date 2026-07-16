use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

use async_trait::async_trait;
use projects::project::domain::ProjectId;
use uuid::Uuid;

use super::*;
use crate::{application::AssistantApplyWorkflowChangeEffect, domain::*, interfaces::*};

#[derive(Clone)]
struct RepositoryFakeImpl(Arc<Mutex<AssistantWorkflowChangeAggregate>>);

#[async_trait]
impl AssistantWorkflowChangeRepositoryInterface for RepositoryFakeImpl {
    async fn load_assistant_workflow_change(
        &self,
        change_id: AssistantWorkflowChangeId,
    ) -> Result<Option<AssistantWorkflowChangeAggregate>, AssistantApplicationError> {
        let value = self.0.lock().unwrap();
        Ok((value.id() == change_id).then(|| value.clone()))
    }
    async fn load_pending_assistant_workflow_change(
        &self,
        _project_id: ProjectId,
        _session_id: AssistantSessionId,
    ) -> Result<Option<AssistantWorkflowChangeAggregate>, AssistantApplicationError> {
        Ok(None)
    }
    async fn insert_assistant_workflow_change(
        &self,
        _change: AssistantWorkflowChangeAggregate,
    ) -> Result<(), AssistantApplicationError> {
        Err(AssistantApplicationError::InvalidTransition)
    }
    async fn commit_assistant_workflow_change_transition(
        &self,
        expected_state: AssistantWorkflowChangeState,
        change: AssistantWorkflowChangeAggregate,
    ) -> Result<(), AssistantApplicationError> {
        let mut value = self.0.lock().unwrap();
        if value.state() != expected_state {
            return Err(AssistantApplicationError::InvalidTransition);
        }
        *value = change;
        Ok(())
    }
    async fn commit_assistant_workflow_change_apply_decision(
        &self,
        _expected_state: AssistantWorkflowChangeState,
        _change: AssistantWorkflowChangeAggregate,
        _effect: AssistantApplyWorkflowChangeEffect,
    ) -> Result<(), AssistantApplicationError> {
        Err(AssistantApplicationError::InvalidTransition)
    }
}

#[derive(Clone)]
struct ApplierFakeImpl(Arc<AtomicUsize>);

#[async_trait]
impl AssistantWorkflowMutationApplierInterface for ApplierFakeImpl {
    async fn apply_assistant_workflow_change(
        &self,
        _request: AssistantWorkflowApplyRequest,
    ) -> Result<AssistantWorkflowApplyReceiptBoundaryValue, AssistantApplicationError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        AssistantWorkflowApplyReceiptBoundaryValue::new(vec![1])
            .map_err(|_| AssistantApplicationError::ExternalBoundaryFailed)
    }
}

#[derive(Clone)]
struct ContinuationStoreFakeImpl(Arc<Mutex<Option<AssistantStoredContinuation>>>);

#[async_trait]
impl AssistantModelContinuationStoreInterface for ContinuationStoreFakeImpl {
    async fn store_assistant_model_continuation(
        &self,
        continuation: AssistantStoredContinuation,
    ) -> Result<(), AssistantApplicationError> {
        *self.0.lock().unwrap() = Some(continuation);
        Ok(())
    }
    async fn load_assistant_model_continuation(
        &self,
        _continuation_ref: &AssistantModelContinuationRef,
    ) -> Result<Option<AssistantStoredContinuation>, AssistantApplicationError> {
        Ok(self.0.lock().unwrap().clone())
    }
    async fn consume_assistant_model_continuation(
        &self,
        _continuation_ref: &AssistantModelContinuationRef,
    ) -> Result<Option<AssistantStoredContinuation>, AssistantApplicationError> {
        Ok(self.0.lock().unwrap().take())
    }
}

#[derive(Clone)]
struct RunnerFakeImpl(Arc<AtomicUsize>);

#[async_trait]
impl AssistantModelRunnerInterface for RunnerFakeImpl {
    async fn start_assistant_model_turn(
        &self,
        _request: AssistantModelTurnRequest,
    ) -> Result<AssistantModelTurnResult, AssistantApplicationError> {
        Err(AssistantApplicationError::ProtocolViolation)
    }
    async fn resume_assistant_model_turn(
        &self,
        _request: AssistantModelResumeRequest,
    ) -> Result<AssistantModelTurnResult, AssistantApplicationError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        AssistantModelTurnResult::new(vec![1])
    }
}

#[derive(Clone)]
struct RunStarterFakeImpl(Arc<AtomicUsize>);

#[async_trait]
impl AssistantWorkflowRunStarterInterface for RunStarterFakeImpl {
    async fn start_assistant_workflow_run(
        &self,
        _request: AssistantWorkflowRunRequest,
    ) -> Result<AssistantWorkflowRunBoundaryValue, AssistantApplicationError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        AssistantWorkflowRunBoundaryValue::new(vec![2])
            .map_err(|_| AssistantApplicationError::ExternalBoundaryFailed)
    }
}

#[tokio::test]
async fn effect_recovers_apply_resume_and_run_link_without_repeating_completed_work() {
    let change = applying_change();
    let change_id = change.id();
    let repository = RepositoryFakeImpl(Arc::new(Mutex::new(change.clone())));
    let apply_count = Arc::new(AtomicUsize::new(0));
    let resume_count = Arc::new(AtomicUsize::new(0));
    let run_count = Arc::new(AtomicUsize::new(0));
    let continuation = AssistantStoredContinuation {
        continuation_ref: change.continuation_ref().unwrap().clone(),
        project_id: change.project_id(),
        session_id: change.session_id(),
        invocation_id: invocation_id(8),
        envelope: AssistantModelContinuationEnvelope::new(vec![1]).unwrap(),
    };
    let use_case = AssistantApplyWorkflowChangeEffectUseCase::new(
        repository,
        ApplierFakeImpl(Arc::clone(&apply_count)),
        ContinuationStoreFakeImpl(Arc::new(Mutex::new(Some(continuation)))),
        RunnerFakeImpl(Arc::clone(&resume_count)),
        RunStarterFakeImpl(Arc::clone(&run_count)),
    );

    let completed = use_case.apply_workflow_change_effect(change_id).await.unwrap();
    assert!(completed.applied_workflow_receipt().is_some());
    assert!(completed.admitted_workflow_run().is_some());
    assert_eq!(completed.continuation_outcome(), AssistantContinuationOutcome::Resumed);
    let replayed = use_case.apply_workflow_change_effect(change_id).await.unwrap();
    assert_eq!(replayed, completed);
    assert_eq!(apply_count.load(Ordering::SeqCst), 1);
    assert_eq!(resume_count.load(Ordering::SeqCst), 1);
    assert_eq!(run_count.load(Ordering::SeqCst), 1);
}

fn applying_change() -> AssistantWorkflowChangeAggregate {
    let mut change = proposed_change();
    let receipt = AssistantReviewReceipt::new(
        change.id(),
        change.mutation_digest(),
        AssistantContractEpoch::new(1).unwrap(),
        AssistantModelIdentity::new("reviewer@1").unwrap(),
        invocation_id(6),
        AssistantToolCallId::new("call_1").unwrap(),
        AssistantReviewVerdict::Pass,
        AssistantReviewedAt::new(10).unwrap(),
    );
    change
        .accept_review(receipt, AssistantModelContinuationRef::new("continuation").unwrap())
        .unwrap();
    change.begin_apply(decision_scope(&change), 20).unwrap();
    change
}

fn decision_scope(
    change: &AssistantWorkflowChangeAggregate,
) -> AssistantWorkflowChangeDecisionScope {
    AssistantWorkflowChangeDecisionScope {
        project_id: change.project_id(),
        session_id: change.session_id(),
        change_id: change.id(),
        approval_scope_id: change.approval_scope_id(),
        mutation_digest: change.mutation_digest(),
    }
}

fn proposed_change() -> AssistantWorkflowChangeAggregate {
    AssistantWorkflowChangeAggregate::new(AssistantWorkflowChangeCandidate {
        id: AssistantWorkflowChangeId::from_uuid(uuid(3)).unwrap(),
        project_id: ProjectId::from_uuid(uuid(1)).unwrap(),
        session_id: AssistantSessionId::from_uuid(uuid(2)).unwrap(),
        base_workflow_revision: WorkflowRevisionBoundaryValue::new(1).unwrap(),
        ordered_mutations: vec![AssistantWorkflowMutation::new(vec![1]).unwrap()],
        stable_aliases: AssistantWorkflowStableAliasSet::default(),
        readiness_issues: vec![],
        mutation_digest: AssistantWorkflowMutationDigest::new([7; 32]),
        resulting_workflow_fingerprint: AssistantWorkflowFingerprint::new([8; 32]),
        lineage: AssistantWorkflowChangeLineage::UserMessage {
            invocation_id: invocation_id(4),
            intent: AssistantUserIntent::new("Create a scene").unwrap(),
        },
        approval_scope_id: AssistantApprovalScopeId::from_uuid(uuid(5)).unwrap(),
        expires_at: AssistantWorkflowChangeExpiry::new(30_000).unwrap(),
    })
    .unwrap()
}

fn invocation_id(seed: u8) -> AssistantModelInvocationId {
    AssistantModelInvocationId::from_uuid(uuid(seed)).unwrap()
}
fn uuid(seed: u8) -> Uuid {
    Uuid::from_bytes([seed, 0, 0, 0, 0, 0, 0x40, 0, 0x80, 0, 0, 0, 0, 0, 0, seed])
}
