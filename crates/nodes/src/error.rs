use crate::GenerationError;
use thiserror::Error;

/// Errors raised by concrete node implementations.
#[derive(Debug, Error)]
pub enum NodesError {
    /// A serialized node parameter had the wrong shape.
    #[error("invalid parameter `{name}`: {reason}")]
    InvalidParam { name: String, reason: String },

    /// A required runtime input was absent.
    #[error("missing required input `{name}`")]
    MissingInput { name: String },

    /// A runtime input had an unexpected engine value variant.
    #[error("input `{name}` expected {expected}")]
    WrongInputType { name: String, expected: &'static str },

    /// A runtime input had an unexpected cardinality.
    #[error("input `{name}` expected {expected} cardinality")]
    WrongInputCardinality { name: String, expected: &'static str },

    /// A media generation capability failed.
    #[error("{operation} failed: {source}")]
    Generation {
        operation: &'static str,
        #[source]
        source: GenerationError,
    },

    /// The shared asset store mutex could not be acquired.
    #[error("asset store lock was poisoned")]
    AssetStoreLock,

    /// Persisting an asset failed.
    #[error("asset store operation failed: {source}")]
    Asset {
        #[source]
        source: assets::AssetError,
    },

    /// A remote media output requires a resolver that is not configured.
    #[error("remote media output requires a resolver")]
    RemoteMediaOutput,

    /// Inline media carried a different modality than the node output.
    #[error("inline media kind `{actual}` cannot satisfy `{expected}` asset")]
    InlineMediaKindMismatch { actual: &'static str, expected: &'static str },

    /// Creating or writing a private inline-media file failed.
    #[error("{operation} inline media: {source}")]
    MaterializeMedia {
        operation: &'static str,
        #[source]
        source: std::io::Error,
    },
}

pub(crate) fn boxed(error: NodesError) -> engine::NodeRunError {
    Box::new(error)
}

pub(crate) fn generation_error(
    operation: &'static str,
    source: GenerationError,
) -> engine::NodeRunError {
    match source {
        GenerationError::TaskCancelled => engine::cancelled_node_run(),
        source => boxed(NodesError::Generation { operation, source }),
    }
}
