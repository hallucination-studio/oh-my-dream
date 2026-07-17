use crate::{
    domain::{
        AssistantWorkflowChangeAggregate, AssistantWorkflowChangeId, AssistantWorkflowChangeState,
        WorkflowRevisionBoundaryValue,
    },
    interfaces::{
        AssistantApplicationError, AssistantModelContinuationStoreInterface,
        AssistantModelResumeRequest, AssistantModelRunnerInterface, AssistantModelTurnInput,
        AssistantWorkflowApplyRequest, AssistantWorkflowChangeRepositoryInterface,
        AssistantWorkflowMutationApplierInterface, AssistantWorkflowRunRequest,
        AssistantWorkflowRunStarterInterface,
    },
};

/// Executes one durable approved-change effect idempotently through canonical bridges.
pub struct AssistantApplyWorkflowChangeEffectUseCase<R, A, S, M, W> {
    repository: R,
    mutation_applier: A,
    continuation_store: S,
    model_runner: M,
    run_starter: W,
}

#[cfg(test)]
mod tests;

impl<R, A, S, M, W> AssistantApplyWorkflowChangeEffectUseCase<R, A, S, M, W>
where
    R: AssistantWorkflowChangeRepositoryInterface,
    A: AssistantWorkflowMutationApplierInterface,
    S: AssistantModelContinuationStoreInterface,
    M: AssistantModelRunnerInterface,
    W: AssistantWorkflowRunStarterInterface,
{
    #[must_use]
    pub const fn new(
        repository: R,
        mutation_applier: A,
        continuation_store: S,
        model_runner: M,
        run_starter: W,
    ) -> Self {
        Self { repository, mutation_applier, continuation_store, model_runner, run_starter }
    }

    pub async fn apply_workflow_change_effect(
        &self,
        change_id: AssistantWorkflowChangeId,
    ) -> Result<AssistantWorkflowChangeAggregate, AssistantApplicationError> {
        let mut change = self
            .repository
            .load_assistant_workflow_change(change_id)
            .await?
            .ok_or(AssistantApplicationError::NotFound)?;
        if change.admitted_workflow_run().is_some() {
            return Ok(change);
        }
        if change.state() == AssistantWorkflowChangeState::Applying {
            let receipt = self
                .mutation_applier
                .apply_assistant_workflow_change(AssistantWorkflowApplyRequest {
                    change: change.clone(),
                })
                .await?;
            change
                .mark_applied(receipt)
                .map_err(|_| AssistantApplicationError::InvalidTransition)?;
            self.repository
                .commit_assistant_workflow_change_transition(
                    AssistantWorkflowChangeState::Applying,
                    change.clone(),
                )
                .await?;
        }
        if change.state() != AssistantWorkflowChangeState::Applied {
            return Err(AssistantApplicationError::InvalidTransition);
        }
        if change.continuation_outcome() == crate::domain::AssistantContinuationOutcome::Pending {
            self.resolve_continuation(&mut change).await?;
            self.repository
                .commit_assistant_workflow_change_transition(
                    AssistantWorkflowChangeState::Applied,
                    change.clone(),
                )
                .await?;
        }
        let run = self
            .run_starter
            .start_assistant_workflow_run(AssistantWorkflowRunRequest {
                project_id: change.project_id(),
                workflow_change_id: change.id(),
                applied_workflow_receipt: change
                    .applied_workflow_receipt()
                    .ok_or(AssistantApplicationError::InvalidTransition)?
                    .clone(),
            })
            .await?;
        change
            .link_admitted_workflow_run(run)
            .map_err(|_| AssistantApplicationError::InvalidTransition)?;
        self.repository
            .commit_assistant_workflow_change_transition(
                AssistantWorkflowChangeState::Applied,
                change.clone(),
            )
            .await?;
        Ok(change)
    }

    async fn resolve_continuation(
        &self,
        change: &mut AssistantWorkflowChangeAggregate,
    ) -> Result<(), AssistantApplicationError> {
        let continuation_ref =
            change.continuation_ref().ok_or(AssistantApplicationError::ContinuationIncompatible)?;
        let stored =
            self.continuation_store.consume_assistant_model_continuation(continuation_ref).await?;
        let resumed = if let Some(stored) = stored
            && stored.project_id == change.project_id()
            && stored.session_id == change.session_id()
        {
            let receipt = change
                .applied_workflow_receipt()
                .ok_or(AssistantApplicationError::InvalidTransition)?;
            self.model_runner
                .resume_assistant_model_turn(AssistantModelResumeRequest {
                    project_id: stored.project_id,
                    session_id: stored.session_id,
                    invocation_id: stored.invocation_id,
                    lineage: change.lineage().clone(),
                    observed_workflow_revision: WorkflowRevisionBoundaryValue::new(
                        change
                            .base_workflow_revision()
                            .get()
                            .checked_add(1)
                            .ok_or(AssistantApplicationError::ProtocolViolation)?,
                    )
                    .map_err(|_| AssistantApplicationError::ProtocolViolation)?,
                    continuation: stored.envelope,
                    input: AssistantModelTurnInput::new(receipt.canonical_bytes().to_vec())?,
                })
                .await
                .is_ok()
        } else {
            false
        };
        let result = if resumed {
            change.mark_continuation_resumed()
        } else {
            change.mark_continuation_interrupted()
        };
        result.map_err(|_| AssistantApplicationError::InvalidTransition)
    }
}
