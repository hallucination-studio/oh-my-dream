//! Closed Generation Task domain failures.

/// A rejected Generation Task value, restoration, or state transition.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum GenerationTaskDomainError {
    /// A UUID is not RFC 9562 version four.
    #[error("Generation Task identity is invalid")]
    InvalidIdentity,
    /// An idempotency key is empty, too long, or contains control characters.
    #[error("Generation Task idempotency key is invalid")]
    InvalidIdempotencyKey,
    /// Provider or route identity violates the frozen lower-case grammar.
    #[error("Generation Provider identity is invalid")]
    InvalidProviderIdentity,
    /// An opaque provider handle is empty, too long, or contains control characters.
    #[error("Generation Provider task handle is invalid")]
    InvalidProviderTaskHandle,
    /// Text is empty or exceeds the frozen byte bound.
    #[error("Generation Task text is invalid")]
    InvalidText,
    /// A request contains an incompatible Asset snapshot or closed value.
    #[error("Generation Task request is invalid")]
    InvalidRequest,
    /// A timestamp is negative or violates aggregate ordering.
    #[error("Generation Task timestamp is invalid")]
    InvalidTimestamp,
    /// An optimistic revision is zero or cannot be incremented.
    #[error("Generation Task revision is invalid")]
    InvalidRevision,
    /// A failure code or safe message violates its bounded contract.
    #[error("Generation Task failure is invalid")]
    InvalidFailure,
    /// Progress is greater than 100 percent.
    #[error("Generation Task progress is outside 0..=100")]
    ProgressOutOfRange,
    /// A known progress value moved backwards or became unknown.
    #[error("Generation Task progress must be monotonic")]
    ProgressRegressed,
    /// A completion result does not match the request kind.
    #[error("Generation Task result kind does not match its request")]
    ResultKindMismatch,
    /// The requested transition is not legal from the current state.
    #[error("Generation Task state transition is illegal")]
    IllegalTransition,
    /// Restored fields do not form one valid aggregate snapshot.
    #[error("Generation Task restoration invariant is violated")]
    InvalidRestoredState,
    /// Restored request hash does not match the immutable request facts.
    #[error("Generation Task request hash is invalid")]
    InvalidRequestHash,
}
