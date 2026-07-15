//! Closed failures exposed by node-capability contracts.

use thiserror::Error;

use std::time::Instant;

use super::{
    NodeCapabilityContractRef, NodeCapabilityInputKey, NodeCapabilityOutputKey,
    NodeCapabilityParameterKey, NodeCapabilityReadinessIssue, NodeCapabilityReadinessTarget,
    WorkflowNodeExecutionId,
};

/// Immutable-registry construction or lookup failure.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum NodeCapabilityRegistryError {
    /// Two implementations declared the same exact ref.
    #[error("duplicate node capability contract ref: {contract_ref}")]
    DuplicateContractRef {
        /// Duplicated exact contract identity.
        contract_ref: NodeCapabilityContractRef,
    },
    /// No active implementation had the requested exact ref.
    #[error("node capability contract is not registered: {contract_ref}")]
    ContractNotRegistered {
        /// Missing exact contract identity.
        contract_ref: NodeCapabilityContractRef,
    },
}

/// Closed provider failure category shared by exact generation interfaces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeCapabilityProviderFailureCategory {
    /// Semantic request was invalid.
    InvalidSemanticRequest,
    /// Provider authentication failed.
    AuthenticationFailed,
    /// Provider denied the operation.
    PermissionDenied,
    /// Content policy rejected the request.
    ContentPolicyRejected,
    /// Provider rate limit was reached.
    RateLimited,
    /// Provider was temporarily unavailable.
    ProviderUnavailable,
    /// Operation deadline was exceeded.
    DeadlineExceeded,
    /// Provider rejected an otherwise valid operation.
    ProviderRejected,
    /// Provider response was invalid.
    InvalidResponse,
    /// Provider content download was rejected.
    DownloadRejected,
    /// Submission outcome could not be proven.
    AmbiguousSubmission,
}

/// Validated provider failure without provider-private text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeCapabilityProviderFailure {
    category: NodeCapabilityProviderFailureCategory,
    retryable: bool,
    safe_retry_at: Option<Instant>,
}

/// Invalid provider retry metadata for its closed category.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
#[error("node capability provider retry metadata is invalid")]
pub struct NodeCapabilityProviderFailureConstructionError;

impl NodeCapabilityProviderFailure {
    /// Creates a provider failure and rejects inconsistent retry metadata.
    pub fn try_new(
        category: NodeCapabilityProviderFailureCategory,
        submission_was_accepted: bool,
        observed_at: Instant,
        safe_retry_at: Option<Instant>,
    ) -> Result<Self, NodeCapabilityProviderFailureConstructionError> {
        let retryable = matches!(
            category,
            NodeCapabilityProviderFailureCategory::RateLimited
                | NodeCapabilityProviderFailureCategory::ProviderUnavailable
        ) || (category == NodeCapabilityProviderFailureCategory::DeadlineExceeded
            && !submission_was_accepted);
        if (!retryable && safe_retry_at.is_some())
            || safe_retry_at.is_some_and(|retry_at| retry_at <= observed_at)
        {
            return Err(NodeCapabilityProviderFailureConstructionError);
        }
        Ok(Self { category, retryable, safe_retry_at })
    }

    /// Returns the closed provider failure category.
    #[must_use]
    pub const fn category(&self) -> NodeCapabilityProviderFailureCategory {
        self.category
    }

    /// Reports whether a new Run may safely retry the semantic operation.
    #[must_use]
    pub const fn is_retryable(&self) -> bool {
        self.retryable
    }

    /// Returns the optional monotonic instant before which retry is unsafe.
    #[must_use]
    pub const fn safe_retry_at(&self) -> Option<Instant> {
        self.safe_retry_at
    }
}

/// Closed managed-media failure category.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeCapabilityMediaFailure {
    /// Selected managed media was unavailable.
    Unavailable,
    /// Selected managed media had the wrong kind.
    KindMismatch {
        /// Media kind required by the caller.
        expected: super::WorkflowDataType,
        /// Media kind observed at the boundary.
        observed: super::WorkflowDataType,
    },
    /// Media content failed validation.
    InvalidMedia,
    /// Media exceeded its exact size limit.
    SizeLimitExceeded,
    /// Content digest did not match verified bytes.
    DigestMismatch,
    /// One output key was reused with different content.
    OutputConflict,
    /// Managed storage failed.
    StorageFailed,
    /// Media inspection failed.
    InspectionFailed,
    /// Managed-media finalization failed.
    FinalizationFailed,
}

/// Stage at which exact capability execution failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeCapabilityExecutionStage {
    /// Runtime input or external selection resolution.
    ResolveInputs,
    /// Exact provider interface invocation.
    CallProvider,
    /// Provider result validation.
    ValidateProviderResult,
    /// Managed-media output publication.
    WriteManagedMedia,
}

/// Safe structured target of an execution failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NodeCapabilityExecutionTarget {
    /// Capability operation as a whole.
    Capability,
    /// One declared parameter.
    Parameter(NodeCapabilityParameterKey),
    /// One declared input.
    Input(NodeCapabilityInputKey),
    /// One declared output.
    Output(NodeCapabilityOutputKey),
}

/// Closed execution failure source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NodeCapabilityExecutionFailure {
    /// Direct execution request did not satisfy the resolved capability contract.
    InvalidCapabilityInvocation,
    /// External readiness changed after admission.
    Readiness(NodeCapabilityReadinessIssue),
    /// Provider boundary failed.
    Provider(NodeCapabilityProviderFailure),
    /// Managed-media boundary failed.
    Media(NodeCapabilityMediaFailure),
    /// Cancellation was observed.
    Cancelled,
    /// Call-scoped deadline was observed.
    DeadlineExceeded,
}

/// Invalid combination of execution stage and target.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
#[error("node capability execution stage and target are inconsistent")]
pub struct NodeCapabilityExecutionErrorConstructionError;

/// Structured safe failure returned by one capability execution.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
#[error("node capability execution failed")]
pub struct NodeCapabilityExecutionError {
    contract_ref: NodeCapabilityContractRef,
    node_execution_id: WorkflowNodeExecutionId,
    stage: NodeCapabilityExecutionStage,
    failure: NodeCapabilityExecutionFailure,
    target: NodeCapabilityExecutionTarget,
}

impl NodeCapabilityExecutionError {
    /// Creates the fixed error for a malformed direct capability invocation.
    #[must_use]
    pub fn invalid_capability_invocation(
        contract_ref: NodeCapabilityContractRef,
        node_execution_id: WorkflowNodeExecutionId,
    ) -> Self {
        Self::resolve_inputs_capability_failure(
            contract_ref,
            node_execution_id,
            NodeCapabilityExecutionFailure::InvalidCapabilityInvocation,
        )
    }

    /// Creates a cancellation observed while resolving capability inputs.
    #[must_use]
    pub fn cancelled_while_resolving_inputs(
        contract_ref: NodeCapabilityContractRef,
        node_execution_id: WorkflowNodeExecutionId,
    ) -> Self {
        Self::resolve_inputs_capability_failure(
            contract_ref,
            node_execution_id,
            NodeCapabilityExecutionFailure::Cancelled,
        )
    }

    /// Creates a deadline failure observed while resolving capability inputs.
    #[must_use]
    pub fn deadline_exceeded_while_resolving_inputs(
        contract_ref: NodeCapabilityContractRef,
        node_execution_id: WorkflowNodeExecutionId,
    ) -> Self {
        Self::resolve_inputs_capability_failure(
            contract_ref,
            node_execution_id,
            NodeCapabilityExecutionFailure::DeadlineExceeded,
        )
    }

    /// Creates cancellation observed while resolving one parameter.
    #[must_use]
    pub fn cancelled_while_resolving_parameter(
        contract_ref: NodeCapabilityContractRef,
        node_execution_id: WorkflowNodeExecutionId,
        parameter_key: NodeCapabilityParameterKey,
    ) -> Self {
        Self {
            contract_ref,
            node_execution_id,
            stage: NodeCapabilityExecutionStage::ResolveInputs,
            failure: NodeCapabilityExecutionFailure::Cancelled,
            target: NodeCapabilityExecutionTarget::Parameter(parameter_key),
        }
    }

    /// Creates deadline failure observed while resolving one parameter.
    #[must_use]
    pub fn deadline_exceeded_while_resolving_parameter(
        contract_ref: NodeCapabilityContractRef,
        node_execution_id: WorkflowNodeExecutionId,
        parameter_key: NodeCapabilityParameterKey,
    ) -> Self {
        Self {
            contract_ref,
            node_execution_id,
            stage: NodeCapabilityExecutionStage::ResolveInputs,
            failure: NodeCapabilityExecutionFailure::DeadlineExceeded,
            target: NodeCapabilityExecutionTarget::Parameter(parameter_key),
        }
    }

    /// Creates an exact managed-media failure for one Asset parameter resolution.
    #[must_use]
    pub fn managed_media_parameter_resolution_failed(
        contract_ref: NodeCapabilityContractRef,
        node_execution_id: WorkflowNodeExecutionId,
        parameter_key: NodeCapabilityParameterKey,
        failure: NodeCapabilityMediaFailure,
    ) -> Self {
        Self {
            contract_ref,
            node_execution_id,
            stage: NodeCapabilityExecutionStage::ResolveInputs,
            failure: NodeCapabilityExecutionFailure::Media(failure),
            target: NodeCapabilityExecutionTarget::Parameter(parameter_key),
        }
    }

    fn resolve_inputs_capability_failure(
        contract_ref: NodeCapabilityContractRef,
        node_execution_id: WorkflowNodeExecutionId,
        failure: NodeCapabilityExecutionFailure,
    ) -> Self {
        Self {
            contract_ref,
            node_execution_id,
            stage: NodeCapabilityExecutionStage::ResolveInputs,
            failure,
            target: NodeCapabilityExecutionTarget::Capability,
        }
    }
    /// Creates an error only when its stage and target agree.
    pub fn try_new(
        contract_ref: NodeCapabilityContractRef,
        node_execution_id: WorkflowNodeExecutionId,
        stage: NodeCapabilityExecutionStage,
        failure: NodeCapabilityExecutionFailure,
        target: NodeCapabilityExecutionTarget,
    ) -> Result<Self, NodeCapabilityExecutionErrorConstructionError> {
        let valid_target = match stage {
            NodeCapabilityExecutionStage::ResolveInputs => matches!(
                target,
                NodeCapabilityExecutionTarget::Capability
                    | NodeCapabilityExecutionTarget::Parameter(_)
                    | NodeCapabilityExecutionTarget::Input(_)
            ),
            NodeCapabilityExecutionStage::CallProvider => {
                matches!(target, NodeCapabilityExecutionTarget::Capability)
            }
            NodeCapabilityExecutionStage::ValidateProviderResult
            | NodeCapabilityExecutionStage::WriteManagedMedia => matches!(
                target,
                NodeCapabilityExecutionTarget::Capability
                    | NodeCapabilityExecutionTarget::Output(_)
            ),
        };
        let readiness_target_matches = match (&failure, &target) {
            (
                NodeCapabilityExecutionFailure::Readiness(issue),
                NodeCapabilityExecutionTarget::Parameter(execution_parameter_key),
            ) => readiness_parameter_key(issue).is_some_and(|key| key == execution_parameter_key),
            (NodeCapabilityExecutionFailure::Readiness(_), _) => false,
            _ => true,
        };
        let invalid_invocation_shape = failure
            == NodeCapabilityExecutionFailure::InvalidCapabilityInvocation
            && (stage != NodeCapabilityExecutionStage::ResolveInputs
                || target != NodeCapabilityExecutionTarget::Capability);
        if !valid_target || !readiness_target_matches || invalid_invocation_shape {
            return Err(NodeCapabilityExecutionErrorConstructionError);
        }
        Ok(Self { contract_ref, node_execution_id, stage, failure, target })
    }

    /// Returns the exact capability contract that failed.
    #[must_use]
    pub const fn contract_ref(&self) -> &NodeCapabilityContractRef {
        &self.contract_ref
    }
    /// Returns the planned node execution that failed.
    #[must_use]
    pub const fn node_execution_id(&self) -> WorkflowNodeExecutionId {
        self.node_execution_id
    }
    /// Returns the exact failure stage.
    #[must_use]
    pub const fn stage(&self) -> NodeCapabilityExecutionStage {
        self.stage
    }
    /// Returns the closed failure source.
    #[must_use]
    pub const fn failure(&self) -> &NodeCapabilityExecutionFailure {
        &self.failure
    }
    /// Returns the safe structured failure target.
    #[must_use]
    pub const fn target(&self) -> &NodeCapabilityExecutionTarget {
        &self.target
    }
}

fn readiness_parameter_key(
    issue: &NodeCapabilityReadinessIssue,
) -> Option<&NodeCapabilityParameterKey> {
    match issue.target() {
        NodeCapabilityReadinessTarget::Capability => None,
        NodeCapabilityReadinessTarget::ManagedAsset { parameter_key, .. }
        | NodeCapabilityReadinessTarget::GenerationProfile { parameter_key, .. } => {
            Some(parameter_key)
        }
    }
}
