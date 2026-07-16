use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use uuid::Uuid;

use super::*;
use crate::{
    application::AssistantApplyWorkflowChangeEffect,
    domain::{
        AssistantApprovalScopeId, AssistantReviewedAt, AssistantUserIntent,
        AssistantWorkflowChangeCandidate, AssistantWorkflowChangeExpiry,
        AssistantWorkflowChangeLineage, AssistantWorkflowFingerprint, AssistantWorkflowMutation,
        AssistantWorkflowStableAliasSet, WorkflowRevisionBoundaryValue,
    },
};

#[derive(Clone, Default)]
struct RepositoryFake {
    values: Arc<Mutex<BTreeMap<AssistantWorkflowChangeId, AssistantWorkflowChangeAggregate>>>,
}

#[async_trait]
impl AssistantWorkflowChangeRepositoryInterface for RepositoryFake {
    async fn load_assistant_workflow_change(
        &self,
        change_id: AssistantWorkflowChangeId,
    ) -> Result<Option<AssistantWorkflowChangeAggregate>, AssistantApplicationError> {
        Ok(self.values.lock().unwrap().get(&change_id).cloned())
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
        change: AssistantWorkflowChangeAggregate,
    ) -> Result<(), AssistantApplicationError> {
        self.values.lock().unwrap().insert(change.id(), change);
        Ok(())
    }
    async fn commit_assistant_workflow_change_transition(
        &self,
        expected_state: AssistantWorkflowChangeState,
        change: AssistantWorkflowChangeAggregate,
    ) -> Result<(), AssistantApplicationError> {
        let mut values = self.values.lock().unwrap();
        if values.get(&change.id()).map(AssistantWorkflowChangeAggregate::state)
            != Some(expected_state)
        {
            return Err(AssistantApplicationError::InvalidTransition);
        }
        values.insert(change.id(), change);
        Ok(())
    }
    async fn commit_assistant_workflow_change_apply_decision(
        &self,
        expected_state: AssistantWorkflowChangeState,
        change: AssistantWorkflowChangeAggregate,
        _effect: AssistantApplyWorkflowChangeEffect,
    ) -> Result<(), AssistantApplicationError> {
        self.commit_assistant_workflow_change_transition(expected_state, change).await
    }
}

#[derive(Clone, Copy)]
struct ClockFake;

impl AssistantClockInterface for ClockFake {
    fn current_assistant_time(&self) -> Result<AssistantReviewedAt, AssistantApplicationError> {
        Ok(AssistantReviewedAt::new(10_000).unwrap())
    }
}

#[tokio::test]
async fn exact_fetch_fact_is_required_and_consumed_only_after_persisted_verdict() {
    let repository = RepositoryFake::default();
    let change = proposed_change();
    repository.insert_assistant_workflow_change(change.clone()).await.unwrap();
    let evidence = AssistantReviewEvidenceRegistry::default();
    let use_case = AssistantReviewWorkflowChangeUseCase::new(repository, ClockFake, evidence);
    let invocation_id = invocation_id(8);
    use_case
        .record_candidate_fetch(AssistantReviewerFetchCommand {
            project_id: change.project_id(),
            session_id: change.session_id(),
            invocation_id,
            tool_call_id: AssistantToolCallId::new("call_1").unwrap(),
            change_id: change.id(),
        })
        .await
        .unwrap();

    let mismatch =
        verdict_command(&change, invocation_id, AssistantWorkflowMutationDigest::new([9; 32]));
    assert_eq!(
        use_case.accept_reviewer_verdict(mismatch).await,
        Err(AssistantApplicationError::ReviewEvidenceInvalid)
    );
    let reviewed = use_case
        .accept_reviewer_verdict(verdict_command(&change, invocation_id, change.mutation_digest()))
        .await
        .unwrap();
    assert_eq!(reviewed.state(), AssistantWorkflowChangeState::AwaitingApproval);
    assert_eq!(
        use_case
            .accept_reviewer_verdict(verdict_command(
                &change,
                invocation_id,
                change.mutation_digest()
            ))
            .await,
        Err(AssistantApplicationError::ReviewEvidenceInvalid)
    );
}

fn verdict_command(
    change: &AssistantWorkflowChangeAggregate,
    invocation_id: AssistantModelInvocationId,
    mutation_digest: AssistantWorkflowMutationDigest,
) -> AssistantReviewerVerdictCommand {
    AssistantReviewerVerdictCommand {
        project_id: change.project_id(),
        session_id: change.session_id(),
        invocation_id,
        change_id: change.id(),
        mutation_digest,
        verdict: AssistantReviewVerdict::Pass,
        reviewer_contract_epoch: AssistantContractEpoch::new(1).unwrap(),
        reviewer_model: AssistantModelIdentity::new("workflow_change_reviewer@1").unwrap(),
        continuation_ref: Some(AssistantModelContinuationRef::new("continuation-1").unwrap()),
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
        expires_at: AssistantWorkflowChangeExpiry::new(20_000).unwrap(),
    })
    .unwrap()
}

fn invocation_id(seed: u8) -> AssistantModelInvocationId {
    AssistantModelInvocationId::from_uuid(uuid(seed)).unwrap()
}

fn uuid(seed: u8) -> Uuid {
    Uuid::from_bytes([seed, 0, 0, 0, 0, 0, 0x40, 0, 0x80, 0, 0, 0, 0, 0, 0, seed])
}
