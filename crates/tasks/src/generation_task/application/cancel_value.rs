//! Trusted internal Generation Task cancellation values.

use crate::generation_task::domain::GenerationTaskId;

/// Internal Workflow-owned request to commit cancellation intent for one exact Task.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GenerationTaskCancelCommand {
    task_id: GenerationTaskId,
}

impl GenerationTaskCancelCommand {
    /// Identifies the exact correlated Task selected by Workflow application code.
    #[must_use]
    pub const fn new(task_id: GenerationTaskId) -> Self {
        Self { task_id }
    }

    pub(super) const fn task_id(self) -> GenerationTaskId {
        self.task_id
    }
}
