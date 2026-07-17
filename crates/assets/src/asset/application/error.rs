//! Asset application failure categories.

/// Stable failures exposed by Asset application operations.
#[derive(Clone, Copy, Debug, thiserror::Error, PartialEq, Eq)]
pub enum AssetApplicationError {
    /// No Asset exists for the requested identity.
    #[error("asset not found")]
    NotFound,
    /// The Asset is outside the requested Project.
    #[error("asset not visible")]
    NotVisible,
    /// The Asset has a different media kind than requested.
    #[error("asset media kind mismatch")]
    MediaKindMismatch {
        /// Media kind required by the caller.
        expected: crate::asset::domain::AssetMediaKind,
        /// Media kind owned by the resolved Asset.
        observed: crate::asset::domain::AssetMediaKind,
    },
    /// Managed content has not completed finalization.
    #[error("asset content pending")]
    ContentPending,
    /// Exact managed content is unavailable.
    #[error("asset content missing")]
    ContentMissing,
    /// Supplied bytes are not valid supported media.
    #[error("invalid asset media")]
    InvalidMedia,
    /// Supplied media exceeds its documented size limit.
    #[error("asset media size limit exceeded")]
    MediaSizeLimitExceeded,
    /// Supplied bytes do not match the expected digest.
    #[error("asset content digest mismatch")]
    ContentDigestMismatch,
    /// A node output key already identifies different content.
    #[error("asset node output conflict")]
    NodeOutputConflict,
    /// Managed storage could not complete its operation.
    #[error("asset managed storage failed")]
    ManagedStorageFailed,
    /// A generated identity conflicts with existing state.
    #[error("asset identity conflict")]
    IdentityConflict,
    /// Media inspection could not complete.
    #[error("asset inspection failed")]
    InspectionFailed,
    /// Managed-content finalization could not complete.
    #[error("asset finalization failed")]
    FinalizationFailed,
    /// A preview lease has invalid values.
    #[error("asset preview lease invalid")]
    PreviewLeaseInvalid,
    /// A preview lease is no longer valid.
    #[error("asset preview lease expired")]
    PreviewLeaseExpired,
    /// A preview byte-range request is invalid.
    #[error("asset preview range invalid")]
    PreviewRangeInvalid,
    /// The caller cancelled the operation.
    #[error("asset operation cancelled")]
    Cancelled,
    /// The caller deadline elapsed.
    #[error("asset operation deadline exceeded")]
    DeadlineExceeded,
}
