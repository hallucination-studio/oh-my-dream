use backends::{
    ImageToVideoRequest as BackendImageToVideoRequest, InferenceBackendInterface, MockBackendImpl,
    ReferenceImageGenerationRequest as BackendReferenceImageGenerationRequest,
    ReferenceVideoGenerationRequest as BackendReferenceVideoGenerationRequest, TaskHandle,
    TaskStatus, TextToAudioRequest as BackendTextToAudioRequest,
    TextToImageRequest as BackendTextToImageRequest,
};
use nodes::{
    GeneratedArtifact, GeneratedOutput, GenerationContextInterface, GenerationError,
    ImageToVideoGeneratorInterface, ImageToVideoRequest, InlineMedia,
    ReferenceImageGenerationRequest, ReferenceImageGeneratorInterface,
    ReferenceVideoGenerationRequest, ReferenceVideoGeneratorInterface,
    TextToAudioGeneratorInterface, TextToAudioRequest, TextToImageGeneratorInterface,
    TextToImageRequest,
};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info, warn};

mod video;

use video::MOCK_VIDEO_WEBM;

const MAX_POLLS: usize = 60;
const POLL_INTERVAL: Duration = Duration::from_millis(10);
const MOCK_IMAGE_PNG: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0,
    0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120, 156, 99, 248, 207, 192, 240, 31, 0,
    5, 0, 1, 255, 137, 153, 61, 29, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];

pub(crate) struct MockGenerationAdapterImpl {
    backend: Arc<MockBackendImpl>,
}

impl MockGenerationAdapterImpl {
    pub(crate) fn new(backend: Arc<MockBackendImpl>) -> Self {
        Self { backend }
    }

    fn wait_for_success(
        &self,
        handle: &TaskHandle,
        media: MockMedia,
        context: &mut dyn GenerationContextInterface,
    ) -> Result<GeneratedOutput, GenerationError> {
        for poll_index in 0..MAX_POLLS {
            self.cancel_if_requested(handle, context)?;
            let status = pollster::block_on(self.backend.poll(handle))
                .map_err(|source| operation_error("poll generation task", source))?;
            match status {
                TaskStatus::Queued => log_pending(handle, poll_index, "queued"),
                TaskStatus::Running { progress } => {
                    context.progress(progress.0);
                    log_pending(handle, poll_index, "running");
                }
                TaskStatus::Succeeded { output, cost } => {
                    context.ensure_active()?;
                    info!(task_id = %handle.task_id, "mock generation task succeeded");
                    return translate_success(media, handle, &output, cost);
                }
                TaskStatus::Failed { reason } => {
                    warn!(task_id = %handle.task_id, reason = %reason, "mock generation task failed");
                    return Err(GenerationError::TaskFailed { reason });
                }
                TaskStatus::Cancelled => {
                    warn!(task_id = %handle.task_id, "mock generation task was cancelled");
                    return Err(GenerationError::TaskCancelled);
                }
            }
            std::thread::sleep(POLL_INTERVAL);
        }
        Err(GenerationError::PollLimit { max_polls: MAX_POLLS })
    }

    fn cancel_if_requested(
        &self,
        handle: &TaskHandle,
        context: &dyn GenerationContextInterface,
    ) -> Result<(), GenerationError> {
        if !context.is_cancelled() {
            return Ok(());
        }
        pollster::block_on(self.backend.cancel(handle))
            .map_err(|source| operation_error("cancel generation task", source))?;
        Err(GenerationError::TaskCancelled)
    }
}

impl TextToImageGeneratorInterface for MockGenerationAdapterImpl {
    fn generate(
        &self,
        request: TextToImageRequest,
        context: &mut dyn GenerationContextInterface,
    ) -> Result<GeneratedOutput, GenerationError> {
        context.ensure_active()?;
        let request = to_backend_text_to_image(request);
        let handle = pollster::block_on(self.backend.text_to_image(request))
            .map_err(|source| operation_error("submit text-to-image task", source))?;
        self.wait_for_success(&handle, MockMedia::Image, context)
    }
}

impl ReferenceImageGeneratorInterface for MockGenerationAdapterImpl {
    fn generate(
        &self,
        request: ReferenceImageGenerationRequest,
        context: &mut dyn GenerationContextInterface,
    ) -> Result<GeneratedOutput, GenerationError> {
        context.ensure_active()?;
        let request = to_backend_reference_image_generation(request);
        let handle = pollster::block_on(self.backend.reference_image_generation(request))
            .map_err(|source| operation_error("submit reference-image task", source))?;
        self.wait_for_success(&handle, MockMedia::ReferenceImage, context)
    }
}

impl ImageToVideoGeneratorInterface for MockGenerationAdapterImpl {
    fn generate(
        &self,
        request: ImageToVideoRequest,
        context: &mut dyn GenerationContextInterface,
    ) -> Result<GeneratedOutput, GenerationError> {
        context.ensure_active()?;
        let request = to_backend_image_to_video(request);
        let handle = pollster::block_on(self.backend.image_to_video(request))
            .map_err(|source| operation_error("submit image-to-video task", source))?;
        self.wait_for_success(&handle, MockMedia::Video, context)
    }
}

impl ReferenceVideoGeneratorInterface for MockGenerationAdapterImpl {
    fn generate(
        &self,
        request: ReferenceVideoGenerationRequest,
        context: &mut dyn GenerationContextInterface,
    ) -> Result<GeneratedOutput, GenerationError> {
        context.ensure_active()?;
        let request = to_backend_reference_video_generation(request);
        let handle = pollster::block_on(self.backend.reference_video_generation(request))
            .map_err(|source| operation_error("submit reference-video task", source))?;
        self.wait_for_success(&handle, MockMedia::ReferenceVideo, context)
    }
}

impl TextToAudioGeneratorInterface for MockGenerationAdapterImpl {
    fn generate(
        &self,
        request: TextToAudioRequest,
        context: &mut dyn GenerationContextInterface,
    ) -> Result<GeneratedOutput, GenerationError> {
        context.ensure_active()?;
        let request = to_backend_text_to_audio(request);
        let handle = pollster::block_on(self.backend.text_to_audio(request))
            .map_err(|source| operation_error("submit text-to-audio task", source))?;
        self.wait_for_success(&handle, MockMedia::Audio, context)
    }
}

fn to_backend_text_to_image(request: TextToImageRequest) -> BackendTextToImageRequest {
    BackendTextToImageRequest {
        model: request.model,
        prompt: request.prompt,
        negative_prompt: request.negative_prompt,
        steps: request.steps,
        seed: request.seed,
    }
}

fn to_backend_image_to_video(request: ImageToVideoRequest) -> BackendImageToVideoRequest {
    BackendImageToVideoRequest {
        model: request.model,
        image: request.image,
        duration_seconds: request.duration_seconds,
        fps: request.fps,
    }
}

fn to_backend_reference_image_generation(
    request: ReferenceImageGenerationRequest,
) -> BackendReferenceImageGenerationRequest {
    BackendReferenceImageGenerationRequest {
        model: request.model,
        images: request.images,
        prompt: request.prompt,
        negative_prompt: request.negative_prompt,
        steps: request.steps,
        seed: request.seed,
    }
}

fn to_backend_reference_video_generation(
    request: ReferenceVideoGenerationRequest,
) -> BackendReferenceVideoGenerationRequest {
    BackendReferenceVideoGenerationRequest {
        model: request.model,
        images: request.images,
        prompt: request.prompt,
        duration_seconds: request.duration_seconds,
        aspect_ratio: request.aspect_ratio,
        resolution: request.resolution,
        fps: request.fps,
    }
}

fn to_backend_text_to_audio(request: TextToAudioRequest) -> BackendTextToAudioRequest {
    BackendTextToAudioRequest { model: request.model, prompt: request.prompt, seed: request.seed }
}

#[derive(Debug, Clone, Copy)]
enum MockMedia {
    Image,
    ReferenceImage,
    Video,
    ReferenceVideo,
    Audio,
}

impl MockMedia {
    fn operation(self) -> &'static str {
        match self {
            Self::Image => "text-to-image",
            Self::ReferenceImage => "reference-image-generation",
            Self::Video => "image-to-video",
            Self::ReferenceVideo => "reference-video-generation",
            Self::Audio => "text-to-audio",
        }
    }

    fn inline_media(self) -> InlineMedia {
        match self {
            Self::Image | Self::ReferenceImage => InlineMedia::png(MOCK_IMAGE_PNG.to_vec()),
            Self::Video | Self::ReferenceVideo => InlineMedia::webm(MOCK_VIDEO_WEBM.to_vec()),
            Self::Audio => InlineMedia::wav(silent_pcm_wave()),
        }
    }
}

fn translate_success(
    media: MockMedia,
    handle: &TaskHandle,
    output: &str,
    cost: Option<i64>,
) -> Result<GeneratedOutput, GenerationError> {
    let expected = format!("mock://mock/{}/{}", media.operation(), handle.task_id);
    if output != expected {
        warn!(task_id = %handle.task_id, "mock backend returned an invalid output reference");
        return Err(GenerationError::InvalidOutput);
    }
    Ok(GeneratedOutput { artifact: GeneratedArtifact::InlineMedia(media.inline_media()), cost })
}

fn operation_error(operation: &'static str, source: backends::BackendError) -> GenerationError {
    GenerationError::OperationFailed { operation, reason: source.to_string() }
}

fn log_pending(handle: &TaskHandle, poll_index: usize, state: &str) {
    debug!(task_id = %handle.task_id, poll_index, state, "mock generation task pending");
}

fn silent_pcm_wave() -> Vec<u8> {
    let mut bytes = Vec::with_capacity(46);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&38_u32.to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&8_000_u32.to_le_bytes());
    bytes.extend_from_slice(&16_000_u32.to_le_bytes());
    bytes.extend_from_slice(&2_u16.to_le_bytes());
    bytes.extend_from_slice(&16_u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&2_u32.to_le_bytes());
    bytes.extend_from_slice(&[0, 0]);
    bytes
}

#[cfg(test)]
mod tests;
