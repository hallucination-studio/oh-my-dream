//! Structured Generation Task application and boundary failures.

/// Persistence failure exposed to Generation Task application use cases.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum GenerationTaskRepositoryError {
    /// Persisted row data violates the authoritative aggregate contract.
    #[error("Generation Task storage is corrupt")]
    Corruption,
    /// Durable storage failed without a trustworthy result.
    #[error("Generation Task storage failed")]
    StorageFailure,
    /// A Project-scoped idempotency key was reused for different immutable facts.
    #[error("Generation Task idempotency conflict")]
    IdempotencyConflict,
    /// One Node Execution was reused for different immutable facts.
    #[error("Generation Task origin conflict")]
    OriginConflict,
    /// The expected aggregate revision is no longer current.
    #[error("Generation Task optimistic revision conflict")]
    OptimisticConflict,
    /// A claimed effect cannot be consumed or rescheduled.
    #[error("Generation Task effect claim conflict")]
    EffectClaimConflict,
}

/// Exact provider-registry resolution failure before an external call.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum GenerationProviderRegistryError {
    /// Persisted provider identity is not registered.
    #[error("Generation Provider is not registered")]
    ProviderNotFound,
    /// Registered provider does not contribute the task request kind.
    #[error("Generation Provider capability is not registered")]
    CapabilityNotFound,
    /// Persisted route identity is not registered for the request kind.
    #[error("Generation Provider route is not registered")]
    RouteNotFound,
    /// Resolved route kind does not match the immutable request.
    #[error("Generation Provider route kind does not match")]
    RequestKindMismatch,
}

/// Failure category shared by focused Asset, Workflow, and origin ports.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum GenerationTaskBoundaryError {
    /// Safe delivery may be retried without repeating submission.
    #[error("Generation Task boundary is transiently unavailable")]
    Transient,
    /// Boundary data or behavior is permanently invalid.
    #[error("Generation Task boundary failed permanently")]
    Permanent,
}

/// Failure returned by a Generation Task application use case.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum GenerationTaskApplicationError {
    /// A bounded command or query argument is invalid.
    #[error("Generation Task argument is invalid")]
    InvalidArgument,
    /// Requested Project-scoped task is absent.
    #[error("Generation Task is not found")]
    TaskNotFound,
    /// A domain value or state transition was rejected.
    #[error(transparent)]
    Domain(#[from] crate::generation_task::domain::GenerationTaskDomainError),
    /// Persistence rejected or failed the operation.
    #[error(transparent)]
    Repository(#[from] GenerationTaskRepositoryError),
    /// Provider registry cannot resolve the immutable target.
    #[error(transparent)]
    ProviderRegistry(#[from] GenerationProviderRegistryError),
    /// A focused external boundary failed.
    #[error(transparent)]
    Boundary(#[from] GenerationTaskBoundaryError),
    /// Claimed effect kind or task identity does not match the use case.
    #[error("Generation Task effect is invalid for this use case")]
    InvalidEffect,
}
