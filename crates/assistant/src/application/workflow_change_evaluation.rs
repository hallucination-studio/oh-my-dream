use crate::{
    domain::AssistantWorkflowChangeAggregate,
    interfaces::{
        AssistantApplicationError, AssistantWorkflowChangeRepositoryInterface,
        AssistantWorkflowEvaluationRequest, AssistantWorkflowMutationEvaluatorInterface,
    },
};

/// Evaluates one canonical candidate without committing Workflow and persists the proposal.
pub struct AssistantEvaluateWorkflowChangeUseCase<E, R> {
    evaluator: E,
    repository: R,
}

#[cfg(test)]
mod tests;

impl<E, R> AssistantEvaluateWorkflowChangeUseCase<E, R>
where
    E: AssistantWorkflowMutationEvaluatorInterface,
    R: AssistantWorkflowChangeRepositoryInterface,
{
    #[must_use]
    pub const fn new(evaluator: E, repository: R) -> Self {
        Self { evaluator, repository }
    }

    pub async fn evaluate_workflow_change(
        &self,
        request: AssistantWorkflowEvaluationRequest,
    ) -> Result<AssistantWorkflowChangeAggregate, AssistantApplicationError> {
        let expected = request.clone();
        let evaluation = self.evaluator.evaluate_assistant_workflow_mutations(request).await?;
        if evaluation.candidate.id != expected.authorization.change_id
            || evaluation.candidate.project_id != expected.authorization.project_id
            || evaluation.candidate.session_id != expected.authorization.session_id
            || evaluation.candidate.base_workflow_revision != expected.base_workflow_revision
            || evaluation.candidate.lineage != expected.authorization.lineage
            || evaluation.candidate.approval_scope_id != expected.authorization.approval_scope_id
            || evaluation.candidate.expires_at != expected.authorization.expires_at
        {
            return Err(AssistantApplicationError::CandidateFingerprintMismatch);
        }
        let change = AssistantWorkflowChangeAggregate::new(evaluation.candidate)
            .map_err(|_| AssistantApplicationError::CandidateFingerprintMismatch)?;
        self.repository.insert_assistant_workflow_change(change.clone()).await?;
        Ok(change)
    }
}
