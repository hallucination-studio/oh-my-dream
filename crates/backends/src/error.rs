//! Error types for the backends layer.

use thiserror::Error;

/// Errors raised by an inference backend.
#[derive(Debug, Error)]
pub enum BackendError {
    /// The backend rejected the request parameters before submission.
    #[error("invalid request for backend `{backend}`: {reason}")]
    InvalidRequest { backend: String, reason: String },

    /// A submitted task failed on the provider side.
    #[error("task `{task_id}` failed on backend `{backend}`: {reason}")]
    TaskFailed { backend: String, task_id: String, reason: String },

    /// The task handle is unknown to this backend (e.g. already reaped).
    #[error("unknown task `{task_id}` on backend `{backend}`")]
    UnknownTask { backend: String, task_id: String },

    /// The task was cancelled by request.
    #[error("task `{task_id}` on backend `{backend}` was cancelled")]
    Cancelled { backend: String, task_id: String },
}

/// Convenient result alias for backend operations.
pub type BackendResult<T> = std::result::Result<T, BackendError>;
