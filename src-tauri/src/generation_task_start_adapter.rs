//! Desktop translation from Node Capability intent to durable Generation Task admission.

use std::sync::{
    Arc,
    atomic::{AtomicI64, Ordering},
};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use assets::asset::domain::{AssetContentDigest, AssetId, AssetMediaKind};
use async_trait::async_trait;
use backends::mock_generation_provider::MockGenerationProviderRegistryImpl;
use engine::node_capability::{
    NodeCapabilityGenerationTaskStartFailure, WorkflowTextPart, WorkflowTextValue,
};
use nodes::{
    ImageAspectRatio as NodeImageAspectRatio, ImageToVideoDurationSeconds,
    NodeCapabilityGenerationTaskId, NodeCapabilityGenerationTaskRequest,
    NodeCapabilityGenerationTaskStartRequest, NodeCapabilityGenerationTaskStartResult,
    NodeCapabilityGenerationTaskStarterInterface,
};
use tasks::generation_task::{
    AssetSnapshotRef, GenerationProviderRegistryError, GenerationTaskApplicationError,
    GenerationTaskBoundaryError, GenerationTaskClockInterface, GenerationTaskId,
    GenerationTaskIdempotencyKey, GenerationTaskOrigin, GenerationTaskRepositoryError,
    GenerationTaskRepositoryInterface, GenerationTaskRequest, GenerationTaskStartCommand,
    GenerationTaskStartUseCase, GenerationTaskText, GenerationTaskTimestamp, ImageAspectRatio,
    ImageGenerationSpec, VideoDurationSeconds, VideoGenerationSpec, VoiceGenerationSpec,
};
use uuid::Uuid;

/// System UTC clock used by Generation Task application use cases.
#[derive(Default)]
pub struct SystemGenerationTaskClockAdapterImpl {
    last_milliseconds: AtomicI64,
}

impl GenerationTaskClockInterface for SystemGenerationTaskClockAdapterImpl {
    fn observe_generation_task_time(
        &self,
    ) -> Result<GenerationTaskTimestamp, GenerationTaskBoundaryError> {
        let milliseconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| GenerationTaskBoundaryError::Transient)?
            .as_millis();
        let milliseconds =
            i64::try_from(milliseconds).map_err(|_| GenerationTaskBoundaryError::Permanent)?;
        let previous = self.last_milliseconds.fetch_max(milliseconds, Ordering::AcqRel);
        GenerationTaskTimestamp::from_utc_milliseconds(previous.max(milliseconds))
            .map_err(|_| GenerationTaskBoundaryError::Permanent)
    }
}

/// Production bridge that persists one exact Task before reporting durable waiting.
pub struct DesktopNodeCapabilityGenerationTaskStartAdapterImpl<R, C> {
    registry: Arc<MockGenerationProviderRegistryImpl>,
    start_use_case: GenerationTaskStartUseCase<R, Arc<MockGenerationProviderRegistryImpl>, C>,
}

impl<R, C> DesktopNodeCapabilityGenerationTaskStartAdapterImpl<R, C>
where
    R: GenerationTaskRepositoryInterface,
    C: GenerationTaskClockInterface,
{
    /// Wires the durable repository, frozen Mock registry, and Task clock.
    #[must_use]
    pub fn new(repository: R, registry: Arc<MockGenerationProviderRegistryImpl>, clock: C) -> Self {
        Self {
            registry: registry.clone(),
            start_use_case: GenerationTaskStartUseCase::new(repository, registry, clock),
        }
    }

    fn command(
        &self,
        request: &NodeCapabilityGenerationTaskStartRequest,
    ) -> Result<GenerationTaskStartCommand, NodeCapabilityGenerationTaskStartFailure> {
        let task_request = translate_request(request.request())?;
        let target = self
            .registry
            .target_for_profile(request.profile_ref(), task_request.kind())
            .map_err(map_registry_error)?;
        let context = request.context();
        let origin = request.origin();
        let task_origin = GenerationTaskOrigin::new(
            context.project_id,
            origin.workflow_id(),
            origin.workflow_revision(),
            context.workflow_run_id,
            origin.workflow_node_id(),
            context.node_execution_id,
            origin.capability_contract_ref().clone(),
        );
        let task_id = GenerationTaskId::from_uuid(Uuid::new_v4())
            .map_err(|_| NodeCapabilityGenerationTaskStartFailure::InvalidRequest)?;
        let idempotency_key =
            GenerationTaskIdempotencyKey::try_new(context.node_execution_id.as_uuid().to_string())
                .map_err(|_| NodeCapabilityGenerationTaskStartFailure::InvalidRequest)?;
        Ok(GenerationTaskStartCommand::new(
            task_id,
            task_origin,
            idempotency_key,
            target,
            task_request,
        ))
    }
}

#[async_trait]
impl<R, C> NodeCapabilityGenerationTaskStarterInterface
    for DesktopNodeCapabilityGenerationTaskStartAdapterImpl<R, C>
where
    R: GenerationTaskRepositoryInterface,
    C: GenerationTaskClockInterface,
{
    async fn start_generation_task(
        &self,
        request: NodeCapabilityGenerationTaskStartRequest,
    ) -> Result<NodeCapabilityGenerationTaskStartResult, NodeCapabilityGenerationTaskStartFailure>
    {
        check_interruption(&request)?;
        let command = self.command(&request)?;
        check_interruption(&request)?;
        let result =
            self.start_use_case.start_generation_task(command).await.map_err(map_start_error)?;
        let task_id = NodeCapabilityGenerationTaskId::from_uuid(result.task_id().as_uuid())?;
        Ok(NodeCapabilityGenerationTaskStartResult::new(task_id))
    }
}

fn check_interruption(
    request: &NodeCapabilityGenerationTaskStartRequest,
) -> Result<(), NodeCapabilityGenerationTaskStartFailure> {
    if request.context().cancellation.is_cancelled() {
        return Err(NodeCapabilityGenerationTaskStartFailure::Cancelled);
    }
    if request.context().deadline.is_reached_at(Instant::now()) {
        return Err(NodeCapabilityGenerationTaskStartFailure::DeadlineExceeded);
    }
    Ok(())
}

fn translate_request(
    request: &NodeCapabilityGenerationTaskRequest,
) -> Result<GenerationTaskRequest, NodeCapabilityGenerationTaskStartFailure> {
    match request {
        NodeCapabilityGenerationTaskRequest::Image { prompt, aspect_ratio } => {
            Ok(GenerationTaskRequest::Image(ImageGenerationSpec::new(
                task_text(prompt)?,
                match aspect_ratio {
                    NodeImageAspectRatio::Square => ImageAspectRatio::Square,
                    NodeImageAspectRatio::LandscapeFourByThree => ImageAspectRatio::Landscape4To3,
                    NodeImageAspectRatio::PortraitThreeByFour => ImageAspectRatio::Portrait3To4,
                    NodeImageAspectRatio::LandscapeSixteenByNine => {
                        ImageAspectRatio::Landscape16To9
                    }
                    NodeImageAspectRatio::PortraitNineBySixteen => ImageAspectRatio::Portrait9To16,
                },
            )))
        }
        NodeCapabilityGenerationTaskRequest::Video { input_image, prompt, duration_seconds } => {
            let image = input_image.image_ref();
            let asset_id = AssetId::from_uuid(Uuid::from_bytes(image.asset_id().as_bytes()))
                .map_err(|_| NodeCapabilityGenerationTaskStartFailure::InvalidRequest)?;
            let snapshot = AssetSnapshotRef::new(
                asset_id,
                AssetMediaKind::Image,
                AssetContentDigest::from_bytes(image.content_fingerprint().as_bytes()),
            );
            let duration = match duration_seconds {
                ImageToVideoDurationSeconds::Five => VideoDurationSeconds::Five,
                ImageToVideoDurationSeconds::Ten => VideoDurationSeconds::Ten,
            };
            Ok(GenerationTaskRequest::Video(
                VideoGenerationSpec::try_new(
                    snapshot,
                    duration,
                    prompt.as_ref().map(task_text).transpose()?,
                )
                .map_err(|_| NodeCapabilityGenerationTaskStartFailure::InvalidRequest)?,
            ))
        }
        NodeCapabilityGenerationTaskRequest::Voice { text } => {
            Ok(GenerationTaskRequest::Voice(VoiceGenerationSpec::new(task_text(text)?)))
        }
    }
}

fn task_text(
    value: &WorkflowTextValue,
) -> Result<GenerationTaskText, NodeCapabilityGenerationTaskStartFailure> {
    let [WorkflowTextPart::Literal(value)] = value.parts() else {
        return Err(NodeCapabilityGenerationTaskStartFailure::InvalidRequest);
    };
    GenerationTaskText::try_new(value.clone())
        .map_err(|_| NodeCapabilityGenerationTaskStartFailure::InvalidRequest)
}

fn map_registry_error(
    _: GenerationProviderRegistryError,
) -> NodeCapabilityGenerationTaskStartFailure {
    NodeCapabilityGenerationTaskStartFailure::Unavailable
}

fn map_start_error(
    error: GenerationTaskApplicationError,
) -> NodeCapabilityGenerationTaskStartFailure {
    match error {
        GenerationTaskApplicationError::InvalidArgument
        | GenerationTaskApplicationError::TaskNotFound
        | GenerationTaskApplicationError::Domain(_) => {
            NodeCapabilityGenerationTaskStartFailure::InvalidRequest
        }
        GenerationTaskApplicationError::Repository(
            GenerationTaskRepositoryError::IdempotencyConflict
            | GenerationTaskRepositoryError::OriginConflict,
        ) => NodeCapabilityGenerationTaskStartFailure::Conflict,
        GenerationTaskApplicationError::Repository(_) => {
            NodeCapabilityGenerationTaskStartFailure::Persistence
        }
        GenerationTaskApplicationError::ProviderRegistry(_) => {
            NodeCapabilityGenerationTaskStartFailure::Unavailable
        }
        GenerationTaskApplicationError::Boundary(_) => {
            NodeCapabilityGenerationTaskStartFailure::Unavailable
        }
        GenerationTaskApplicationError::InvalidEffect => {
            NodeCapabilityGenerationTaskStartFailure::InvalidRequest
        }
    }
}

#[cfg(test)]
mod tests;
