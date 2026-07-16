use crate::{
    application::AssistantApplyWorkflowChangeEffect,
    domain::{
        AssistantWorkflowChangeDecisionScope, AssistantWorkflowChangeError,
        AssistantWorkflowChangeId, AssistantWorkflowChangeState,
    },
    interfaces::{
        AssistantApplicationError, AssistantModelContinuationStoreInterface,
        AssistantWorkflowChangeRepositoryInterface,
    },
};

/// Closed human decision over one exact reviewed proposal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssistantWorkflowChangeDecision {
    Approve,
    Reject,
}

/// Exact decision command after Desktop translates all approval proof.
pub struct AssistantDecideWorkflowChangeCommand {
    pub workflow_change_id: AssistantWorkflowChangeId,
    pub scope: AssistantWorkflowChangeDecisionScope,
    pub decision: AssistantWorkflowChangeDecision,
    pub now_epoch_ms: i64,
}

/// Commits the terminal rejection or Applying+effect transaction before external work.
pub struct AssistantDecideWorkflowChangeUseCase<R, S> {
    repository: R,
    continuation_store: S,
}

impl<R, S> AssistantDecideWorkflowChangeUseCase<R, S>
where
    R: AssistantWorkflowChangeRepositoryInterface,
    S: AssistantModelContinuationStoreInterface,
{
    #[must_use]
    pub const fn new(repository: R, continuation_store: S) -> Self {
        Self { repository, continuation_store }
    }

    pub async fn decide_workflow_change(
        &self,
        command: AssistantDecideWorkflowChangeCommand,
    ) -> Result<crate::domain::AssistantWorkflowChangeAggregate, AssistantApplicationError> {
        let mut change = self
            .repository
            .load_assistant_workflow_change(command.workflow_change_id)
            .await?
            .ok_or(AssistantApplicationError::NotFound)?;
        if change.project_id() != command.scope.project_id
            || change.session_id() != command.scope.session_id
        {
            return Err(AssistantApplicationError::NotVisible);
        }
        if !change.matches_decision_scope(command.scope) {
            return Err(AssistantApplicationError::ApprovalMismatch);
        }
        if command.decision == AssistantWorkflowChangeDecision::Approve
            && matches!(
                change.state(),
                AssistantWorkflowChangeState::Applying | AssistantWorkflowChangeState::Applied
            )
        {
            return Ok(change);
        }
        if command.decision == AssistantWorkflowChangeDecision::Reject
            && change.state() == AssistantWorkflowChangeState::Rejected
        {
            let continuation_ref = change
                .continuation_ref()
                .ok_or(AssistantApplicationError::ContinuationIncompatible)?;
            self.continuation_store.consume_assistant_model_continuation(continuation_ref).await?;
            return Ok(change);
        }
        match command.decision {
            AssistantWorkflowChangeDecision::Approve => {
                change
                    .begin_apply(command.scope, command.now_epoch_ms)
                    .map_err(map_decision_error)?;
                let effect = AssistantApplyWorkflowChangeEffect::new(change.id());
                self.repository
                    .commit_assistant_workflow_change_apply_decision(
                        AssistantWorkflowChangeState::AwaitingApproval,
                        change.clone(),
                        effect,
                    )
                    .await?;
            }
            AssistantWorkflowChangeDecision::Reject => {
                let continuation_ref = change
                    .continuation_ref()
                    .cloned()
                    .ok_or(AssistantApplicationError::ContinuationIncompatible)?;
                change.reject(command.scope, command.now_epoch_ms).map_err(map_decision_error)?;
                self.repository
                    .commit_assistant_workflow_change_transition(
                        AssistantWorkflowChangeState::AwaitingApproval,
                        change.clone(),
                    )
                    .await?;
                self.continuation_store
                    .consume_assistant_model_continuation(&continuation_ref)
                    .await?;
            }
        }
        Ok(change)
    }
}

fn map_decision_error(error: AssistantWorkflowChangeError) -> AssistantApplicationError {
    match error {
        AssistantWorkflowChangeError::ApprovalExpired => AssistantApplicationError::ApprovalExpired,
        AssistantWorkflowChangeError::InvalidValue => AssistantApplicationError::ApprovalMismatch,
        _ => AssistantApplicationError::InvalidTransition,
    }
}

#[cfg(test)]
mod tests;
