use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

use async_trait::async_trait;
use projects::project::domain::ProjectId;
use uuid::Uuid;

use super::*;
use crate::{domain::*, interfaces::*};

#[derive(Clone)]
struct RepositoryFakeImpl {
    value: Arc<Mutex<AssistantWorkflowChangeAggregate>>,
    effect_committed: Arc<AtomicBool>,
}

#[async_trait]
impl AssistantWorkflowChangeRepositoryInterface for RepositoryFakeImpl {
    async fn load_assistant_workflow_change(
        &self,
        change_id: AssistantWorkflowChangeId,
    ) -> Result<Option<AssistantWorkflowChangeAggregate>, AssistantApplicationError> {
        let value = self.value.lock().unwrap();
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
        self.replace(expected_state, change)
    }
    async fn commit_assistant_workflow_change_apply_decision(
        &self,
        expected_state: AssistantWorkflowChangeState,
        change: AssistantWorkflowChangeAggregate,
        effect: AssistantApplyWorkflowChangeEffect,
    ) -> Result<(), AssistantApplicationError> {
        if effect.workflow_change_id() != change.id() {
            return Err(AssistantApplicationError::InvalidTransition);
        }
        self.replace(expected_state, change)?;
        self.effect_committed.store(true, Ordering::SeqCst);
        Ok(())
    }
}

impl RepositoryFakeImpl {
    fn replace(
        &self,
        expected_state: AssistantWorkflowChangeState,
        change: AssistantWorkflowChangeAggregate,
    ) -> Result<(), AssistantApplicationError> {
        let mut value = self.value.lock().unwrap();
        if value.state() != expected_state {
            return Err(AssistantApplicationError::InvalidTransition);
        }
        *value = change;
        Ok(())
    }
}

#[derive(Clone)]
struct ContinuationStoreFakeImpl {
    repository: RepositoryFakeImpl,
    consumed_after_terminal_commit: Arc<AtomicBool>,
}

#[async_trait]
impl AssistantModelContinuationStoreInterface for ContinuationStoreFakeImpl {
    async fn store_assistant_model_continuation(
        &self,
        _continuation: AssistantStoredContinuation,
    ) -> Result<(), AssistantApplicationError> {
        Ok(())
    }
    async fn load_assistant_model_continuation(
        &self,
        _continuation_ref: &AssistantModelContinuationRef,
    ) -> Result<Option<AssistantStoredContinuation>, AssistantApplicationError> {
        Ok(None)
    }
    async fn consume_assistant_model_continuation(
        &self,
        _continuation_ref: &AssistantModelContinuationRef,
    ) -> Result<Option<AssistantStoredContinuation>, AssistantApplicationError> {
        let terminal =
            self.repository.value.lock().unwrap().state() == AssistantWorkflowChangeState::Rejected;
        self.consumed_after_terminal_commit.store(terminal, Ordering::SeqCst);
        Ok(None)
    }
}

#[tokio::test]
async fn approval_atomically_commits_applying_with_effect() {
    let change = awaiting_change();
    let repository = repository(change.clone());
    let consumed = Arc::new(AtomicBool::new(false));
    let use_case = AssistantDecideWorkflowChangeUseCase::new(
        repository.clone(),
        ContinuationStoreFakeImpl {
            repository: repository.clone(),
            consumed_after_terminal_commit: consumed,
        },
    );
    let result = use_case
        .decide_workflow_change(command(&change, AssistantWorkflowChangeDecision::Approve))
        .await
        .unwrap();
    assert_eq!(result.state(), AssistantWorkflowChangeState::Applying);
    assert!(repository.effect_committed.load(Ordering::SeqCst));
}

#[tokio::test]
async fn rejection_consumes_continuation_only_after_terminal_commit() {
    let change = awaiting_change();
    let repository = repository(change.clone());
    let consumed = Arc::new(AtomicBool::new(false));
    let use_case = AssistantDecideWorkflowChangeUseCase::new(
        repository.clone(),
        ContinuationStoreFakeImpl {
            repository,
            consumed_after_terminal_commit: Arc::clone(&consumed),
        },
    );
    let result = use_case
        .decide_workflow_change(command(&change, AssistantWorkflowChangeDecision::Reject))
        .await
        .unwrap();
    assert_eq!(result.state(), AssistantWorkflowChangeState::Rejected);
    assert!(consumed.load(Ordering::SeqCst));
}

fn repository(change: AssistantWorkflowChangeAggregate) -> RepositoryFakeImpl {
    RepositoryFakeImpl {
        value: Arc::new(Mutex::new(change)),
        effect_committed: Arc::new(AtomicBool::new(false)),
    }
}

fn command(
    change: &AssistantWorkflowChangeAggregate,
    decision: AssistantWorkflowChangeDecision,
) -> AssistantDecideWorkflowChangeCommand {
    AssistantDecideWorkflowChangeCommand {
        workflow_change_id: change.id(),
        scope: AssistantWorkflowChangeDecisionScope {
            project_id: change.project_id(),
            session_id: change.session_id(),
            change_id: change.id(),
            approval_scope_id: change.approval_scope_id(),
            mutation_digest: change.mutation_digest(),
        },
        decision,
        now_epoch_ms: 20,
    }
}

fn awaiting_change() -> AssistantWorkflowChangeAggregate {
    let mut change = AssistantWorkflowChangeAggregate::new(AssistantWorkflowChangeCandidate {
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
    .unwrap();
    change
        .accept_review(
            AssistantReviewReceipt::new(
                change.id(),
                change.mutation_digest(),
                AssistantContractEpoch::new(1).unwrap(),
                AssistantModelIdentity::new("reviewer@1").unwrap(),
                invocation_id(6),
                AssistantToolCallId::new("call_1").unwrap(),
                AssistantReviewVerdict::Pass,
                AssistantReviewedAt::new(10).unwrap(),
            ),
            AssistantModelContinuationRef::new("continuation").unwrap(),
        )
        .unwrap();
    change
}

fn invocation_id(seed: u8) -> AssistantModelInvocationId {
    AssistantModelInvocationId::from_uuid(uuid(seed)).unwrap()
}
fn uuid(seed: u8) -> Uuid {
    Uuid::from_bytes([seed, 0, 0, 0, 0, 0, 0x40, 0, 0x80, 0, 0, 0, 0, 0, 0, seed])
}
