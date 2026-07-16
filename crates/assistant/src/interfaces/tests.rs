use std::{collections::BTreeMap, sync::Mutex};

use async_trait::async_trait;
use projects::project::domain::ProjectId;
use uuid::Uuid;

use super::*;
use crate::domain::*;

#[derive(Default)]
struct ContinuationStoreFake {
    values: Mutex<BTreeMap<String, AssistantStoredContinuation>>,
}

#[async_trait]
impl AssistantModelContinuationStoreInterface for ContinuationStoreFake {
    async fn store_assistant_model_continuation(
        &self,
        continuation: AssistantStoredContinuation,
    ) -> Result<(), AssistantApplicationError> {
        self.values
            .lock()
            .unwrap()
            .insert(continuation.continuation_ref.as_str().to_owned(), continuation);
        Ok(())
    }

    async fn load_assistant_model_continuation(
        &self,
        continuation_ref: &AssistantModelContinuationRef,
    ) -> Result<Option<AssistantStoredContinuation>, AssistantApplicationError> {
        Ok(self.values.lock().unwrap().get(continuation_ref.as_str()).cloned())
    }

    async fn consume_assistant_model_continuation(
        &self,
        continuation_ref: &AssistantModelContinuationRef,
    ) -> Result<Option<AssistantStoredContinuation>, AssistantApplicationError> {
        Ok(self.values.lock().unwrap().remove(continuation_ref.as_str()))
    }
}

#[derive(Default)]
struct ChangeRepositoryFake {
    values: Mutex<BTreeMap<AssistantWorkflowChangeId, AssistantWorkflowChangeAggregate>>,
}

#[async_trait]
impl AssistantWorkflowChangeRepositoryInterface for ChangeRepositoryFake {
    async fn load_assistant_workflow_change(
        &self,
        change_id: AssistantWorkflowChangeId,
    ) -> Result<Option<AssistantWorkflowChangeAggregate>, AssistantApplicationError> {
        Ok(self.values.lock().unwrap().get(&change_id).cloned())
    }

    async fn load_pending_assistant_workflow_change(
        &self,
        project_id: ProjectId,
        session_id: AssistantSessionId,
    ) -> Result<Option<AssistantWorkflowChangeAggregate>, AssistantApplicationError> {
        Ok(self
            .values
            .lock()
            .unwrap()
            .values()
            .find(|change| {
                change.project_id() == project_id
                    && change.session_id() == session_id
                    && change.state() == AssistantWorkflowChangeState::AwaitingApproval
            })
            .cloned())
    }

    async fn insert_assistant_workflow_change(
        &self,
        change: AssistantWorkflowChangeAggregate,
    ) -> Result<(), AssistantApplicationError> {
        let mut values = self.values.lock().unwrap();
        if values.values().any(|stored| {
            stored.project_id() == change.project_id()
                && stored.session_id() == change.session_id()
                && stored.state() == AssistantWorkflowChangeState::AwaitingApproval
                && change.state() == AssistantWorkflowChangeState::AwaitingApproval
        }) {
            return Err(AssistantApplicationError::PendingApprovalExists);
        }
        values.insert(change.id(), change);
        Ok(())
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
        effect: crate::application::AssistantApplyWorkflowChangeEffect,
    ) -> Result<(), AssistantApplicationError> {
        if effect.workflow_change_id() != change.id() {
            return Err(AssistantApplicationError::InvalidTransition);
        }
        self.replace(expected_state, change)
    }
}

impl ChangeRepositoryFake {
    fn replace(
        &self,
        expected_state: AssistantWorkflowChangeState,
        change: AssistantWorkflowChangeAggregate,
    ) -> Result<(), AssistantApplicationError> {
        let mut values = self.values.lock().unwrap();
        let stored = values.get(&change.id()).ok_or(AssistantApplicationError::NotFound)?;
        if stored.state() != expected_state {
            return Err(AssistantApplicationError::InvalidTransition);
        }
        values.insert(change.id(), change);
        Ok(())
    }
}

#[derive(Default)]
struct RepairRepositoryFake {
    values: Mutex<BTreeMap<(ProjectId, AssistantFailedWorkflowRunId), AssistantRepairActivation>>,
}

#[async_trait]
impl AssistantRepairActivationRepositoryInterface for RepairRepositoryFake {
    async fn record_or_get_repair_activation(
        &self,
        activation: AssistantRepairActivation,
    ) -> Result<AssistantRepairActivationRecordResult, AssistantApplicationError> {
        let key = (activation.project_id(), activation.failed_workflow_run_id());
        let mut values = self.values.lock().unwrap();
        if let Some(existing) = values.get(&key) {
            return Ok(AssistantRepairActivationRecordResult::Existing(existing.clone()));
        }
        values.insert(key, activation.clone());
        Ok(AssistantRepairActivationRecordResult::Created(activation))
    }

    async fn load_repair_activation(
        &self,
        project_id: ProjectId,
        activation_id: AssistantRepairActivationId,
    ) -> Result<Option<AssistantRepairActivation>, AssistantApplicationError> {
        Ok(self
            .values
            .lock()
            .unwrap()
            .values()
            .find(|value| value.project_id() == project_id && value.id() == activation_id)
            .cloned())
    }

    async fn load_repair_activation_for_failed_run(
        &self,
        project_id: ProjectId,
        failed_workflow_run_id: AssistantFailedWorkflowRunId,
    ) -> Result<Option<AssistantRepairActivation>, AssistantApplicationError> {
        Ok(self.values.lock().unwrap().get(&(project_id, failed_workflow_run_id)).cloned())
    }
}

#[tokio::test]
async fn continuation_contract_consumes_exactly_once() {
    let store = ContinuationStoreFake::default();
    let reference = AssistantModelContinuationRef::new("continuation-1").unwrap();
    store
        .store_assistant_model_continuation(AssistantStoredContinuation {
            continuation_ref: reference.clone(),
            project_id: project_id(1),
            session_id: session_id(2),
            invocation_id: invocation_id(3),
            envelope: AssistantModelContinuationEnvelope::new(vec![1]).unwrap(),
        })
        .await
        .unwrap();
    assert!(store.load_assistant_model_continuation(&reference).await.unwrap().is_some());
    assert!(store.consume_assistant_model_continuation(&reference).await.unwrap().is_some());
    assert!(store.consume_assistant_model_continuation(&reference).await.unwrap().is_none());
}

#[tokio::test]
async fn repair_contract_is_idempotent_per_project_and_failed_run() {
    let repository = RepairRepositoryFake::default();
    let first = repair_activation(project_id(1), 2, 3);
    let second = repair_activation(project_id(1), 4, 3);
    assert!(matches!(
        repository.record_or_get_repair_activation(first.clone()).await.unwrap(),
        AssistantRepairActivationRecordResult::Created(value) if value == first
    ));
    assert!(matches!(
        repository.record_or_get_repair_activation(second).await.unwrap(),
        AssistantRepairActivationRecordResult::Existing(value) if value == first
    ));
    let other_project = repair_activation(project_id(9), 5, 3);
    assert!(matches!(
        repository.record_or_get_repair_activation(other_project).await.unwrap(),
        AssistantRepairActivationRecordResult::Created(_)
    ));
}

#[tokio::test]
async fn change_contract_allows_one_pending_approval_per_project_session() {
    let repository = ChangeRepositoryFake::default();
    let first = awaiting_change(project_id(1), 2, 3);
    let duplicate_scope = awaiting_change(project_id(1), 2, 4);
    let independent_project = awaiting_change(project_id(9), 2, 5);
    repository.insert_assistant_workflow_change(first.clone()).await.unwrap();
    assert_eq!(
        repository.insert_assistant_workflow_change(duplicate_scope).await,
        Err(AssistantApplicationError::PendingApprovalExists)
    );
    repository.insert_assistant_workflow_change(independent_project).await.unwrap();
    assert_eq!(
        repository
            .load_pending_assistant_workflow_change(project_id(1), session_id(2))
            .await
            .unwrap()
            .map(|change| change.id()),
        Some(first.id())
    );
}

fn awaiting_change(
    project_id: ProjectId,
    session_seed: u8,
    change_seed: u8,
) -> AssistantWorkflowChangeAggregate {
    let change_id = AssistantWorkflowChangeId::from_uuid(uuid(change_seed)).unwrap();
    let digest = AssistantWorkflowMutationDigest::new([change_seed; 32]);
    let expiry = AssistantWorkflowChangeExpiry::new(20_000).unwrap();
    let candidate = AssistantWorkflowChangeCandidate {
        id: change_id,
        project_id,
        session_id: session_id(session_seed),
        base_workflow_revision: WorkflowRevisionBoundaryValue::new(1).unwrap(),
        ordered_mutations: vec![AssistantWorkflowMutation::new(vec![1]).unwrap()],
        stable_aliases: AssistantWorkflowStableAliasSet::default(),
        readiness_issues: vec![],
        mutation_digest: digest,
        resulting_workflow_fingerprint: AssistantWorkflowFingerprint::new([1; 32]),
        lineage: AssistantWorkflowChangeLineage::UserMessage {
            invocation_id: invocation_id(6),
            intent: AssistantUserIntent::new("intent").unwrap(),
        },
        approval_scope_id: AssistantApprovalScopeId::from_uuid(uuid(7)).unwrap(),
        expires_at: expiry,
    };
    let mut change = AssistantWorkflowChangeAggregate::new(candidate).unwrap();
    let receipt = AssistantReviewReceipt::new(
        change_id,
        digest,
        AssistantContractEpoch::new(1).unwrap(),
        AssistantModelIdentity::new("reviewer@1").unwrap(),
        invocation_id(8),
        AssistantToolCallId::new("call_1").unwrap(),
        AssistantReviewVerdict::Pass,
        AssistantReviewedAt::new(10_000).unwrap(),
    );
    change
        .accept_review(receipt, AssistantModelContinuationRef::new("continuation").unwrap())
        .unwrap();
    change
}

fn repair_activation(
    project_id: ProjectId,
    activation_seed: u8,
    run_seed: u8,
) -> AssistantRepairActivation {
    AssistantRepairActivation::new(
        repair_id(activation_seed),
        project_id,
        session_id(7),
        AssistantFailedWorkflowRunId(*uuid(run_seed).as_bytes()),
        vec![1],
        1,
    )
    .unwrap()
}

fn project_id(seed: u8) -> ProjectId {
    ProjectId::from_uuid(uuid(seed)).unwrap()
}
fn session_id(seed: u8) -> AssistantSessionId {
    AssistantSessionId::from_uuid(uuid(seed)).unwrap()
}
fn invocation_id(seed: u8) -> AssistantModelInvocationId {
    AssistantModelInvocationId::from_uuid(uuid(seed)).unwrap()
}
fn repair_id(seed: u8) -> AssistantRepairActivationId {
    AssistantRepairActivationId::from_uuid(uuid(seed)).unwrap()
}
fn uuid(seed: u8) -> Uuid {
    Uuid::from_bytes([seed, 0, 0, 0, 0, 0, 0x40, 0, 0x80, 0, 0, 0, 0, 0, 0, seed])
}
