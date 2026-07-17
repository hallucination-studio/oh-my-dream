use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use assets::asset::domain::AssetMediaKind;
use async_trait::async_trait;
use tasks::generation_task::*;

use crate::support::{asset_result, new_task, time};

pub(crate) struct AmbiguousImageExecutorFakeImpl {
    pub(crate) calls: Arc<AtomicUsize>,
}

pub(crate) struct UnusedImageRemoteFakeImpl;
pub(crate) struct TransientImagePollerFakeImpl;
pub(crate) struct TransientImagePollerAtFakeImpl {
    pub(crate) observed_at: GenerationTaskTimestamp,
}
pub(crate) struct TransientOriginReaderFakeImpl;
pub(crate) struct AcceptingCancellerFakeImpl {
    pub(crate) calls: Arc<AtomicUsize>,
}
pub(crate) struct TransientWorkflowCompletionFakeImpl;
pub(crate) struct RecordingImagePollerFakeImpl {
    pub(crate) events: Arc<Mutex<Vec<&'static str>>>,
}
pub(crate) struct RecordingAssetSinkFakeImpl {
    pub(crate) events: Arc<Mutex<Vec<&'static str>>>,
    pub(crate) recovery: GenerationTaskAssetRecovery,
}
pub(crate) struct CancellingImageExecutorFakeImpl {
    pub(crate) repository: GenerationTaskRepositoryFakeImpl,
    pub(crate) task_id: GenerationTaskId,
}

#[async_trait]
impl ImageGenerationSubmitterInterface for UnusedImageRemoteFakeImpl {
    async fn submit_image_generation(
        &self,
        _context: &GenerationProviderCallContext,
        _spec: &ImageGenerationSpec,
    ) -> Result<ImageGenerationSubmitOutcome, GenerationProviderCallError> {
        unreachable!("cancelled origins must not submit")
    }
}

#[async_trait]
impl ImageGenerationPollerInterface for UnusedImageRemoteFakeImpl {
    async fn poll_image_generation(
        &self,
        _context: &GenerationProviderCallContext,
        _handle: &GenerationProviderTaskHandle,
    ) -> Result<ImageGenerationPollOutcome, GenerationProviderCallError> {
        unreachable!("cancelled origins must not poll")
    }
}

#[async_trait]
impl ImageGenerationPollerInterface for TransientImagePollerFakeImpl {
    async fn poll_image_generation(
        &self,
        _context: &GenerationProviderCallContext,
        _handle: &GenerationProviderTaskHandle,
    ) -> Result<ImageGenerationPollOutcome, GenerationProviderCallError> {
        transient_poll_error(time(110))
    }
}

#[async_trait]
impl ImageGenerationPollerInterface for TransientImagePollerAtFakeImpl {
    async fn poll_image_generation(
        &self,
        _context: &GenerationProviderCallContext,
        _handle: &GenerationProviderTaskHandle,
    ) -> Result<ImageGenerationPollOutcome, GenerationProviderCallError> {
        transient_poll_error(self.observed_at)
    }
}

#[async_trait]
impl ImageGenerationPollerInterface for RecordingImagePollerFakeImpl {
    async fn poll_image_generation(
        &self,
        _context: &GenerationProviderCallContext,
        _handle: &GenerationProviderTaskHandle,
    ) -> Result<ImageGenerationPollOutcome, GenerationProviderCallError> {
        self.events.lock().unwrap().push("poll");
        transient_poll_error(time(110))
    }
}

#[async_trait]
impl GenerationTaskAssetSinkInterface for RecordingAssetSinkFakeImpl {
    async fn recover_generation_task_asset(
        &self,
        _key: GenerationTaskAssetKey,
    ) -> Result<GenerationTaskAssetRecovery, GenerationTaskBoundaryError> {
        self.events.lock().map_err(|_| GenerationTaskBoundaryError::Permanent)?.push("recover");
        Ok(self.recovery.clone())
    }

    async fn store_generation_task_asset(
        &self,
        _command: GenerationTaskStoreAssetCommand,
    ) -> Result<GenerationTaskAvailableAsset, GenerationTaskBoundaryError> {
        Err(GenerationTaskBoundaryError::Permanent)
    }
}

#[async_trait]
impl GenerationTaskOriginStateReaderInterface for TransientOriginReaderFakeImpl {
    async fn read_generation_task_origin_state(
        &self,
        _task: &GenerationTaskAggregate,
    ) -> Result<GenerationTaskOriginState, GenerationTaskBoundaryError> {
        Err(GenerationTaskBoundaryError::Transient)
    }
}

#[async_trait]
impl GenerationCancellerInterface for AcceptingCancellerFakeImpl {
    async fn cancel_generation(
        &self,
        _context: &GenerationProviderCallContext,
        _handle: &GenerationProviderTaskHandle,
    ) -> Result<GenerationCancellationOutcome, GenerationProviderCallError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(GenerationCancellationOutcome::Accepted)
    }
}

#[async_trait]
impl GenerationTaskWorkflowCompletionInterface for TransientWorkflowCompletionFakeImpl {
    async fn complete_generation_task_workflow_origin(
        &self,
        _task: &GenerationTaskAggregate,
    ) -> Result<GenerationTaskWorkflowCompletionOutcome, GenerationTaskBoundaryError> {
        Err(GenerationTaskBoundaryError::Transient)
    }
}

#[async_trait]
impl ImageGenerationImmediateExecutorInterface for CancellingImageExecutorFakeImpl {
    async fn execute_image_generation(
        &self,
        _context: &GenerationProviderCallContext,
        _spec: &ImageGenerationSpec,
    ) -> Result<ImageGenerationImmediateOutcome, GenerationProviderCallError> {
        GenerationTaskCancelUseCase::new(
            self.repository.clone(),
            GenerationTaskClockFakeImpl::new(time(110)),
        )
        .cancel_generation_task(GenerationTaskCancelCommand::new(self.task_id))
        .await
        .unwrap();
        Ok(ImageGenerationImmediateOutcome::Completed(
            ImageGenerationProviderResult::try_new(vec![1, 2, 3]).unwrap(),
        ))
    }
}

#[async_trait]
impl ImageGenerationImmediateExecutorInterface for AmbiguousImageExecutorFakeImpl {
    async fn execute_image_generation(
        &self,
        _context: &GenerationProviderCallContext,
        _spec: &ImageGenerationSpec,
    ) -> Result<ImageGenerationImmediateOutcome, GenerationProviderCallError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Err(GenerationProviderCallError::try_new(
            GenerationProviderCallErrorKind::Permanent,
            "SUBMIT_UNCERTAIN",
            "Submission outcome is uncertain.",
            None,
            time(110),
        )
        .unwrap())
    }
}

pub(crate) async fn seeded_repository() -> GenerationTaskRepositoryFakeImpl {
    let repository = GenerationTaskRepositoryFakeImpl::default();
    let task = new_task();
    repository
        .create_generation_task(
            &task,
            GenerationTaskEffect::new(task.id(), GenerationTaskEffectKind::SubmitTask, time(100)),
        )
        .await
        .unwrap();
    repository
}

pub(crate) fn source_required_asset_sink() -> GenerationTaskAssetSinkFakeImpl {
    GenerationTaskAssetSinkFakeImpl::new(
        GenerationTaskAssetRecovery::SourceRequired,
        GenerationTaskAvailableAsset::try_new(asset_result(AssetMediaKind::Image)).unwrap(),
    )
}

pub(crate) fn policy() -> GenerationProviderRoutePolicy {
    GenerationProviderRoutePolicy::try_new(30_000, 500).unwrap()
}

fn transient_poll_error(
    observed_at: GenerationTaskTimestamp,
) -> Result<ImageGenerationPollOutcome, GenerationProviderCallError> {
    Err(GenerationProviderCallError::try_new(
        GenerationProviderCallErrorKind::Transient,
        "POLL_UNAVAILABLE",
        "Polling is temporarily unavailable.",
        None,
        observed_at,
    )
    .unwrap())
}
