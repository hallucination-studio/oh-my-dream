//! Call-scoped capability readiness and execution contracts.

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

use projects::project::domain::ProjectId;

use super::{
    NodeCapabilityGenerationProfileRefParameterValue, NodeCapabilityNormalizedParameters,
    NodeCapabilityParameterKey, WorkflowDataType, WorkflowManagedAssetIdBoundaryValue,
    WorkflowNodeExecutionId, WorkflowNodeInputSet, WorkflowRunId,
};

/// Project-scoped pre-dispatch readiness request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeCapabilityReadinessRequest {
    /// Project whose visibility rules apply.
    pub project_id: ProjectId,
    /// Complete normalized parameters.
    pub normalized_parameters: NodeCapabilityNormalizedParameters,
}

/// Closed readiness category ordered by frozen table order.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum NodeCapabilityReadinessCategory {
    /// Selected managed Asset is not Available and visible.
    ManagedAssetUnavailable,
    /// Selected managed Asset has a different media kind.
    ManagedAssetKindMismatch,
    /// Selected Generation Profile does not support the exact capability.
    GenerationProfileIncompatible,
    /// Selected Generation Profile is currently unavailable.
    GenerationProfileUnavailable,
    /// Selected Generation Profile availability cannot be trusted.
    GenerationProfileAvailabilityIndeterminate,
}

/// Exact external selection targeted by a readiness issue.
#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum NodeCapabilityReadinessTarget {
    /// Parameter-selected managed Asset.
    ManagedAsset {
        /// Parameter that selected the Asset.
        parameter_key: NodeCapabilityParameterKey,
        /// Engine Asset-ID boundary representation.
        asset_id: WorkflowManagedAssetIdBoundaryValue,
    },
    /// Parameter-selected Generation Profile.
    GenerationProfile {
        /// Parameter that selected the Generation Profile.
        parameter_key: NodeCapabilityParameterKey,
        /// Engine Generation Profile ref boundary representation.
        generation_profile_ref: NodeCapabilityGenerationProfileRefParameterValue,
    },
}

/// One safe, deterministically ordered external readiness issue.
#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct NodeCapabilityReadinessIssue {
    category: NodeCapabilityReadinessCategory,
    target: NodeCapabilityReadinessTarget,
    media_kind_mismatch: Option<(WorkflowDataType, WorkflowDataType)>,
}

/// Invalid readiness category, target, or detail combination.
#[derive(Clone, Copy, Debug, thiserror::Error, PartialEq, Eq)]
#[error("node capability readiness issue shape is invalid")]
pub struct NodeCapabilityReadinessIssueConstructionError;

impl NodeCapabilityReadinessIssue {
    /// Creates one issue only when category, target, and kind detail agree.
    pub fn try_new(
        category: NodeCapabilityReadinessCategory,
        target: NodeCapabilityReadinessTarget,
        media_kind_mismatch: Option<(WorkflowDataType, WorkflowDataType)>,
    ) -> Result<Self, NodeCapabilityReadinessIssueConstructionError> {
        let managed_target = matches!(target, NodeCapabilityReadinessTarget::ManagedAsset { .. });
        let profile_target =
            matches!(target, NodeCapabilityReadinessTarget::GenerationProfile { .. });
        let valid = match category {
            NodeCapabilityReadinessCategory::ManagedAssetUnavailable => {
                managed_target && media_kind_mismatch.is_none()
            }
            NodeCapabilityReadinessCategory::ManagedAssetKindMismatch => {
                managed_target
                    && media_kind_mismatch.is_some_and(|(expected, observed)| {
                        expected != observed
                            && expected != WorkflowDataType::Text
                            && observed != WorkflowDataType::Text
                    })
            }
            NodeCapabilityReadinessCategory::GenerationProfileIncompatible
            | NodeCapabilityReadinessCategory::GenerationProfileUnavailable
            | NodeCapabilityReadinessCategory::GenerationProfileAvailabilityIndeterminate => {
                profile_target && media_kind_mismatch.is_none()
            }
        };
        if !valid {
            return Err(NodeCapabilityReadinessIssueConstructionError);
        }
        Ok(Self { category, target, media_kind_mismatch })
    }

    /// Returns the closed readiness category.
    #[must_use]
    pub const fn category(&self) -> NodeCapabilityReadinessCategory {
        self.category
    }
    /// Returns the exact parameter selection.
    #[must_use]
    pub const fn target(&self) -> &NodeCapabilityReadinessTarget {
        &self.target
    }
    /// Returns expected and observed media kinds for a kind mismatch.
    #[must_use]
    pub const fn media_kind_mismatch(&self) -> Option<(WorkflowDataType, WorkflowDataType)> {
        self.media_kind_mismatch
    }
}

/// Call-scoped monotonic execution deadline.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct NodeCapabilityExecutionDeadline(Instant);

impl NodeCapabilityExecutionDeadline {
    /// Wraps an absolute monotonic deadline.
    #[must_use]
    pub const fn at(instant: Instant) -> Self {
        Self(instant)
    }
    /// Reports whether the supplied monotonic observation reached the deadline.
    #[must_use]
    pub fn is_reached_at(self, now: Instant) -> bool {
        now >= self.0
    }
}

/// Cloneable idempotent cancellation signal for one node execution.
#[derive(Clone, Debug, Default)]
pub struct NodeCapabilityExecutionCancellation(Arc<AtomicBool>);

impl NodeCapabilityExecutionCancellation {
    /// Creates an initially active signal.
    #[must_use]
    pub fn active() -> Self {
        Self::default()
    }
    /// Idempotently marks the execution cancelled.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
    /// Reports whether cancellation has been observed.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

/// Context shared with one exact capability and its provider call.
#[derive(Clone, Debug)]
pub struct WorkflowNodeExecutionContext {
    /// Project whose isolation rules apply.
    pub project_id: ProjectId,
    /// Admitted Run identity used for output idempotency.
    pub workflow_run_id: WorkflowRunId,
    /// Planned node execution identity.
    pub node_execution_id: WorkflowNodeExecutionId,
    /// Call-scoped monotonic deadline.
    pub deadline: NodeCapabilityExecutionDeadline,
    /// Call-scoped cancellation signal.
    pub cancellation: NodeCapabilityExecutionCancellation,
}

/// Exact immutable request passed to a capability implementation.
#[derive(Clone, Debug)]
pub struct NodeCapabilityExecutionRequest {
    /// Execution identity, deadline, and cancellation.
    pub context: WorkflowNodeExecutionContext,
    /// Complete normalized parameters.
    pub normalized_parameters: NodeCapabilityNormalizedParameters,
    /// Complete contract-validated runtime inputs.
    pub inputs: WorkflowNodeInputSet,
}
