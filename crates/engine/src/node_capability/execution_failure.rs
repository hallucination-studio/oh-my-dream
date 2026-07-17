//! Closed failure values for exact node-capability execution.

use thiserror::Error;

use super::{
    NodeCapabilityInputKey, NodeCapabilityOutputKey, NodeCapabilityParameterKey,
    NodeCapabilityProviderFailure, NodeCapabilityReadinessIssue, NodeCapabilityReadinessTarget,
};

pub(super) fn readiness_parameter_key(
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

/// Closed safe durable Generation Task start failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeCapabilityGenerationTaskStartFailure {
    /// The translated request was not valid.
    InvalidRequest,
    /// The idempotency coordinates conflict with different facts.
    Conflict,
    /// Task admission is currently unavailable.
    Unavailable,
    /// Cancellation was observed before durable admission.
    Cancelled,
    /// Deadline was reached before durable admission.
    DeadlineExceeded,
    /// Durable Task persistence failed.
    Persistence,
}

/// Stage at which exact capability execution failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeCapabilityExecutionStage {
    /// Runtime input or external selection resolution.
    ResolveInputs,
    /// Durable Generation Task admission.
    StartGenerationTask,
    /// Exact provider interface invocation.
    CallProvider,
    /// Provider result validation.
    ValidateProviderResult,
    /// Managed-media output publication.
    WriteManagedMedia,
    /// Final contract-complete output-set assembly.
    AssembleOutputs,
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
    /// Capability-owned fixed result construction violated its own contract.
    InvalidCapabilityResult,
    /// External readiness changed after admission.
    Readiness(NodeCapabilityReadinessIssue),
    /// Provider boundary failed.
    Provider(NodeCapabilityProviderFailure),
    /// Durable Generation Task admission failed safely.
    GenerationTaskStart(NodeCapabilityGenerationTaskStartFailure),
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
