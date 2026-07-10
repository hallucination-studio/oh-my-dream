use backends::{
    ImageToVideoRequest as BackendImageToVideoRequest, InferenceBackend, MockBackend, TaskHandle,
    TaskStatus, TextToAudioRequest as BackendTextToAudioRequest,
    TextToImageRequest as BackendTextToImageRequest,
};
use nodes::{
    GeneratedArtifact, GeneratedOutput, GenerationError, ImageToVideoGenerator,
    ImageToVideoRequest, InlineMedia, TextToAudioGenerator, TextToAudioRequest,
    TextToImageGenerator, TextToImageRequest,
};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info, warn};

const MAX_POLLS: usize = 60;
const POLL_INTERVAL: Duration = Duration::from_millis(10);
const MOCK_IMAGE_PNG: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0,
    0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120, 156, 99, 248, 207, 192, 240, 31, 0,
    5, 0, 1, 255, 137, 153, 61, 29, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];
const MOCK_VIDEO: &[u8] = b"OH_MY_DREAM_MOCK_VIDEO_V1\n";

pub(crate) struct MockGenerationAdapter {
    backend: Arc<MockBackend>,
}

impl MockGenerationAdapter {
    pub(crate) fn new(backend: Arc<MockBackend>) -> Self {
        Self { backend }
    }

    fn wait_for_success(
        &self,
        handle: &TaskHandle,
        media: MockMedia,
        on_progress: &mut dyn FnMut(f32),
    ) -> Result<GeneratedOutput, GenerationError> {
        for poll_index in 0..MAX_POLLS {
            let status = pollster::block_on(self.backend.poll(handle))
                .map_err(|source| operation_error("poll generation task", source))?;
            match status {
                TaskStatus::Queued => log_pending(handle, poll_index, "queued"),
                TaskStatus::Running { progress } => {
                    on_progress(progress.0);
                    log_pending(handle, poll_index, "running");
                }
                TaskStatus::Succeeded { output, cost } => {
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
}

impl TextToImageGenerator for MockGenerationAdapter {
    fn generate(
        &self,
        request: TextToImageRequest,
        on_progress: &mut dyn FnMut(f32),
    ) -> Result<GeneratedOutput, GenerationError> {
        let request = to_backend_text_to_image(request);
        let handle = pollster::block_on(self.backend.text_to_image(request))
            .map_err(|source| operation_error("submit text-to-image task", source))?;
        self.wait_for_success(&handle, MockMedia::Image, on_progress)
    }
}

impl ImageToVideoGenerator for MockGenerationAdapter {
    fn generate(
        &self,
        request: ImageToVideoRequest,
        on_progress: &mut dyn FnMut(f32),
    ) -> Result<GeneratedOutput, GenerationError> {
        let request = to_backend_image_to_video(request);
        let handle = pollster::block_on(self.backend.image_to_video(request))
            .map_err(|source| operation_error("submit image-to-video task", source))?;
        self.wait_for_success(&handle, MockMedia::Video, on_progress)
    }
}

impl TextToAudioGenerator for MockGenerationAdapter {
    fn generate(
        &self,
        request: TextToAudioRequest,
        on_progress: &mut dyn FnMut(f32),
    ) -> Result<GeneratedOutput, GenerationError> {
        let request = to_backend_text_to_audio(request);
        let handle = pollster::block_on(self.backend.text_to_audio(request))
            .map_err(|source| operation_error("submit text-to-audio task", source))?;
        self.wait_for_success(&handle, MockMedia::Audio, on_progress)
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

fn to_backend_text_to_audio(request: TextToAudioRequest) -> BackendTextToAudioRequest {
    BackendTextToAudioRequest { model: request.model, prompt: request.prompt, seed: request.seed }
}

#[derive(Debug, Clone, Copy)]
enum MockMedia {
    Image,
    Video,
    Audio,
}

impl MockMedia {
    fn operation(self) -> &'static str {
        match self {
            Self::Image => "text-to-image",
            Self::Video => "image-to-video",
            Self::Audio => "text-to-audio",
        }
    }

    fn inline_media(self) -> InlineMedia {
        match self {
            Self::Image => InlineMedia::png(MOCK_IMAGE_PNG.to_vec()),
            Self::Video => InlineMedia::opaque_video(MOCK_VIDEO.to_vec()),
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
mod tests {
    use super::{
        MockGenerationAdapter, MockMedia, to_backend_image_to_video, to_backend_text_to_audio,
        to_backend_text_to_image, translate_success,
    };
    use backends::{MockBackend, TaskHandle};
    use nodes::{
        GeneratedArtifact, GenerationError, ImageToVideoGenerator, ImageToVideoRequest,
        MediaFormat, MediaKind, TextToAudioGenerator, TextToAudioRequest, TextToImageGenerator,
        TextToImageRequest,
    };
    use std::sync::Arc;

    #[test]
    fn translates_nodes_owned_requests_field_for_field() {
        let image = to_backend_text_to_image(TextToImageRequest {
            model: "image-model".to_owned(),
            prompt: "bright sky".to_owned(),
            negative_prompt: Some("clouds".to_owned()),
            steps: Some(17),
            seed: Some(23),
        });
        assert_eq!(image.model, "image-model");
        assert_eq!(image.prompt, "bright sky");
        assert_eq!(image.negative_prompt.as_deref(), Some("clouds"));
        assert_eq!(image.steps, Some(17));
        assert_eq!(image.seed, Some(23));

        let video = to_backend_image_to_video(ImageToVideoRequest {
            model: "video-model".to_owned(),
            image: "/tmp/source.png".to_owned(),
            duration_seconds: Some(3.5),
            fps: Some(24),
        });
        assert_eq!(video.model, "video-model");
        assert_eq!(video.image, "/tmp/source.png");
        assert_eq!(video.duration_seconds, Some(3.5));
        assert_eq!(video.fps, Some(24));

        let audio = to_backend_text_to_audio(TextToAudioRequest {
            model: "audio-model".to_owned(),
            prompt: "soft rain".to_owned(),
            seed: Some(29),
        });
        assert_eq!(audio.model, "audio-model");
        assert_eq!(audio.prompt, "soft rain");
        assert_eq!(audio.seed, Some(29));
    }

    #[test]
    fn text_to_image_translates_mock_success_and_progress() {
        let adapter = MockGenerationAdapter::new(Arc::new(MockBackend::new()));
        let mut progress = Vec::new();

        let output = TextToImageGenerator::generate(
            &adapter,
            TextToImageRequest {
                model: "mock-image".to_owned(),
                prompt: "a bright sky".to_owned(),
                negative_prompt: None,
                steps: Some(4),
                seed: Some(7),
            },
            &mut |value| progress.push(value),
        )
        .expect("mock image generation");

        let media = expect_inline(output.artifact, MediaKind::Image, MediaFormat::Png);
        assert_eq!(output.cost, Some(250));
        assert_eq!(progress, vec![0.25, 0.75]);
        assert!(media.bytes().starts_with(b"\x89PNG\r\n\x1a\n"));
    }

    #[test]
    fn image_to_video_translates_mock_success_to_inline_video() {
        let adapter = MockGenerationAdapter::new(Arc::new(MockBackend::new()));

        let output = ImageToVideoGenerator::generate(
            &adapter,
            ImageToVideoRequest {
                model: "mock-video".to_owned(),
                image: "/tmp/source.png".to_owned(),
                duration_seconds: Some(2.0),
                fps: Some(12),
            },
            &mut |_| {},
        )
        .expect("mock video generation");

        let media = expect_inline(output.artifact, MediaKind::Video, MediaFormat::OpaqueVideo);
        assert_eq!(output.cost, Some(900));
        assert!(media.bytes().starts_with(b"OH_MY_DREAM_MOCK_VIDEO_V1\n"));
    }

    #[test]
    fn text_to_audio_translates_mock_success_to_inline_wave() {
        let adapter = MockGenerationAdapter::new(Arc::new(MockBackend::new()));

        let output = TextToAudioGenerator::generate(
            &adapter,
            TextToAudioRequest {
                model: "mock-audio".to_owned(),
                prompt: "rain on glass".to_owned(),
                seed: Some(7),
            },
            &mut |_| {},
        )
        .expect("mock audio generation");

        let media = expect_inline(output.artifact, MediaKind::Audio, MediaFormat::Wav);
        assert_eq!(output.cost, Some(125));
        assert_eq!(&media.bytes()[0..4], b"RIFF");
        assert_eq!(&media.bytes()[8..12], b"WAVE");
    }

    #[test]
    fn failed_mock_task_maps_to_generation_failure() {
        let adapter =
            MockGenerationAdapter::new(Arc::new(MockBackend::always_fails("provider outage")));

        let error = TextToAudioGenerator::generate(
            &adapter,
            TextToAudioRequest {
                model: "mock-audio".to_owned(),
                prompt: "rain".to_owned(),
                seed: None,
            },
            &mut |_| {},
        )
        .expect_err("failed mock task");

        assert_eq!(error, GenerationError::TaskFailed { reason: "provider outage".to_owned() });
    }

    #[test]
    fn invalid_mock_output_is_rejected_without_echoing_credentials() {
        let output = "https://media.example/image.png?token=secret";
        let handle = TaskHandle { backend: "mock".to_owned(), task_id: "task-1".to_owned() };

        let error = translate_success(MockMedia::Image, &handle, output, None)
            .expect_err("unexpected mock output");

        assert_eq!(error, GenerationError::InvalidOutput);
        assert!(!error.to_string().contains(output));
        assert!(!error.to_string().contains("secret"));
    }

    fn expect_inline(
        artifact: GeneratedArtifact,
        kind: MediaKind,
        format: MediaFormat,
    ) -> nodes::InlineMedia {
        let GeneratedArtifact::InlineMedia(media) = artifact else {
            panic!("expected inline media");
        };
        assert_eq!(media.kind(), kind);
        assert_eq!(media.format(), format);
        media
    }
}
