//! Project-scoped Generation Task query and projection values.

use projects::project::domain::ProjectId;

use super::GenerationTaskApplicationError;
use crate::generation_task::domain::{
    GenerationTaskAggregate, GenerationTaskId, GenerationTaskOrigin, GenerationTaskRequestKind,
    GenerationTaskState, GenerationTaskTimestamp,
};

/// Stable task-list status projection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GenerationTaskStatus {
    /// Queued before submission.
    Queued,
    /// Submission or accepted work is active.
    Running,
    /// Cancellation convergence is active.
    CancelRequested,
    /// Completed successfully.
    Succeeded,
    /// Completed with failure.
    Failed,
    /// Completed by cancellation.
    Cancelled,
}

impl GenerationTaskStatus {
    /// Projects the authoritative state without redefining transitions.
    #[must_use]
    pub const fn from_state(state: &GenerationTaskState) -> Self {
        match state {
            GenerationTaskState::Queued => Self::Queued,
            GenerationTaskState::Submitting | GenerationTaskState::Running { .. } => Self::Running,
            GenerationTaskState::CancelRequested { .. } => Self::CancelRequested,
            GenerationTaskState::Succeeded { .. } => Self::Succeeded,
            GenerationTaskState::Failed { .. } => Self::Failed,
            GenerationTaskState::Cancelled { .. } => Self::Cancelled,
        }
    }
}

/// Rule-free bounded task-list projection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GenerationTaskSummaryView {
    /// Task identity.
    pub id: GenerationTaskId,
    /// Exact origin coordinates.
    pub origin: GenerationTaskOrigin,
    /// Request-owned kind.
    pub request_kind: GenerationTaskRequestKind,
    /// Normalized current status.
    pub status: GenerationTaskStatus,
    /// Known normalized progress.
    pub progress_percent: Option<u8>,
    /// Whether a terminal result is present.
    pub has_result: bool,
    /// Creation time used for stable ordering.
    pub created_at: GenerationTaskTimestamp,
    /// Latest transition time.
    pub updated_at: GenerationTaskTimestamp,
}

impl GenerationTaskSummaryView {
    /// Mechanically projects one aggregate.
    #[must_use]
    pub fn from_task(task: &GenerationTaskAggregate) -> Self {
        Self {
            id: task.id(),
            origin: task.origin().clone(),
            request_kind: task.request().kind(),
            status: GenerationTaskStatus::from_state(task.state()),
            progress_percent: task.progress_percent(),
            has_result: task.result().is_some(),
            created_at: task.created_at(),
            updated_at: task.updated_at(),
        }
    }
}

/// Stable descending keyset cursor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GenerationTaskListCursor {
    /// Last item creation time.
    pub created_at: GenerationTaskTimestamp,
    /// Last item identity.
    pub task_id: GenerationTaskId,
}

/// Bounded Project-scoped list query.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GenerationTaskListQuery {
    project_id: ProjectId,
    status: Option<GenerationTaskStatus>,
    request_kind: Option<GenerationTaskRequestKind>,
    cursor: Option<GenerationTaskListCursor>,
    limit: u8,
}

impl GenerationTaskListQuery {
    /// Creates a Project-scoped query with a `1..=100` page size.
    pub const fn try_new(
        project_id: ProjectId,
        status: Option<GenerationTaskStatus>,
        request_kind: Option<GenerationTaskRequestKind>,
        cursor: Option<GenerationTaskListCursor>,
        limit: u8,
    ) -> Result<Self, GenerationTaskApplicationError> {
        if limit == 0 || limit > 100 {
            return Err(GenerationTaskApplicationError::InvalidArgument);
        }
        Ok(Self { project_id, status, request_kind, cursor, limit })
    }

    /// Returns the required Project scope.
    #[must_use]
    pub const fn project_id(&self) -> ProjectId {
        self.project_id
    }
    /// Returns the optional normalized status filter.
    #[must_use]
    pub const fn status(&self) -> Option<GenerationTaskStatus> {
        self.status
    }
    /// Returns the optional request-kind filter.
    #[must_use]
    pub const fn request_kind(&self) -> Option<GenerationTaskRequestKind> {
        self.request_kind
    }
    /// Returns the optional stable cursor.
    #[must_use]
    pub const fn cursor(&self) -> Option<GenerationTaskListCursor> {
        self.cursor
    }
    /// Returns the bounded page size.
    #[must_use]
    pub const fn limit(&self) -> u8 {
        self.limit
    }
}

/// One stable bounded task-list page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GenerationTaskCursorPage<T> {
    /// Current page items.
    pub items: Vec<T>,
    /// Cursor for the next page when more items exist.
    pub next_cursor: Option<GenerationTaskListCursor>,
}
