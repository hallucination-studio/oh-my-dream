use backends::BackendError;
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

    /// A backend operation failed.
    #[error("{operation} failed: {source}")]
    Backend {
        operation: &'static str,
        #[source]
        source: BackendError,
    },

    /// A backend task reached a failed terminal status.
    #[error("task `{task_id}` on backend `{backend}` failed: {reason}")]
    TaskFailed { backend: String, task_id: String, reason: String },

    /// A backend task reached a cancelled terminal status.
    #[error("task `{task_id}` on backend `{backend}` was cancelled")]
    TaskCancelled { backend: String, task_id: String },

    /// Polling exceeded the node's bounded wait policy.
    #[error("task `{task_id}` on backend `{backend}` did not complete after {max_polls} polls")]
    PollLimit { backend: String, task_id: String, max_polls: usize },

    /// The shared asset store mutex could not be acquired.
    #[error("asset store lock was poisoned")]
    AssetStoreLock,

    /// Persisting an asset failed.
    #[error("asset store operation failed: {source}")]
    Asset {
        #[source]
        source: assets::AssetError,
    },

    /// A media reference could not be materialized for the local asset store.
    #[error("materialize media reference `{reference}`: {message}")]
    MaterializeMedia { reference: String, message: String },
}

pub(crate) fn boxed(error: NodesError) -> engine::NodeRunError {
    Box::new(error)
}
