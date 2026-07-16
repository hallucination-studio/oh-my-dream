use super::{
    AssistantContinuationOutcome, AssistantModelContinuationRef, AssistantReviewReceipt,
    AssistantReviewVerdict, AssistantWorkflowApplyReceiptBoundaryValue,
    AssistantWorkflowChangeAggregate, AssistantWorkflowChangeCandidate,
    AssistantWorkflowChangeError, AssistantWorkflowChangeState, AssistantWorkflowRunBoundaryValue,
};

const MAX_ITEMS: usize = 128;
const MAX_COMBINED_BYTES: usize = 1024 * 1024;

pub(super) fn valid_restored_evidence(
    aggregate: &AssistantWorkflowChangeAggregate,
    review: Option<&AssistantReviewReceipt>,
    continuation_ref: Option<&AssistantModelContinuationRef>,
    state: AssistantWorkflowChangeState,
    applied_receipt: Option<&AssistantWorkflowApplyReceiptBoundaryValue>,
    admitted_run: Option<&AssistantWorkflowRunBoundaryValue>,
    continuation_outcome: AssistantContinuationOutcome,
) -> bool {
    let verdict = review.map(|receipt| receipt.verdict);
    let review_matches = review.is_none_or(|receipt| {
        receipt.change_id == aggregate.id()
            && receipt.mutation_digest == aggregate.mutation_digest()
    });
    review_matches
        && valid_outcomes(state, applied_receipt, admitted_run, continuation_outcome)
        && valid_review_state(state, verdict, review, continuation_ref)
}

pub(super) fn validate_candidate(
    candidate: &AssistantWorkflowChangeCandidate,
) -> Result<(), AssistantWorkflowChangeError> {
    if candidate.ordered_mutations.is_empty()
        || candidate.ordered_mutations.len() > MAX_ITEMS
        || candidate.readiness_issues.len() > MAX_ITEMS
    {
        return Err(AssistantWorkflowChangeError::CandidateTooLarge);
    }
    let bytes = candidate
        .ordered_mutations
        .iter()
        .map(|value| value.canonical_bytes().len())
        .chain(candidate.readiness_issues.iter().map(|value| value.canonical_bytes().len()))
        .try_fold(0usize, usize::checked_add)
        .ok_or(AssistantWorkflowChangeError::CandidateTooLarge)?;
    if bytes > MAX_COMBINED_BYTES {
        Err(AssistantWorkflowChangeError::CandidateTooLarge)
    } else {
        Ok(())
    }
}

fn valid_outcomes(
    state: AssistantWorkflowChangeState,
    applied_receipt: Option<&AssistantWorkflowApplyReceiptBoundaryValue>,
    admitted_run: Option<&AssistantWorkflowRunBoundaryValue>,
    continuation_outcome: AssistantContinuationOutcome,
) -> bool {
    match state {
        AssistantWorkflowChangeState::Applied => {
            applied_receipt.is_some()
                && (admitted_run.is_none()
                    || continuation_outcome != AssistantContinuationOutcome::Pending)
        }
        _ => {
            applied_receipt.is_none()
                && admitted_run.is_none()
                && continuation_outcome == AssistantContinuationOutcome::Pending
        }
    }
}

fn valid_review_state(
    state: AssistantWorkflowChangeState,
    verdict: Option<AssistantReviewVerdict>,
    review: Option<&AssistantReviewReceipt>,
    continuation_ref: Option<&AssistantModelContinuationRef>,
) -> bool {
    match state {
        AssistantWorkflowChangeState::Proposed => review.is_none() && continuation_ref.is_none(),
        AssistantWorkflowChangeState::ReviewRejected => {
            verdict == Some(AssistantReviewVerdict::Reject) && continuation_ref.is_none()
        }
        AssistantWorkflowChangeState::Rejected
        | AssistantWorkflowChangeState::Expired
        | AssistantWorkflowChangeState::AwaitingApproval
        | AssistantWorkflowChangeState::Applying
        | AssistantWorkflowChangeState::Applied
        | AssistantWorkflowChangeState::ApplyFailed => {
            verdict == Some(AssistantReviewVerdict::Pass) && continuation_ref.is_some()
        }
    }
}
