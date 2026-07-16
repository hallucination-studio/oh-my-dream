//! The [`InferenceBackendInterface`] trait: the pluggable seam over generation providers.

use async_trait::async_trait;

use crate::error::BackendResult;
use crate::request::{
    ImageToVideoRequest, ReferenceImageGenerationRequest, ReferenceVideoGenerationRequest,
    TextToAudioRequest, TextToImageRequest,
};
use crate::task::{TaskHandle, TaskStatus};

/// A generation provider.
///
/// Submission returns a [`TaskHandle`] immediately; callers then [`poll`] until
/// the task reaches a terminal state, and may [`cancel`] in between. This async,
/// poll-based shape matches how cloud vendors behave and keeps the mock and any
/// future real backend interchangeable.
///
/// [`poll`]: InferenceBackendInterface::poll
/// [`cancel`]: InferenceBackendInterface::cancel
#[async_trait]
pub trait InferenceBackendInterface: Send + Sync {
    /// Stable identifier of this backend (e.g. `"mock"`).
    fn name(&self) -> &str;

    /// Submits a text-to-image task.
    async fn text_to_image(&self, request: TextToImageRequest) -> BackendResult<TaskHandle>;

    /// Submits an ordered-reference image generation task.
    async fn reference_image_generation(
        &self,
        request: ReferenceImageGenerationRequest,
    ) -> BackendResult<TaskHandle>;

    /// Submits an image-to-video task.
    async fn image_to_video(&self, request: ImageToVideoRequest) -> BackendResult<TaskHandle>;

    /// Submits an ordered-reference video generation task.
    async fn reference_video_generation(
        &self,
        request: ReferenceVideoGenerationRequest,
    ) -> BackendResult<TaskHandle>;

    /// Submits a text-to-audio task.
    async fn text_to_audio(&self, request: TextToAudioRequest) -> BackendResult<TaskHandle>;

    /// Returns the current status of a previously submitted task.
    async fn poll(&self, handle: &TaskHandle) -> BackendResult<TaskStatus>;

    /// Requests cancellation of a running task.
    async fn cancel(&self, handle: &TaskHandle) -> BackendResult<()>;
}
