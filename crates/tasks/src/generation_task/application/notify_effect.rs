//! Closed NotifyWorkflow effect execution.

use super::effect_support::{future_time, reschedule_effect};
use super::{
    GenerationTaskApplicationError, GenerationTaskClaimedEffect, GenerationTaskEffectKind,
    GenerationTaskOutboxChanges,
};
use crate::generation_task::interfaces::{
    GenerationTaskClockInterface, GenerationTaskRepositoryInterface,
    GenerationTaskWorkflowCompletionInterface,
};

/// Delivers one terminal task outcome to its exact Workflow origin.
pub struct GenerationTaskNotifyWorkflowEffectUseCase<R, W, C> {
    repository: R,
    workflow_completion: W,
    clock: C,
}

impl<R, W, C> GenerationTaskNotifyWorkflowEffectUseCase<R, W, C>
where
    R: GenerationTaskRepositoryInterface,
    W: GenerationTaskWorkflowCompletionInterface,
    C: GenerationTaskClockInterface,
{
    /// Wires durable notification dependencies.
    #[must_use]
    pub const fn new(repository: R, workflow_completion: W, clock: C) -> Self {
        Self { repository, workflow_completion, clock }
    }

    /// Applies or idempotently observes one terminal completion.
    pub async fn execute_generation_task_notify_workflow_effect(
        &self,
        claimed: GenerationTaskClaimedEffect,
    ) -> Result<(), GenerationTaskApplicationError> {
        if claimed.effect().kind() != GenerationTaskEffectKind::NotifyWorkflow {
            return Err(GenerationTaskApplicationError::InvalidEffect);
        }
        let task = self
            .repository
            .load_generation_task(claimed.effect().task_id())
            .await?
            .ok_or(GenerationTaskApplicationError::TaskNotFound)?;
        if !task.state().is_terminal() {
            return Err(GenerationTaskApplicationError::InvalidEffect);
        }
        match self.workflow_completion.complete_generation_task_workflow_origin(&task).await {
            Ok(_) => self
                .repository
                .save_generation_task(
                    &task,
                    task.revision().get(),
                    GenerationTaskOutboxChanges {
                        consume: Some(claimed.claim()),
                        enqueue: Vec::new(),
                    },
                )
                .await
                .map_err(Into::into),
            Err(super::GenerationTaskBoundaryError::Transient) => {
                let now = self.clock.observe_generation_task_time()?;
                reschedule_effect(&self.repository, &task, claimed, future_time(now, 500)?).await
            }
            Err(error) => Err(error.into()),
        }
    }
}
