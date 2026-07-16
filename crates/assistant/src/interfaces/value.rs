use projects::project::domain::ProjectId;

use crate::domain::{
    AssistantModelContinuationRef, AssistantModelInvocationId, AssistantRepairActivationId,
    AssistantSessionId, AssistantUserIntent, AssistantWorkflowChangeCandidate,
    AssistantWorkflowChangeId, AssistantWorkflowMutation, WorkflowRevisionBoundaryValue,
};

/// Closed Assistant application and boundary failures.
#[derive(Clone, Copy, Debug, thiserror::Error, PartialEq, Eq)]
pub enum AssistantApplicationError {
    #[error("Assistant value not found")]
    NotFound,
    #[error("Assistant value is not visible")]
    NotVisible,
    #[error("Assistant revision conflict")]
    RevisionConflict,
    #[error("Assistant transition is invalid")]
    InvalidTransition,
    #[error("Assistant invocation is already active")]
    ConcurrentInvocation,
    #[error("Assistant Session already has a pending approval")]
    PendingApprovalExists,
    #[error("Assistant Workflow revision is stale")]
    StaleWorkflowRevision,
    #[error("Assistant approval does not match")]
    ApprovalMismatch,
    #[error("Assistant approval expired")]
    ApprovalExpired,
    #[error("Assistant review evidence is invalid")]
    ReviewEvidenceInvalid,
    #[error("Assistant candidate fingerprint does not match")]
    CandidateFingerprintMismatch,
    #[error("Assistant continuation is incompatible")]
    ContinuationIncompatible,
    #[error("Assistant continuation was interrupted")]
    ContinuationInterrupted,
    #[error("Assistant model is unavailable")]
    ModelUnavailable,
    #[error("Assistant protocol violation")]
    ProtocolViolation,
    #[error("Assistant budget exceeded")]
    BudgetExceeded,
    #[error("Assistant deadline exceeded")]
    DeadlineExceeded,
    #[error("Assistant external boundary failed")]
    ExternalBoundaryFailed,
}

macro_rules! bounded_bytes {
    ($name:ident, $max:expr) => {
        #[derive(Clone, Debug, PartialEq, Eq)]
        pub struct $name(Vec<u8>);
        impl $name {
            pub fn new(value: Vec<u8>) -> Result<Self, AssistantApplicationError> {
                if value.is_empty() || value.len() > $max {
                    Err(AssistantApplicationError::ProtocolViolation)
                } else {
                    Ok(Self(value))
                }
            }
            #[must_use]
            pub fn as_bytes(&self) -> &[u8] {
                &self.0
            }
        }
    };
}

bounded_bytes!(AssistantWorkspaceSnapshot, 1024 * 1024);
bounded_bytes!(AssistantNodeCapabilityCatalogSnapshot, 1024 * 1024);
bounded_bytes!(AssistantModelTurnInput, 1024 * 1024);
bounded_bytes!(AssistantModelTurnResult, 16 * 1024 * 1024);
bounded_bytes!(AssistantModelContinuationEnvelope, 4 * 1024 * 1024);
bounded_bytes!(AssistantWorkflowApplyReceiptBoundaryValue, 1024 * 1024);
bounded_bytes!(AssistantWorkflowRunBoundaryValue, 1024 * 1024);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssistantModelTurnRequest {
    pub project_id: ProjectId,
    pub session_id: AssistantSessionId,
    pub invocation_id: AssistantModelInvocationId,
    pub intent: AssistantUserIntent,
    pub workspace_snapshot: AssistantWorkspaceSnapshot,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssistantModelResumeRequest {
    pub project_id: ProjectId,
    pub session_id: AssistantSessionId,
    pub invocation_id: AssistantModelInvocationId,
    pub continuation: AssistantModelContinuationEnvelope,
    pub input: AssistantModelTurnInput,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssistantWorkflowEvaluationRequest {
    pub project_id: ProjectId,
    pub session_id: AssistantSessionId,
    pub base_workflow_revision: WorkflowRevisionBoundaryValue,
    pub ordered_mutations: Vec<AssistantWorkflowMutation>,
}

#[derive(Clone, Debug)]
pub struct AssistantWorkflowApplyRequest {
    pub change: crate::domain::AssistantWorkflowChangeAggregate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AssistantWorkflowRunRequest {
    pub project_id: ProjectId,
    pub workflow_change_id: AssistantWorkflowChangeId,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct AssistantFailedWorkflowRunId(pub [u8; 16]);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssistantRepairActivation {
    id: AssistantRepairActivationId,
    project_id: ProjectId,
    session_id: AssistantSessionId,
    failed_workflow_run_id: AssistantFailedWorkflowRunId,
    exact_failed_run_facts: Vec<u8>,
    created_at_epoch_ms: i64,
}

impl AssistantRepairActivation {
    pub fn new(
        id: AssistantRepairActivationId,
        project_id: ProjectId,
        session_id: AssistantSessionId,
        failed_workflow_run_id: AssistantFailedWorkflowRunId,
        exact_failed_run_facts: Vec<u8>,
        created_at_epoch_ms: i64,
    ) -> Result<Self, AssistantApplicationError> {
        if exact_failed_run_facts.is_empty()
            || exact_failed_run_facts.len() > 1024 * 1024
            || created_at_epoch_ms < 0
        {
            return Err(AssistantApplicationError::ProtocolViolation);
        }
        Ok(Self {
            id,
            project_id,
            session_id,
            failed_workflow_run_id,
            exact_failed_run_facts,
            created_at_epoch_ms,
        })
    }

    #[must_use]
    pub const fn id(&self) -> AssistantRepairActivationId {
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
    pub const fn failed_workflow_run_id(&self) -> AssistantFailedWorkflowRunId {
        self.failed_workflow_run_id
    }
    #[must_use]
    pub fn exact_failed_run_facts(&self) -> &[u8] {
        &self.exact_failed_run_facts
    }
    #[must_use]
    pub const fn created_at_epoch_ms(&self) -> i64 {
        self.created_at_epoch_ms
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AssistantRepairActivationRecordResult {
    Created(AssistantRepairActivation),
    Existing(AssistantRepairActivation),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssistantStoredContinuation {
    pub continuation_ref: AssistantModelContinuationRef,
    pub project_id: ProjectId,
    pub session_id: AssistantSessionId,
    pub invocation_id: AssistantModelInvocationId,
    pub envelope: AssistantModelContinuationEnvelope,
}

#[derive(Clone, Debug)]
pub struct AssistantWorkflowEvaluationResult {
    pub candidate: AssistantWorkflowChangeCandidate,
}
