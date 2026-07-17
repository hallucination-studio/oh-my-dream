//! Internal Workflow-driven Generation Task cancellation.

use super::{
    GenerationTaskApplicationError, GenerationTaskCancelCommand, GenerationTaskOutboxChanges,
};
use crate::generation_task::interfaces::{
    GenerationTaskClockInterface, GenerationTaskRepositoryInterface,
};

/// Commits Workflow-owned cancellation intent without exposing a public Task command.
pub struct GenerationTaskCancelUseCase<R, C> {
    repository: R,
    clock: C,
}

impl<R, C> GenerationTaskCancelUseCase<R, C>
where
    R: GenerationTaskRepositoryInterface,
    C: GenerationTaskClockInterface,
{
    /// Wires trusted task persistence and time observation.
    #[must_use]
    pub const fn new(repository: R, clock: C) -> Self {
        Self { repository, clock }
    }

    /// Commits cancellation; the existing Submit/Poll effect performs external convergence.
    pub async fn cancel_generation_task(
        &self,
        command: GenerationTaskCancelCommand,
    ) -> Result<(), GenerationTaskApplicationError> {
        let mut task = self
            .repository
            .load_generation_task(command.task_id())
            .await?
            .ok_or(GenerationTaskApplicationError::TaskNotFound)?;
        let expected_revision = task.revision().get();
        let now = self.clock.observe_generation_task_time()?;
        task.request_cancellation(now)?;
        self.repository
            .save_generation_task(&task, expected_revision, GenerationTaskOutboxChanges::default())
            .await?;
        Ok(())
    }
}
