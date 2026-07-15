//! Closed Workflow graph construction failures.

/// Invalid frozen Workflow graph value or entity shape.
#[derive(Clone, Copy, Debug, thiserror::Error, PartialEq, Eq)]
pub enum WorkflowGraphConstructionError {
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
}
