use super::{
    MockGenerationAdapter, MockMedia, to_backend_image_to_video,
    to_backend_reference_image_generation, to_backend_reference_video_generation,
    to_backend_text_to_audio, to_backend_text_to_image, translate_success,
};
use backends::{InferenceBackend, MockBackend, TaskHandle, TaskStatus};
use nodes::{
    GeneratedArtifact, GenerationContext, GenerationError, ImageToVideoGenerator,
    ImageToVideoRequest, MediaFormat, MediaKind, ReferenceImageGenerationRequest,
    ReferenceImageGenerator, ReferenceVideoGenerationRequest, ReferenceVideoGenerator,
    TextToAudioGenerator, TextToAudioRequest, TextToImageGenerator, TextToImageRequest,
};
use std::cell::Cell;
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
fn translates_ordered_reference_requests_field_for_field() {
    let images = vec!["/tmp/second.png".to_owned(), "/tmp/first.png".to_owned()];
    let image = to_backend_reference_image_generation(ReferenceImageGenerationRequest {
        model: "reference-image-model".to_owned(),
        images: images.clone(),
        prompt: "combine them".to_owned(),
        negative_prompt: Some("blur".to_owned()),
        steps: Some(17),
        seed: Some(23),
    });
    assert_eq!(image.images, images);
    assert_eq!(image.model, "reference-image-model");
    assert_eq!(image.prompt, "combine them");
    assert_eq!(image.negative_prompt.as_deref(), Some("blur"));
    assert_eq!(image.steps, Some(17));
    assert_eq!(image.seed, Some(23));

    let images = vec!["/tmp/third.png".to_owned(), "/tmp/first.png".to_owned()];
    let video = to_backend_reference_video_generation(ReferenceVideoGenerationRequest {
        model: "reference-video-model".to_owned(),
        images: images.clone(),
        prompt: "animate them".to_owned(),
        duration_seconds: Some(3.5),
        aspect_ratio: Some("16:9".to_owned()),
        resolution: Some("720p".to_owned()),
        fps: Some(24),
    });
    assert_eq!(video.images, images);
    assert_eq!(video.model, "reference-video-model");
    assert_eq!(video.prompt, "animate them");
    assert_eq!(video.duration_seconds, Some(3.5));
    assert_eq!(video.aspect_ratio.as_deref(), Some("16:9"));
    assert_eq!(video.resolution.as_deref(), Some("720p"));
    assert_eq!(video.fps, Some(24));
}

#[test]
fn text_to_image_translates_mock_success_and_progress() {
    let adapter = MockGenerationAdapter::new(Arc::new(MockBackend::new()));
    let mut context = TestGenerationContext::default();

    let output = TextToImageGenerator::generate(
        &adapter,
        TextToImageRequest {
            model: "mock-image".to_owned(),
            prompt: "a bright sky".to_owned(),
            negative_prompt: None,
            steps: Some(4),
            seed: Some(7),
        },
        &mut context,
    )
    .expect("mock image generation");

    let media = expect_inline(output.artifact, MediaKind::Image, MediaFormat::Png);
    assert_eq!(output.cost, Some(250));
    assert_eq!(context.progress, vec![0.25, 0.75]);
    assert!(media.bytes().starts_with(b"\x89PNG\r\n\x1a\n"));
}

#[test]
fn reference_image_generation_returns_inline_png() {
    let adapter = MockGenerationAdapter::new(Arc::new(MockBackend::new()));
    let mut context = TestGenerationContext::default();

    let output = ReferenceImageGenerator::generate(
        &adapter,
        ReferenceImageGenerationRequest {
            model: "mock-reference-image".to_owned(),
            images: vec!["/tmp/second.png".to_owned(), "/tmp/first.png".to_owned()],
            prompt: "combine them".to_owned(),
            negative_prompt: None,
            steps: Some(4),
            seed: Some(7),
        },
        &mut context,
    )
    .expect("mock reference image generation");

    let media = expect_inline(output.artifact, MediaKind::Image, MediaFormat::Png);
    assert_eq!(output.cost, Some(400));
    assert!(media.bytes().starts_with(b"\x89PNG\r\n\x1a\n"));
}

#[test]
fn image_to_video_translates_mock_success_to_inline_video() {
    let adapter = MockGenerationAdapter::new(Arc::new(MockBackend::new()));
    let mut context = TestGenerationContext::default();

    let output = ImageToVideoGenerator::generate(
        &adapter,
        ImageToVideoRequest {
            model: "mock-video".to_owned(),
            image: "/tmp/source.png".to_owned(),
            duration_seconds: Some(2.0),
            fps: Some(12),
        },
        &mut context,
    )
    .expect("mock video generation");

    let media = expect_inline(output.artifact, MediaKind::Video, MediaFormat::WebM);
    assert_eq!(output.cost, Some(900));
    assert_playable_webm(media.bytes());
}

#[test]
fn reference_video_generation_returns_inline_video() {
    let adapter = MockGenerationAdapter::new(Arc::new(MockBackend::new()));
    let mut context = TestGenerationContext::default();

    let output = ReferenceVideoGenerator::generate(
        &adapter,
        ReferenceVideoGenerationRequest {
            model: "mock-reference-video".to_owned(),
            images: vec!["/tmp/second.png".to_owned(), "/tmp/first.png".to_owned()],
            prompt: "animate them".to_owned(),
            duration_seconds: Some(2.0),
            aspect_ratio: Some("16:9".to_owned()),
            resolution: Some("720p".to_owned()),
            fps: Some(12),
        },
        &mut context,
    )
    .expect("mock reference video generation");

    let media = expect_inline(output.artifact, MediaKind::Video, MediaFormat::WebM);
    assert_eq!(output.cost, Some(1_200));
    assert_playable_webm(media.bytes());
}

#[test]
fn text_to_audio_translates_mock_success_to_inline_wave() {
    let adapter = MockGenerationAdapter::new(Arc::new(MockBackend::new()));
    let mut context = TestGenerationContext::default();

    let output = TextToAudioGenerator::generate(
        &adapter,
        TextToAudioRequest {
            model: "mock-audio".to_owned(),
            prompt: "rain on glass".to_owned(),
            seed: Some(7),
        },
        &mut context,
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
    let mut context = TestGenerationContext::default();

    let error = TextToAudioGenerator::generate(
        &adapter,
        TextToAudioRequest {
            model: "mock-audio".to_owned(),
            prompt: "rain".to_owned(),
            seed: None,
        },
        &mut context,
    )
    .expect_err("failed mock task");

    assert_eq!(error, GenerationError::TaskFailed { reason: "provider outage".to_owned() });
}

#[test]
fn cancellation_reaches_the_submitted_backend_task() {
    let backend = Arc::new(MockBackend::new());
    let adapter = MockGenerationAdapter::new(Arc::clone(&backend));
    let mut context = TestGenerationContext { cancel_on_progress: true, ..Default::default() };

    let error = TextToImageGenerator::generate(
        &adapter,
        TextToImageRequest {
            model: "mock-image".to_owned(),
            prompt: "a bright sky".to_owned(),
            negative_prompt: None,
            steps: Some(4),
            seed: Some(7),
        },
        &mut context,
    )
    .expect_err("generation should be cancelled");

    let handle = TaskHandle { backend: "mock".to_owned(), task_id: "task-1".to_owned() };
    let status = pollster::block_on(backend.poll(&handle)).expect("poll cancelled task");
    assert_eq!(error, GenerationError::TaskCancelled);
    assert_eq!(status, TaskStatus::Cancelled);
}

#[test]
fn cancellation_before_submission_creates_no_backend_task() {
    let backend = Arc::new(MockBackend::new());
    let adapter = MockGenerationAdapter::new(Arc::clone(&backend));
    let mut context = TestGenerationContext { cancelled: true, ..Default::default() };

    let error = TextToImageGenerator::generate(
        &adapter,
        TextToImageRequest {
            model: "mock-image".to_owned(),
            prompt: "a bright sky".to_owned(),
            negative_prompt: None,
            steps: Some(4),
            seed: Some(7),
        },
        &mut context,
    )
    .expect_err("pre-cancelled generation should not be submitted");

    assert_eq!(error, GenerationError::TaskCancelled);
    assert_eq!(backend.submitted_task_count(), 0);
}

#[test]
fn cancellation_after_provider_success_does_not_cancel_terminal_task() {
    let backend = Arc::new(MockBackend::new());
    let adapter = MockGenerationAdapter::new(Arc::clone(&backend));
    let mut context = TestGenerationContext { cancel_on_check: Some(6), ..Default::default() };

    let error = TextToImageGenerator::generate(
        &adapter,
        TextToImageRequest {
            model: "mock-image".to_owned(),
            prompt: "a bright sky".to_owned(),
            negative_prompt: None,
            steps: Some(4),
            seed: Some(7),
        },
        &mut context,
    )
    .expect_err("locally cancelled output should be discarded");

    let handle = TaskHandle { backend: "mock".to_owned(), task_id: "task-1".to_owned() };
    let status = pollster::block_on(backend.poll(&handle)).expect("poll terminal task");
    assert_eq!(error, GenerationError::TaskCancelled);
    assert!(matches!(status, TaskStatus::Succeeded { .. }));
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

fn assert_playable_webm(bytes: &[u8]) {
    assert!(bytes.starts_with(&[0x1a, 0x45, 0xdf, 0xa3]));
    assert!(bytes.windows(4).any(|window| window == b"webm"));
    assert!(bytes.windows(5).any(|window| window == b"V_VP8"));
    assert!(bytes.windows(4).any(|window| window == b"\x1fC\xb6u"));
}

#[derive(Default)]
struct TestGenerationContext {
    progress: Vec<f32>,
    cancel_on_progress: bool,
    cancelled: bool,
    cancel_on_check: Option<usize>,
    cancellation_checks: Cell<usize>,
}

impl GenerationContext for TestGenerationContext {
    fn progress(&mut self, progress: f32) {
        self.progress.push(progress);
        self.cancelled = self.cancel_on_progress;
    }

    fn is_cancelled(&self) -> bool {
        let checks = self.cancellation_checks.get() + 1;
        self.cancellation_checks.set(checks);
        self.cancelled || self.cancel_on_check == Some(checks)
    }
}
