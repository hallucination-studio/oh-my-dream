//! Authoritative Generation Task lifecycle state.

use super::{GenerationProviderTaskHandle, GenerationTaskFailure, GenerationTaskTimestamp};

/// Durable Generation Task lifecycle state.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum GenerationTaskState {
    /// Durable intent exists but submission has not begun.
    Queued,
    /// Submission may be in flight and must not be blindly repeated.
    Submitting,
    /// Accepted remote work is being observed.
    Running {
        /// Opaque accepted provider identity.
        handle: GenerationProviderTaskHandle,
        /// Optional normalized progress in `0..=100`.
        progress_percent: Option<u8>,
    },
    /// Workflow cancellation intent won while external work may exist.
    CancelRequested {
        /// Accepted handle when known after a submission race.
        handle: Option<GenerationProviderTaskHandle>,
    },
    /// One valid result committed atomically with completion.
    Succeeded {
        /// Durable completion time.
        completed_at: GenerationTaskTimestamp,
    },
    /// One structured terminal failure committed.
    Failed {
        /// Durable completion time.
        completed_at: GenerationTaskTimestamp,
        /// Safe structured failure.
        failure: GenerationTaskFailure,
    },
    /// Cancellation committed without a result.
    Cancelled {
        /// Durable completion time.
        completed_at: GenerationTaskTimestamp,
    },
}

impl GenerationTaskState {
    /// Reports whether no further transition is legal.
    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        matches!(self, Self::Succeeded { .. } | Self::Failed { .. } | Self::Cancelled { .. })
    }

    /// Returns known running progress.
    #[must_use]
    pub const fn progress_percent(&self) -> Option<u8> {
        match self {
            Self::Running { progress_percent, .. } => *progress_percent,
            _ => None,
        }
    }

    /// Returns an accepted provider handle when one is durable.
    #[must_use]
    pub const fn remote_handle(&self) -> Option<&GenerationProviderTaskHandle> {
        match self {
            Self::Running { handle, .. } | Self::CancelRequested { handle: Some(handle) } => {
                Some(handle)
            }
            _ => None,
        }
    }

    pub(super) const fn completed_at(&self) -> Option<GenerationTaskTimestamp> {
        match self {
            Self::Succeeded { completed_at }
            | Self::Failed { completed_at, .. }
            | Self::Cancelled { completed_at } => Some(*completed_at),
            _ => None,
        }
    }
}
