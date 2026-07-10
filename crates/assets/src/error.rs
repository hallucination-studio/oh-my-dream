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

    /// A failed operation was followed by a failed cleanup attempt.
    #[error("{operation} failed: {source}; cleanup `{path}` also failed: {cleanup}")]
    Cleanup {
        operation: &'static str,
        #[source]
        source: Box<AssetError>,
        path: std::path::PathBuf,
        cleanup: std::io::Error,
    },
}

/// Convenient result alias for asset operations.
pub type Result<T> = std::result::Result<T, AssetError>;
