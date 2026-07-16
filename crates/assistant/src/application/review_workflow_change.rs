use std::{
    collections::{BTreeMap, btree_map::Entry},
    sync::{Arc, Mutex},
};

use projects::project::domain::ProjectId;

use crate::{
    domain::{
        AssistantContractEpoch, AssistantModelContinuationRef, AssistantModelIdentity,
        AssistantModelInvocationId, AssistantReviewReceipt, AssistantReviewVerdict,
        AssistantSessionId, AssistantToolCallId, AssistantWorkflowChangeAggregate,
        AssistantWorkflowChangeId, AssistantWorkflowChangeState, AssistantWorkflowMutationDigest,
    },
    interfaces::{
        AssistantApplicationError, AssistantClockInterface,
        AssistantWorkflowChangeRepositoryInterface,
    },
};

#[derive(Clone, Debug, PartialEq, Eq)]
struct AssistantReviewerFetchFact {
    project_id: ProjectId,
    session_id: AssistantSessionId,
    invocation_id: AssistantModelInvocationId,
    tool_call_id: AssistantToolCallId,
    change_id: AssistantWorkflowChangeId,
    mutation_digest: AssistantWorkflowMutationDigest,
}

/// Process-local exact Reviewer fetch evidence.
#[derive(Clone, Default)]
pub struct AssistantReviewEvidenceRegistry {
    facts: Arc<Mutex<BTreeMap<AssistantModelInvocationId, AssistantReviewerFetchFact>>>,
}

/// Trusted Reviewer fetch request admitted by the tool dispatcher.
pub struct AssistantReviewerFetchCommand {
    pub project_id: ProjectId,
    pub session_id: AssistantSessionId,
    pub invocation_id: AssistantModelInvocationId,
    pub tool_call_id: AssistantToolCallId,
    pub change_id: AssistantWorkflowChangeId,
}

/// Typed Reviewer verdict plus trusted Runner identity and optional pass continuation.
pub struct AssistantReviewerVerdictCommand {
    pub project_id: ProjectId,
    pub session_id: AssistantSessionId,
    pub invocation_id: AssistantModelInvocationId,
    pub change_id: AssistantWorkflowChangeId,
    pub mutation_digest: AssistantWorkflowMutationDigest,
    pub verdict: AssistantReviewVerdict,
    pub reviewer_contract_epoch: AssistantContractEpoch,
    pub reviewer_model: AssistantModelIdentity,
    pub continuation_ref: Option<AssistantModelContinuationRef>,
}

/// Records exact candidate fetches and persists only verified Reviewer verdicts.
#[derive(Clone)]
pub struct AssistantReviewWorkflowChangeUseCase<R, C> {
    repository: R,
    clock: C,
    evidence: AssistantReviewEvidenceRegistry,
}

impl<R, C> AssistantReviewWorkflowChangeUseCase<R, C>
where
    R: AssistantWorkflowChangeRepositoryInterface,
    C: AssistantClockInterface,
{
    #[must_use]
    pub const fn new(repository: R, clock: C, evidence: AssistantReviewEvidenceRegistry) -> Self {
        Self { repository, clock, evidence }
    }

    pub async fn record_candidate_fetch(
        &self,
        command: AssistantReviewerFetchCommand,
    ) -> Result<AssistantWorkflowChangeAggregate, AssistantApplicationError> {
        let change = self
            .load_visible_change(command.project_id, command.session_id, command.change_id)
            .await?;
        if change.state() != AssistantWorkflowChangeState::Proposed {
            return Err(AssistantApplicationError::InvalidTransition);
        }
        let fact = AssistantReviewerFetchFact {
            project_id: command.project_id,
            session_id: command.session_id,
            invocation_id: command.invocation_id,
            tool_call_id: command.tool_call_id,
            change_id: command.change_id,
            mutation_digest: change.mutation_digest(),
        };
        let mut facts = self
            .evidence
            .facts
            .lock()
            .map_err(|_| AssistantApplicationError::ExternalBoundaryFailed)?;
        match facts.entry(command.invocation_id) {
            Entry::Vacant(entry) => {
                entry.insert(fact);
            }
            Entry::Occupied(_) => return Err(AssistantApplicationError::ReviewEvidenceInvalid),
        }
        Ok(change)
    }

    pub async fn accept_reviewer_verdict(
        &self,
        command: AssistantReviewerVerdictCommand,
    ) -> Result<AssistantWorkflowChangeAggregate, AssistantApplicationError> {
        let mut change = self
            .load_visible_change(command.project_id, command.session_id, command.change_id)
            .await?;
        let fact = self.matching_fact(&command)?;
        let receipt = AssistantReviewReceipt::new(
            command.change_id,
            command.mutation_digest,
            command.reviewer_contract_epoch,
            command.reviewer_model,
            command.invocation_id,
            fact.tool_call_id,
            command.verdict,
            self.clock.current_assistant_time()?,
        );
        apply_verdict(&mut change, receipt, command.continuation_ref)?;
        self.repository
            .commit_assistant_workflow_change_transition(
                AssistantWorkflowChangeState::Proposed,
                change.clone(),
            )
            .await?;
        self.consume_fact(command.invocation_id)?;
        Ok(change)
    }

    async fn load_visible_change(
        &self,
        project_id: ProjectId,
        session_id: AssistantSessionId,
        change_id: AssistantWorkflowChangeId,
    ) -> Result<AssistantWorkflowChangeAggregate, AssistantApplicationError> {
        let change = self
            .repository
            .load_assistant_workflow_change(change_id)
            .await?
            .ok_or(AssistantApplicationError::NotFound)?;
        if change.project_id() != project_id || change.session_id() != session_id {
            Err(AssistantApplicationError::NotVisible)
        } else {
            Ok(change)
        }
    }

    fn matching_fact(
        &self,
        command: &AssistantReviewerVerdictCommand,
    ) -> Result<AssistantReviewerFetchFact, AssistantApplicationError> {
        let facts = self
            .evidence
            .facts
            .lock()
            .map_err(|_| AssistantApplicationError::ExternalBoundaryFailed)?;
        let fact = facts
            .get(&command.invocation_id)
            .ok_or(AssistantApplicationError::ReviewEvidenceInvalid)?;
        if fact.project_id != command.project_id
            || fact.session_id != command.session_id
            || fact.change_id != command.change_id
            || fact.mutation_digest != command.mutation_digest
        {
            return Err(AssistantApplicationError::ReviewEvidenceInvalid);
        }
        Ok(fact.clone())
    }

    fn consume_fact(
        &self,
        invocation_id: AssistantModelInvocationId,
    ) -> Result<(), AssistantApplicationError> {
        self.evidence
            .facts
            .lock()
            .map_err(|_| AssistantApplicationError::ExternalBoundaryFailed)?
            .remove(&invocation_id);
        Ok(())
    }
}

fn apply_verdict(
    change: &mut AssistantWorkflowChangeAggregate,
    receipt: AssistantReviewReceipt,
    continuation_ref: Option<AssistantModelContinuationRef>,
) -> Result<(), AssistantApplicationError> {
    match receipt.verdict {
        AssistantReviewVerdict::Pass => change.accept_review(
            receipt,
            continuation_ref.ok_or(AssistantApplicationError::ContinuationIncompatible)?,
        ),
        AssistantReviewVerdict::Reject if continuation_ref.is_none() => {
            change.reject_review(receipt)
        }
        AssistantReviewVerdict::Reject => {
            return Err(AssistantApplicationError::ReviewEvidenceInvalid);
        }
    }
    .map_err(|_| AssistantApplicationError::ReviewEvidenceInvalid)
}

#[cfg(test)]
mod tests;
