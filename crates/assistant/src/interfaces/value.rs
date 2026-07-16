use projects::project::domain::ProjectId;
use uuid::{Uuid, Variant, Version};

use crate::domain::{
    AssistantApprovalScopeId, AssistantModelContinuationRef, AssistantModelInvocationId,
    AssistantRepairActivationId, AssistantSessionId, AssistantUserIntent,
    AssistantWorkflowChangeCandidate, AssistantWorkflowChangeExpiry, AssistantWorkflowChangeId,
    AssistantWorkflowChangeLineage, WorkflowRevisionBoundaryValue,
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
bounded_bytes!(AssistantWorkflowMutationProposal, 1024 * 1024);

macro_rules! selected_uuid {
    ($name:ident) => {
        #[doc = "Validated selected RFC 9562 UUIDv4 boundary bytes."]
        #[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
        pub struct $name([u8; 16]);
        impl $name {
            /// Restores only an RFC 9562 UUIDv4 selection.
            pub fn from_bytes(value: [u8; 16]) -> Result<Self, AssistantApplicationError> {
                let uuid = Uuid::from_bytes(value);
                if uuid.get_version() == Some(Version::Random)
                    && uuid.get_variant() == Variant::RFC4122
                {
                    Ok(Self(value))
                } else {
                    Err(AssistantApplicationError::ProtocolViolation)
                }
            }
            #[must_use]
            pub const fn as_bytes(self) -> [u8; 16] {
                self.0
            }
        }
    };
}

selected_uuid!(AssistantSelectedWorkflowNodeId);
selected_uuid!(AssistantSelectedAssetId);

/// Trusted bounded workspace observation and selection request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssistantWorkspaceSnapshotRequest {
    /// Authoritative Project scope.
    pub project_id: ProjectId,
    /// Authoritative Assistant Session scope.
    pub session_id: AssistantSessionId,
    /// Optional Workflow revision observed by the user.
    pub observed_workflow_revision: Option<WorkflowRevisionBoundaryValue>,
    /// Unique selected Workflow nodes.
    pub selected_node_ids: Vec<AssistantSelectedWorkflowNodeId>,
    /// Unique selected managed Assets.
    pub selected_asset_ids: Vec<AssistantSelectedAssetId>,
}

impl AssistantWorkspaceSnapshotRequest {
    /// Validates unique selection lists of at most 32 entries each.
    pub fn try_new(
        project_id: ProjectId,
        session_id: AssistantSessionId,
        observed_workflow_revision: Option<WorkflowRevisionBoundaryValue>,
        selected_node_ids: Vec<AssistantSelectedWorkflowNodeId>,
        selected_asset_ids: Vec<AssistantSelectedAssetId>,
    ) -> Result<Self, AssistantApplicationError> {
        let nodes = selected_node_ids.iter().collect::<std::collections::BTreeSet<_>>();
        let assets = selected_asset_ids.iter().collect::<std::collections::BTreeSet<_>>();
        if selected_node_ids.len() > 32
            || selected_asset_ids.len() > 32
            || nodes.len() != selected_node_ids.len()
            || assets.len() != selected_asset_ids.len()
        {
            return Err(AssistantApplicationError::ProtocolViolation);
        }
        Ok(Self {
            project_id,
            session_id,
            observed_workflow_revision,
            selected_node_ids,
            selected_asset_ids,
        })
    }
}

/// Closed authoritative capability-catalog query.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AssistantNodeCapabilityCatalogRequest {
    /// Lists bounded active contract summaries.
    List,
    /// Describes one to three exact active contract references.
    Describe { contract_refs: Vec<String> },
}

impl AssistantNodeCapabilityCatalogRequest {
    /// Validates exact description references without interpreting capability semantics.
    pub fn describe(contract_refs: Vec<String>) -> Result<Self, AssistantApplicationError> {
        let valid = (1..=3).contains(&contract_refs.len())
            && contract_refs
                .iter()
                .all(|value| !value.is_empty() && value.len() <= 256 && value.is_ascii());
        let unique = contract_refs.iter().collect::<std::collections::BTreeSet<_>>();
        if valid && unique.len() == contract_refs.len() {
            Ok(Self::Describe { contract_refs })
        } else {
            Err(AssistantApplicationError::ProtocolViolation)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AssistantModelTurnStart {
    UserMessage(AssistantUserIntent),
    RepairActivation(AssistantRepairActivation),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssistantModelTurnRequest {
    pub project_id: ProjectId,
    pub session_id: AssistantSessionId,
    pub invocation_id: AssistantModelInvocationId,
    pub start: AssistantModelTurnStart,
    pub workspace_request: AssistantWorkspaceSnapshotRequest,
    pub workspace_snapshot: AssistantWorkspaceSnapshot,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssistantModelResumeRequest {
    pub project_id: ProjectId,
    pub session_id: AssistantSessionId,
    pub invocation_id: AssistantModelInvocationId,
    pub lineage: AssistantWorkflowChangeLineage,
    pub observed_workflow_revision: WorkflowRevisionBoundaryValue,
    pub continuation: AssistantModelContinuationEnvelope,
    pub input: AssistantModelTurnInput,
}

/// Rust-authorized immutable facts for one evaluated Workflow candidate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssistantWorkflowCandidateAuthorization {
    /// Reserved Workflow change identity.
    pub change_id: AssistantWorkflowChangeId,
    /// Authoritative Project scope.
    pub project_id: ProjectId,
    /// Authoritative Session scope.
    pub session_id: AssistantSessionId,
    /// Exact user-message or repair lineage.
    pub lineage: AssistantWorkflowChangeLineage,
    /// Reserved human approval scope.
    pub approval_scope_id: AssistantApprovalScopeId,
    /// Immutable candidate expiry.
    pub expires_at: AssistantWorkflowChangeExpiry,
}

/// Non-committing Workflow proposal evaluation request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssistantWorkflowEvaluationRequest {
    /// Trusted candidate authorization.
    pub authorization: AssistantWorkflowCandidateAuthorization,
    /// Exact required current Workflow revision.
    pub base_workflow_revision: WorkflowRevisionBoundaryValue,
    /// Strict model proposal JSON actions.
    pub proposed_mutations: Vec<AssistantWorkflowMutationProposal>,
}

#[derive(Clone, Debug)]
pub struct AssistantWorkflowApplyRequest {
    pub change: crate::domain::AssistantWorkflowChangeAggregate,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssistantWorkflowRunRequest {
    /// Authoritative Project scope.
    pub project_id: ProjectId,
    /// Approved Workflow change identity used for stable Run admission.
    pub workflow_change_id: AssistantWorkflowChangeId,
    /// Exact committed Workflow apply receipt.
    pub applied_workflow_receipt: crate::domain::AssistantWorkflowApplyReceiptBoundaryValue,
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
