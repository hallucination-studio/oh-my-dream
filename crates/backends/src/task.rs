//! Task handles and status for asynchronous generation.
//!
//! Cloud providers are typically asynchronous: submit, then poll. We model that
//! shape uniformly even for the mock, so swapping in a real backend later does
//! not change the calling code.

use serde::{Deserialize, Serialize};

/// An opaque handle to a submitted generation task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskHandle {
    /// Identifier of the backend that owns this task.
    pub backend: String,
    /// Provider-scoped task identifier.
    pub task_id: String,
}

/// Progress of a running task, in `[0.0, 1.0]` when known.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TaskProgress(pub f32);

/// The current status of a submitted task.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum TaskStatus {
    /// Accepted, not yet started.
    Queued,
    /// Running, with best-effort progress.
    Running { progress: TaskProgress },
    /// Finished; `output` references the produced media (asset id / URL).
    Succeeded { output: String },
    /// Failed with a human-readable reason.
    Failed { reason: String },
    /// Cancelled by request.
    Cancelled,
}
