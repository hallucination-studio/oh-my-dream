//! Closed Asset domain invariant failures.

/// Exact domain failure categories owned by the Asset aggregate and values.
#[derive(Clone, Copy, Debug, thiserror::Error, PartialEq, Eq)]
pub enum AssetDomainError {
    /// An Asset-owned UUID is not RFC 9562 version four.
    #[error("asset identity is invalid")]
    InvalidIdentity,
    /// Display name violates its normalized text contract.
    #[error("asset display name is invalid")]
    InvalidDisplayName,
    /// Original file name is invalid or contains a path separator.
    #[error("asset original file name is invalid")]
    InvalidOriginalFileName,
    /// Content descriptor fields disagree or exceed the media contract.
    #[error("asset content descriptor is invalid")]
    InvalidDescriptor,
    /// Inspected media facts violate their exact kind-specific bounds.
    #[error("asset media facts are invalid")]
    InvalidMediaFacts,
    /// Asset provenance is incomplete or inconsistent.
    #[error("asset origin is invalid")]
    InvalidOrigin,
    /// Requested managed-content transition is not legal.
    #[error("asset managed content transition is invalid")]
    InvalidTransition,
    /// Finalization identity does not match the Pending state.
    #[error("asset content finalization identity does not match")]
    FinalizationIdentityMismatch,
}
