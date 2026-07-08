//! The [`InferenceBackend`] trait: the pluggable seam over generation providers.

use async_trait::async_trait;

use crate::error::Result;
use crate::request::{ImageToVideoRequest, TextToAudioRequest, TextToImageRequest};
use crate::task::{TaskHandle, TaskStatus};

/// A generation provider.
///
/// Submission returns a [`TaskHandle`] immediately; callers then [`poll`] until
/// the task reaches a terminal state, and may [`cancel`] in between. This async,
/// poll-based shape matches how cloud vendors behave and keeps the mock and any
/// future real backend interchangeable.
///
/// [`poll`]: InferenceBackend::poll
/// [`cancel`]: InferenceBackend::cancel
#[async_trait]
pub trait InferenceBackend: Send + Sync {
    /// Stable identifier of this backend (e.g. `"mock"`).
    fn name(&self) -> &str;

    /// Submits a text-to-image task.
    async fn text_to_image(&self, request: TextToImageRequest) -> Result<TaskHandle>;

    /// Submits an image-to-video task.
    async fn image_to_video(&self, request: ImageToVideoRequest) -> Result<TaskHandle>;

    /// Submits a text-to-audio task.
    async fn text_to_audio(&self, request: TextToAudioRequest) -> Result<TaskHandle>;

    /// Returns the current status of a previously submitted task.
    async fn poll(&self, handle: &TaskHandle) -> Result<TaskStatus>;

    /// Requests cancellation of a running task.
    async fn cancel(&self, handle: &TaskHandle) -> Result<()>;
}
