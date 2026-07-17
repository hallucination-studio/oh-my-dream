//! Closed dispatcher from claimed Task effects to canonical application use cases.

use async_trait::async_trait;
use tasks::generation_task::{
    GenerationProviderRegistryInterface, GenerationTaskApplicationError,
    GenerationTaskAssetSinkInterface, GenerationTaskCancelRemoteEffectUseCase,
    GenerationTaskClaimedEffect, GenerationTaskClockInterface,
    GenerationTaskEffectExecutorInterface, GenerationTaskEffectKind,
    GenerationTaskNotifyWorkflowEffectUseCase, GenerationTaskOriginStateReaderInterface,
    GenerationTaskPollEffectUseCase, GenerationTaskRepositoryInterface,
    GenerationTaskSubmitEffectUseCase, GenerationTaskWorkflowCompletionInterface,
};

/// Executes only the four frozen Generation Task effect kinds.
pub struct DesktopGenerationTaskEffectExecutorImpl<R, P, O, A, W, C> {
    repository: R,
    provider_registry: P,
    origin_reader: O,
    asset_sink: A,
    workflow_completion: W,
    clock: C,
}

impl<R, P, O, A, W, C> DesktopGenerationTaskEffectExecutorImpl<R, P, O, A, W, C> {
    /// Wires the exact dependencies consumed by the four Task effect use cases.
    #[must_use]
    pub const fn new(
        repository: R,
        provider_registry: P,
        origin_reader: O,
        asset_sink: A,
        workflow_completion: W,
        clock: C,
    ) -> Self {
        Self {
            repository,
            provider_registry,
            origin_reader,
            asset_sink,
            workflow_completion,
            clock,
        }
    }
}

#[async_trait]
impl<R, P, O, A, W, C> GenerationTaskEffectExecutorInterface
    for DesktopGenerationTaskEffectExecutorImpl<R, P, O, A, W, C>
where
    R: GenerationTaskRepositoryInterface + Clone,
    P: GenerationProviderRegistryInterface + Clone,
    O: GenerationTaskOriginStateReaderInterface + Clone,
    A: GenerationTaskAssetSinkInterface + Clone,
    W: GenerationTaskWorkflowCompletionInterface + Clone,
    C: GenerationTaskClockInterface + Clone,
{
    async fn execute_generation_task_effect(
        &self,
        claimed: GenerationTaskClaimedEffect,
    ) -> Result<(), GenerationTaskApplicationError> {
        match claimed.effect().kind() {
            GenerationTaskEffectKind::SubmitTask => {
                GenerationTaskSubmitEffectUseCase::new(
                    self.repository.clone(),
                    self.provider_registry.clone(),
                    self.origin_reader.clone(),
                    self.asset_sink.clone(),
                    self.clock.clone(),
                )
                .execute_generation_task_submit_effect(claimed)
                .await
            }
            GenerationTaskEffectKind::PollTask => {
                GenerationTaskPollEffectUseCase::new(
                    self.repository.clone(),
                    self.provider_registry.clone(),
                    self.origin_reader.clone(),
                    self.asset_sink.clone(),
                    self.clock.clone(),
                )
                .execute_generation_task_poll_effect(claimed)
                .await
            }
            GenerationTaskEffectKind::CancelRemoteTask => {
                GenerationTaskCancelRemoteEffectUseCase::new(
                    self.repository.clone(),
                    self.provider_registry.clone(),
                    self.origin_reader.clone(),
                    self.clock.clone(),
                )
                .execute_generation_task_cancel_remote_effect(claimed)
                .await
            }
            GenerationTaskEffectKind::NotifyWorkflow => {
                GenerationTaskNotifyWorkflowEffectUseCase::new(
                    self.repository.clone(),
                    self.workflow_completion.clone(),
                    self.clock.clone(),
                )
                .execute_generation_task_notify_workflow_effect(claimed)
                .await
            }
        }
    }
}
