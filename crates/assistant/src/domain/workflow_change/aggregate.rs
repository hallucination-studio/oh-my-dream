use projects::project::domain::ProjectId;

use super::validation::{valid_restored_evidence, validate_candidate};
use super::{
    AssistantContinuationOutcome, AssistantModelContinuationRef, AssistantReviewReceipt,
    AssistantReviewVerdict, AssistantWorkflowApplyReceiptBoundaryValue,
    AssistantWorkflowChangeDecisionScope, AssistantWorkflowChangeError,
    AssistantWorkflowChangeExpiry, AssistantWorkflowChangeLineage, AssistantWorkflowChangeState,
    AssistantWorkflowFingerprint, AssistantWorkflowMutation, AssistantWorkflowMutationDigest,
    AssistantWorkflowReadinessIssueBoundaryValue, AssistantWorkflowRunBoundaryValue,
    AssistantWorkflowStableAliasSet, WorkflowRevisionBoundaryValue,
};
use crate::domain::{AssistantApprovalScopeId, AssistantSessionId, AssistantWorkflowChangeId};

/// Complete immutable evaluator result used to create one proposal.
#[derive(Clone, Debug)]
pub struct AssistantWorkflowChangeCandidate {
    pub id: AssistantWorkflowChangeId,
    pub project_id: ProjectId,
    pub session_id: AssistantSessionId,
    pub base_workflow_revision: WorkflowRevisionBoundaryValue,
    pub ordered_mutations: Vec<AssistantWorkflowMutation>,
    pub stable_aliases: AssistantWorkflowStableAliasSet,
    pub readiness_issues: Vec<AssistantWorkflowReadinessIssueBoundaryValue>,
    pub mutation_digest: AssistantWorkflowMutationDigest,
    pub resulting_workflow_fingerprint: AssistantWorkflowFingerprint,
    pub lineage: AssistantWorkflowChangeLineage,
    pub approval_scope_id: AssistantApprovalScopeId,
    pub expires_at: AssistantWorkflowChangeExpiry,
}

/// Immutable proposal plus its authoritative review and decision lifecycle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssistantWorkflowChangeAggregate {
    id: AssistantWorkflowChangeId,
    project_id: ProjectId,
    session_id: AssistantSessionId,
    base_workflow_revision: WorkflowRevisionBoundaryValue,
    ordered_mutations: Vec<AssistantWorkflowMutation>,
    stable_aliases: AssistantWorkflowStableAliasSet,
    readiness_issues: Vec<AssistantWorkflowReadinessIssueBoundaryValue>,
    mutation_digest: AssistantWorkflowMutationDigest,
    resulting_workflow_fingerprint: AssistantWorkflowFingerprint,
    lineage: AssistantWorkflowChangeLineage,
    review: Option<AssistantReviewReceipt>,
    approval_scope_id: AssistantApprovalScopeId,
    continuation_ref: Option<AssistantModelContinuationRef>,
    state: AssistantWorkflowChangeState,
    expires_at: AssistantWorkflowChangeExpiry,
    applied_workflow_receipt: Option<AssistantWorkflowApplyReceiptBoundaryValue>,
    admitted_workflow_run: Option<AssistantWorkflowRunBoundaryValue>,
    continuation_outcome: AssistantContinuationOutcome,
}

impl AssistantWorkflowChangeAggregate {
    /// Creates one immutable Proposed candidate from evaluator-owned evidence.
    pub fn new(
        candidate: AssistantWorkflowChangeCandidate,
    ) -> Result<Self, AssistantWorkflowChangeError> {
        validate_candidate(&candidate)?;
        Ok(Self {
            id: candidate.id,
            project_id: candidate.project_id,
            session_id: candidate.session_id,
            base_workflow_revision: candidate.base_workflow_revision,
            ordered_mutations: candidate.ordered_mutations,
            stable_aliases: candidate.stable_aliases,
            readiness_issues: candidate.readiness_issues,
            mutation_digest: candidate.mutation_digest,
            resulting_workflow_fingerprint: candidate.resulting_workflow_fingerprint,
            lineage: candidate.lineage,
            review: None,
            approval_scope_id: candidate.approval_scope_id,
            continuation_ref: None,
            state: AssistantWorkflowChangeState::Proposed,
            expires_at: candidate.expires_at,
            applied_workflow_receipt: None,
            admitted_workflow_run: None,
            continuation_outcome: AssistantContinuationOutcome::Pending,
        })
    }

    /// Restores one persisted aggregate while rechecking state-associated evidence.
    pub fn try_restore(
        candidate: AssistantWorkflowChangeCandidate,
        review: Option<AssistantReviewReceipt>,
        continuation_ref: Option<AssistantModelContinuationRef>,
        state: AssistantWorkflowChangeState,
        applied_workflow_receipt: Option<AssistantWorkflowApplyReceiptBoundaryValue>,
        admitted_workflow_run: Option<AssistantWorkflowRunBoundaryValue>,
        continuation_outcome: AssistantContinuationOutcome,
    ) -> Result<Self, AssistantWorkflowChangeError> {
        let mut aggregate = Self::new(candidate)?;
        if !valid_restored_evidence(
            &aggregate,
            review.as_ref(),
            continuation_ref.as_ref(),
            state,
            applied_workflow_receipt.as_ref(),
            admitted_workflow_run.as_ref(),
            continuation_outcome,
        ) {
            return Err(AssistantWorkflowChangeError::InvalidValue);
        }
        aggregate.review = review;
        aggregate.continuation_ref = continuation_ref;
        aggregate.state = state;
        aggregate.applied_workflow_receipt = applied_workflow_receipt;
        aggregate.admitted_workflow_run = admitted_workflow_run;
        aggregate.continuation_outcome = continuation_outcome;
        Ok(aggregate)
    }

    #[must_use]
    pub const fn id(&self) -> AssistantWorkflowChangeId {
        self.id
    }
    #[must_use]
    pub const fn project_id(&self) -> ProjectId {
        self.project_id
    }
    #[must_use]
    pub const fn session_id(&self) -> AssistantSessionId {
        self.session_id
    }
    #[must_use]
    pub const fn mutation_digest(&self) -> AssistantWorkflowMutationDigest {
        self.mutation_digest
    }
    #[must_use]
    pub const fn state(&self) -> AssistantWorkflowChangeState {
        self.state
    }
    #[must_use]
    pub const fn base_workflow_revision(&self) -> WorkflowRevisionBoundaryValue {
        self.base_workflow_revision
    }
    #[must_use]
    pub fn ordered_mutations(&self) -> &[AssistantWorkflowMutation] {
        &self.ordered_mutations
    }
    #[must_use]
    pub const fn stable_aliases(&self) -> &AssistantWorkflowStableAliasSet {
        &self.stable_aliases
    }
    #[must_use]
    pub fn readiness_issues(&self) -> &[AssistantWorkflowReadinessIssueBoundaryValue] {
        &self.readiness_issues
    }
    #[must_use]
    pub const fn resulting_workflow_fingerprint(&self) -> AssistantWorkflowFingerprint {
        self.resulting_workflow_fingerprint
    }
    #[must_use]
    pub const fn lineage(&self) -> &AssistantWorkflowChangeLineage {
        &self.lineage
    }
    #[must_use]
    pub const fn approval_scope_id(&self) -> AssistantApprovalScopeId {
        self.approval_scope_id
    }
    #[must_use]
    pub const fn expires_at(&self) -> AssistantWorkflowChangeExpiry {
        self.expires_at
    }
    #[must_use]
    pub fn applied_workflow_receipt(&self) -> Option<&AssistantWorkflowApplyReceiptBoundaryValue> {
        self.applied_workflow_receipt.as_ref()
    }
    #[must_use]
    pub fn admitted_workflow_run(&self) -> Option<&AssistantWorkflowRunBoundaryValue> {
        self.admitted_workflow_run.as_ref()
    }
    #[must_use]
    pub const fn continuation_outcome(&self) -> AssistantContinuationOutcome {
        self.continuation_outcome
    }
    #[must_use]
    pub fn review(&self) -> Option<&AssistantReviewReceipt> {
        self.review.as_ref()
    }
    #[must_use]
    pub fn continuation_ref(&self) -> Option<&AssistantModelContinuationRef> {
        self.continuation_ref.as_ref()
    }
    #[must_use]
    pub fn matches_decision_scope(&self, scope: AssistantWorkflowChangeDecisionScope) -> bool {
        scope.project_id == self.project_id
            && scope.session_id == self.session_id
            && scope.change_id == self.id
            && scope.approval_scope_id == self.approval_scope_id
            && scope.mutation_digest == self.mutation_digest
    }

    /// Stores verified pass evidence and enters human approval.
    pub fn accept_review(
        &mut self,
        receipt: AssistantReviewReceipt,
        continuation_ref: AssistantModelContinuationRef,
    ) -> Result<(), AssistantWorkflowChangeError> {
        self.validate_review(&receipt, AssistantReviewVerdict::Pass)?;
        self.review = Some(receipt);
        self.continuation_ref = Some(continuation_ref);
        self.state = AssistantWorkflowChangeState::AwaitingApproval;
        Ok(())
    }

    /// Stores verified rejection evidence and terminates the proposal.
    pub fn reject_review(
        &mut self,
        receipt: AssistantReviewReceipt,
    ) -> Result<(), AssistantWorkflowChangeError> {
        self.validate_review(&receipt, AssistantReviewVerdict::Reject)?;
        self.review = Some(receipt);
        self.state = AssistantWorkflowChangeState::ReviewRejected;
        Ok(())
    }

    /// Records the human rejection of an unexpired reviewed proposal.
    pub fn reject(
        &mut self,
        scope: AssistantWorkflowChangeDecisionScope,
        now_epoch_ms: i64,
    ) -> Result<(), AssistantWorkflowChangeError> {
        self.require_unexpired_approval(scope, now_epoch_ms)?;
        self.state = AssistantWorkflowChangeState::Rejected;
        Ok(())
    }

    /// Commits the recoverable Applying decision before external effects.
    pub fn begin_apply(
        &mut self,
        scope: AssistantWorkflowChangeDecisionScope,
        now_epoch_ms: i64,
    ) -> Result<(), AssistantWorkflowChangeError> {
        self.require_unexpired_approval(scope, now_epoch_ms)?;
        self.state = AssistantWorkflowChangeState::Applying;
        Ok(())
    }

    /// Marks a recoverable apply as durably completed.
    pub fn mark_applied(
        &mut self,
        receipt: AssistantWorkflowApplyReceiptBoundaryValue,
    ) -> Result<(), AssistantWorkflowChangeError> {
        if self.state != AssistantWorkflowChangeState::Applying {
            return Err(AssistantWorkflowChangeError::InvalidTransition);
        }
        self.applied_workflow_receipt = Some(receipt);
        self.state = AssistantWorkflowChangeState::Applied;
        Ok(())
    }

    /// Links the stable admitted Run idempotently after canonical apply.
    pub fn link_admitted_workflow_run(
        &mut self,
        run: AssistantWorkflowRunBoundaryValue,
    ) -> Result<(), AssistantWorkflowChangeError> {
        if self.state != AssistantWorkflowChangeState::Applied
            || self.continuation_outcome == AssistantContinuationOutcome::Pending
        {
            return Err(AssistantWorkflowChangeError::InvalidTransition);
        }
        match self.admitted_workflow_run.as_ref() {
            None => self.admitted_workflow_run = Some(run),
            Some(existing) if existing == &run => {}
            Some(_) => return Err(AssistantWorkflowChangeError::InvalidTransition),
        }
        Ok(())
    }

    /// Records a successful single-use continuation resume idempotently.
    pub fn mark_continuation_resumed(&mut self) -> Result<(), AssistantWorkflowChangeError> {
        self.mark_continuation_outcome(AssistantContinuationOutcome::Resumed)
    }

    /// Records a non-replayable continuation interruption idempotently.
    pub fn mark_continuation_interrupted(&mut self) -> Result<(), AssistantWorkflowChangeError> {
        self.mark_continuation_outcome(AssistantContinuationOutcome::Interrupted)
    }

    fn mark_continuation_outcome(
        &mut self,
        outcome: AssistantContinuationOutcome,
    ) -> Result<(), AssistantWorkflowChangeError> {
        if self.state != AssistantWorkflowChangeState::Applied {
            return Err(AssistantWorkflowChangeError::InvalidTransition);
        }
        match self.continuation_outcome {
            AssistantContinuationOutcome::Pending => self.continuation_outcome = outcome,
            existing if existing == outcome => {}
            _ => return Err(AssistantWorkflowChangeError::InvalidTransition),
        }
        Ok(())
    }

    /// Marks a recoverable apply as permanently failed.
    pub fn mark_apply_failed(&mut self) -> Result<(), AssistantWorkflowChangeError> {
        self.transition_applying(AssistantWorkflowChangeState::ApplyFailed)
    }

    /// Expires an awaiting proposal once its exact boundary time is reached.
    pub fn expire(&mut self, now_epoch_ms: i64) -> Result<(), AssistantWorkflowChangeError> {
        if self.state != AssistantWorkflowChangeState::AwaitingApproval
            || now_epoch_ms < self.expires_at.epoch_ms()
        {
            return Err(AssistantWorkflowChangeError::InvalidTransition);
        }
        self.state = AssistantWorkflowChangeState::Expired;
        Ok(())
    }

    fn validate_review(
        &self,
        receipt: &AssistantReviewReceipt,
        verdict: AssistantReviewVerdict,
    ) -> Result<(), AssistantWorkflowChangeError> {
        if self.state != AssistantWorkflowChangeState::Proposed {
            return Err(AssistantWorkflowChangeError::InvalidTransition);
        }
        if receipt.change_id != self.id
            || receipt.mutation_digest != self.mutation_digest
            || receipt.verdict != verdict
            || receipt.reviewed_at.epoch_ms() >= self.expires_at.epoch_ms()
        {
            return Err(AssistantWorkflowChangeError::ReviewEvidenceInvalid);
        }
        Ok(())
    }

    fn require_unexpired_approval(
        &self,
        scope: AssistantWorkflowChangeDecisionScope,
        now_epoch_ms: i64,
    ) -> Result<(), AssistantWorkflowChangeError> {
        if self.state != AssistantWorkflowChangeState::AwaitingApproval {
            return Err(AssistantWorkflowChangeError::InvalidTransition);
        }
        if now_epoch_ms < 0 || now_epoch_ms >= self.expires_at.epoch_ms() {
            return Err(AssistantWorkflowChangeError::ApprovalExpired);
        }
        if !self.matches_decision_scope(scope) {
            return Err(AssistantWorkflowChangeError::InvalidValue);
        }
        Ok(())
    }

    fn transition_applying(
        &mut self,
        next: AssistantWorkflowChangeState,
    ) -> Result<(), AssistantWorkflowChangeError> {
        if self.state != AssistantWorkflowChangeState::Applying {
            return Err(AssistantWorkflowChangeError::InvalidTransition);
        }
        self.state = next;
        Ok(())
    }
}
