use std::sync::Arc;

use async_trait::async_trait;
use tasks::generation_task::*;

use crate::support::{image_request, origin, target, time, uuid};

#[tokio::test]
async fn start_is_idempotent_and_creates_one_submit_effect() {
    let repository = GenerationTaskRepositoryFakeImpl::default();
    let route = GenerationProviderResolvedRoute::Image {
        execution: ImageGenerationProviderExecution::Immediate(Arc::new(ImageExecutorFakeImpl)),
        policy: GenerationProviderRoutePolicy::try_new(30_000, 500).unwrap(),
    };
    let registry = GenerationProviderRegistryFakeImpl::new(route);
    let clock = GenerationTaskClockFakeImpl::new(time(100));
    let use_case = GenerationTaskStartUseCase::new(repository.clone(), registry, clock);
    let command = GenerationTaskStartCommand::new(
        GenerationTaskId::from_uuid(uuid(101)).unwrap(),
        origin(1),
        GenerationTaskIdempotencyKey::try_new("start-1").unwrap(),
        target("mock.image.high-quality-general.v1"),
        image_request("one image"),
    );

    let first = use_case.start_generation_task(command.clone()).await.unwrap();
    let replay = use_case.start_generation_task(command).await.unwrap();

    assert_eq!(first, replay);
    assert_eq!(repository.generation_task_count(), 1);
    assert_eq!(repository.ready_effect_kinds(), vec![GenerationTaskEffectKind::SubmitTask]);
}

#[tokio::test]
async fn start_rejects_reused_idempotency_key_with_different_request() {
    let repository = GenerationTaskRepositoryFakeImpl::default();
    let use_case = GenerationTaskStartUseCase::new(
        repository,
        image_registry(),
        GenerationTaskClockFakeImpl::new(time(100)),
    );
    let first = start_command(101, "start-1", "one image");
    let conflicting = start_command(102, "start-1", "different image");

    use_case.start_generation_task(first).await.unwrap();
    let error = use_case.start_generation_task(conflicting).await.unwrap_err();

    assert_eq!(
        error,
        GenerationTaskApplicationError::Repository(
            GenerationTaskRepositoryError::IdempotencyConflict
        )
    );
}

#[tokio::test]
async fn start_rejects_reused_origin_with_different_request() {
    let repository = GenerationTaskRepositoryFakeImpl::default();
    let use_case = GenerationTaskStartUseCase::new(
        repository,
        image_registry(),
        GenerationTaskClockFakeImpl::new(time(100)),
    );

    use_case.start_generation_task(start_command(101, "start-1", "one image")).await.unwrap();
    let error = use_case
        .start_generation_task(start_command(102, "start-2", "different image"))
        .await
        .unwrap_err();

    assert_eq!(
        error,
        GenerationTaskApplicationError::Repository(GenerationTaskRepositoryError::OriginConflict)
    );
}

fn image_registry() -> GenerationProviderRegistryFakeImpl {
    GenerationProviderRegistryFakeImpl::new(GenerationProviderResolvedRoute::Image {
        execution: ImageGenerationProviderExecution::Immediate(Arc::new(ImageExecutorFakeImpl)),
        policy: GenerationProviderRoutePolicy::try_new(30_000, 500).unwrap(),
    })
}

fn start_command(
    task_seed: u128,
    idempotency_key: &str,
    prompt: &str,
) -> GenerationTaskStartCommand {
    GenerationTaskStartCommand::new(
        GenerationTaskId::from_uuid(uuid(task_seed)).unwrap(),
        origin(1),
        GenerationTaskIdempotencyKey::try_new(idempotency_key).unwrap(),
        target("mock.image.high-quality-general.v1"),
        image_request(prompt),
    )
}

struct ImageExecutorFakeImpl;

#[async_trait]
impl ImageGenerationImmediateExecutorInterface for ImageExecutorFakeImpl {
    async fn execute_image_generation(
        &self,
        _context: &GenerationProviderCallContext,
        _spec: &ImageGenerationSpec,
    ) -> Result<ImageGenerationImmediateOutcome, GenerationProviderCallError> {
        Ok(ImageGenerationImmediateOutcome::Completed(
            ImageGenerationProviderResult::try_new(vec![1]).unwrap(),
        ))
    }
}
