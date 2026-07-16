use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use projects::project::domain::ProjectId;
use uuid::Uuid;

use super::*;
use crate::{
    application::AssistantApplyWorkflowChangeEffect, domain::*,
    interfaces::AssistantWorkflowEvaluationResult,
};

struct EvaluatorFake(AssistantWorkflowChangeCandidate);

#[async_trait]
impl AssistantWorkflowMutationEvaluatorInterface for EvaluatorFake {
    async fn evaluate_assistant_workflow_mutations(
        &self,
        _request: AssistantWorkflowEvaluationRequest,
    ) -> Result<AssistantWorkflowEvaluationResult, AssistantApplicationError> {
        Ok(AssistantWorkflowEvaluationResult { candidate: self.0.clone() })
    }
}

#[derive(Clone, Default)]
struct RepositoryFake(Arc<Mutex<Option<AssistantWorkflowChangeAggregate>>>);

#[async_trait]
impl AssistantWorkflowChangeRepositoryInterface for RepositoryFake {
    async fn load_assistant_workflow_change(
        &self,
        change_id: AssistantWorkflowChangeId,
    ) -> Result<Option<AssistantWorkflowChangeAggregate>, AssistantApplicationError> {
        Ok(self.0.lock().unwrap().as_ref().filter(|value| value.id() == change_id).cloned())
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
        *self.0.lock().unwrap() = Some(change);
        Ok(())
    }
    async fn commit_assistant_workflow_change_transition(
        &self,
        _expected_state: AssistantWorkflowChangeState,
        _change: AssistantWorkflowChangeAggregate,
    ) -> Result<(), AssistantApplicationError> {
        Err(AssistantApplicationError::InvalidTransition)
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

#[tokio::test]
async fn evaluator_owned_candidate_is_persisted_without_workflow_commit() {
    let candidate = candidate();
    let repository = RepositoryFake::default();
    let use_case = AssistantEvaluateWorkflowChangeUseCase::new(
        EvaluatorFake(candidate.clone()),
        repository.clone(),
    );
    let result = use_case
        .evaluate_workflow_change(AssistantWorkflowEvaluationRequest {
            project_id: candidate.project_id,
            session_id: candidate.session_id,
            base_workflow_revision: candidate.base_workflow_revision,
            ordered_mutations: candidate.ordered_mutations.clone(),
        })
        .await
        .unwrap();
    assert_eq!(result.state(), AssistantWorkflowChangeState::Proposed);
    assert_eq!(repository.load_assistant_workflow_change(result.id()).await.unwrap(), Some(result));
}

fn candidate() -> AssistantWorkflowChangeCandidate {
    AssistantWorkflowChangeCandidate {
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
            invocation_id: AssistantModelInvocationId::from_uuid(uuid(4)).unwrap(),
            intent: AssistantUserIntent::new("Create a scene").unwrap(),
        },
        approval_scope_id: AssistantApprovalScopeId::from_uuid(uuid(5)).unwrap(),
        expires_at: AssistantWorkflowChangeExpiry::new(20_000).unwrap(),
    }
}

fn uuid(seed: u8) -> Uuid {
    Uuid::from_bytes([seed, 0, 0, 0, 0, 0, 0x40, 0, 0x80, 0, 0, 0, 0, 0, 0, seed])
}
