//! Error types for the asset library.

use thiserror::Error;

/// Errors raised while storing or querying assets.
#[derive(Debug, Error)]
pub enum AssetError {
    /// No asset exists with the given id.
    #[error("asset `{id}` not found")]
    NotFound { id: String },

    /// A storage-layer failure (filesystem or database). Message is contextual.
    #[error("asset storage failure: {message}")]
    Storage { message: String },

    /// Thumbnail generation failed for an asset.
    #[error("failed to generate thumbnail for asset `{id}`: {message}")]
    Thumbnail { id: String, message: String },
}

/// Convenient result alias for asset operations.
pub type Result<T> = std::result::Result<T, AssetError>;
