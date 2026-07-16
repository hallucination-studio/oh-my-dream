//! Closed Workflow graph and mutation domain failures.

/// Invalid frozen Workflow graph value or entity shape.
#[derive(Clone, Copy, Debug, thiserror::Error, PartialEq, Eq)]
pub enum WorkflowGraphError {
    /// A mutation command contains zero or more than 128 actions.
    #[error("workflow mutation action limit is exceeded")]
    ActionLimitExceeded,
    /// Requested base revision is not the aggregate's current revision.
    #[error("workflow revision conflicts with the mutation base revision")]
    RevisionConflict,
    /// Current revision cannot advance by exactly one.
    #[error("workflow revision overflowed")]
    RevisionOverflow,
    /// Updated timestamp cannot advance monotonically.
    #[error("workflow timestamp overflowed")]
    TimestampOverflow,
    /// One mutation request identity was reused with different command content.
    #[error("workflow mutation request conflicts with its receipt")]
    MutationIdempotencyConflict,
    /// Persisted Workflow mutation receipt integrity validation failed.
    #[error("workflow persistence integrity validation failed")]
    PersistenceFailure,
    /// Schema version is zero or not the hard-cut version.
    #[error("workflow schema version is unsupported")]
    SchemaVersionUnsupported,
    /// Revision is zero.
    #[error("workflow revision must be non-zero")]
    RevisionZero,
    /// An identity is not an RFC 9562 UUIDv4.
    #[error("workflow identity is not UUIDv4")]
    IdentityNotVersionFour,
    /// A timestamp is negative.
    #[error("workflow timestamp is out of range")]
    TimestampOutOfRange,
    /// A canvas coordinate is non-finite or outside the frozen bounds.
    #[error("workflow canvas position is out of bounds")]
    CanvasPositionOutOfBounds,
    /// A single item declared a role or an ordered item omitted one.
    #[error("workflow input binding shape does not match item roles")]
    BindingShapeMismatch,
    /// An ordered binding has no items.
    #[error("workflow ordered reference binding must be non-empty")]
    CardinalityViolation,
    /// Two restored nodes use the same Workflow-local identity.
    #[error("workflow contains a duplicate node")]
    DuplicateNode,
    /// Two restored edge items use the same stable identity.
    #[error("workflow contains a duplicate input item")]
    DuplicateInputItem,
    /// A node referenced by a binding does not exist.
    #[error("workflow node was not found")]
    NodeNotFound,
    /// A target input is not declared by its exact capability.
    #[error("workflow input was not found")]
    InputNotFound,
    /// An addressed stable item does not exist in the target binding.
    #[error("workflow input item was not found")]
    InputItemNotFound,
    /// A target input already has a binding.
    #[error("workflow input is occupied")]
    InputOccupied,
    /// A source output is not declared by its exact capability.
    #[error("workflow output was not found")]
    OutputNotFound,
    /// Source and target are the same node.
    #[error("workflow self edge is forbidden")]
    SelfEdge,
    /// Source output and target input types are incompatible.
    #[error("workflow input data type does not match")]
    DataTypeMismatch,
    /// An ordered item role is absent, undeclared, or incompatible.
    #[error("workflow input role is invalid")]
    RoleViolation,
    /// The graph contains a directed cycle.
    #[error("workflow graph contains a cycle")]
    Cycle,
    /// A capability contract or opaque parameter reference is invalid.
    #[error("workflow contains an invalid capability reference")]
    ReferenceViolation,
    /// Canonical mutation action bytes are malformed or non-canonical.
    #[error("workflow canonical mutation action is invalid")]
    CanonicalMutationInvalid,
}
