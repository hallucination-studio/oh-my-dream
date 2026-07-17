use std::io::Cursor;
use std::sync::Arc;
use std::time::{Duration, Instant};

use backends::deterministic_provider::{
    DeterministicImageToVideoProviderRouteImpl, DeterministicTextToImageProviderRouteImpl,
    DeterministicTextToSpeechProviderRouteImpl,
};
use backends::provider_routing::{
    ImageToVideoProviderRouteInterface, ImageToVideoProviderRouterImpl,
    TextToImageProviderRouteInterface, TextToImageProviderRouterImpl,
    TextToSpeechProviderRouteInterface, TextToSpeechProviderRouterImpl,
};
use engine::node_capability::{
    NodeCapabilityExecutionCancellation, NodeCapabilityExecutionDeadline,
    NodeCapabilityProviderFailureCategory, WorkflowManagedAssetIdBoundaryValue,
    WorkflowManagedContentFingerprint, WorkflowManagedImageRef, WorkflowNodeExecutionContext,
    WorkflowNodeExecutionId, WorkflowRunId, WorkflowTextPart, WorkflowTextValue,
};
use nodes::{
    GenerationProfileId, GenerationProfileRef, GenerationProfileVersion, ImageAspectRatio,
    ImageToVideoDurationSeconds, ImageToVideoProviderInterface, ImageToVideoProviderRequest,
    NodeCapabilityDeclaredMediaFacts, NodeCapabilityMediaContentDigest,
    NodeCapabilityMediaMimeType, NodeCapabilityMediaSourceLease, NodeCapabilityReadableImageInput,
    TextToImageProviderInterface, TextToImageProviderRequest, TextToSpeechProviderInterface,
    TextToSpeechProviderRequest,
};
use projects::project::domain::ProjectId;
use sha2::{Digest, Sha256};
use tokio::io::AsyncReadExt;
use uuid::Uuid;

#[tokio::test]
async fn deterministic_routes_satisfy_the_three_exact_provider_contracts_through_routers() {
    let image_router = TextToImageProviderRouterImpl::try_new([(
        profile("image.high_quality_general"),
        Arc::new(DeterministicTextToImageProviderRouteImpl::try_new().unwrap())
            as Arc<dyn TextToImageProviderRouteInterface>,
    )])
    .unwrap();
    let video_router = ImageToVideoProviderRouterImpl::try_new([(
        profile("video.cinematic_image_animation"),
        Arc::new(DeterministicImageToVideoProviderRouteImpl::try_new().unwrap())
            as Arc<dyn ImageToVideoProviderRouteInterface>,
    )])
    .unwrap();
    let speech_router = TextToSpeechProviderRouterImpl::try_new([(
        profile("speech.multilingual_narration"),
        Arc::new(DeterministicTextToSpeechProviderRouteImpl::try_new().unwrap())
            as Arc<dyn TextToSpeechProviderRouteInterface>,
    )])
    .unwrap();

    let image = image_router
        .generate_image_from_text(TextToImageProviderRequest::new(
            profile("image.high_quality_general"),
            context(1, future()),
            text("draw a moon"),
            ImageAspectRatio::LandscapeFourByThree,
        ))
        .await
        .unwrap();
    assert_payload(
        image.mime_type(),
        image.facts(),
        image.into_source(),
        NodeCapabilityMediaMimeType::ImagePng,
        NodeCapabilityDeclaredMediaFacts::try_image(32, 32).unwrap(),
        vec![1; 16],
    )
    .await;

    let video = video_router
        .generate_video_from_image(ImageToVideoProviderRequest::new(
            profile("video.cinematic_image_animation"),
            context(2, future()),
            readable_image(),
            Some(text("slow camera")),
            ImageToVideoDurationSeconds::Ten,
        ))
        .await
        .unwrap();
    assert_payload(
        video.mime_type(),
        video.facts(),
        video.into_source(),
        NodeCapabilityMediaMimeType::VideoMp4,
        NodeCapabilityDeclaredMediaFacts::try_video(32, 32, 5_000, false).unwrap(),
        vec![2; 24],
    )
    .await;

    let speech = speech_router
        .synthesize_speech_from_text(TextToSpeechProviderRequest::new(
            profile("speech.multilingual_narration"),
            context(3, future()),
            text("hello"),
        ))
        .await
        .unwrap();
    assert_payload(
        speech.mime_type(),
        speech.facts(),
        speech.into_source(),
        NodeCapabilityMediaMimeType::AudioMpeg,
        NodeCapabilityDeclaredMediaFacts::try_audio(1_000, 44_100, 1).unwrap(),
        vec![3; 12],
    )
    .await;
}

#[tokio::test]
async fn deterministic_route_rejects_an_expired_execution_deadline() {
    let router = TextToImageProviderRouterImpl::try_new([(
        profile("image.high_quality_general"),
        Arc::new(DeterministicTextToImageProviderRouteImpl::try_new().unwrap())
            as Arc<dyn TextToImageProviderRouteInterface>,
    )])
    .unwrap();
    let result = router
        .generate_image_from_text(TextToImageProviderRequest::new(
            profile("image.high_quality_general"),
            context(4, Instant::now()),
            text("late"),
            ImageAspectRatio::Square,
        ))
        .await;
    let Err(failure) = result else { panic!("expired deterministic request succeeded") };
    assert_eq!(failure.category(), NodeCapabilityProviderFailureCategory::DeadlineExceeded);
}

async fn assert_payload(
    mime: NodeCapabilityMediaMimeType,
    facts: NodeCapabilityDeclaredMediaFacts,
    source: NodeCapabilityMediaSourceLease,
    expected_mime: NodeCapabilityMediaMimeType,
    expected_facts: NodeCapabilityDeclaredMediaFacts,
    expected_bytes: Vec<u8>,
) {
    assert_eq!(mime, expected_mime);
    assert_eq!(facts, expected_facts);
    assert_eq!(source.digest(), digest(&expected_bytes));
    let mut bytes = Vec::new();
    source.try_take_stream().unwrap().read_to_end(&mut bytes).await.unwrap();
    assert_eq!(bytes, expected_bytes);
}

fn readable_image() -> NodeCapabilityReadableImageInput {
    let bytes = vec![4; 12];
    NodeCapabilityReadableImageInput::try_new(
        WorkflowManagedImageRef::new(
            WorkflowManagedAssetIdBoundaryValue::from_bytes(uuid(80).into_bytes()).unwrap(),
            WorkflowManagedContentFingerprint::from_bytes(digest(&bytes).as_bytes()),
        ),
        NodeCapabilityMediaMimeType::ImagePng,
        NodeCapabilityDeclaredMediaFacts::try_image(32, 32).unwrap(),
        NodeCapabilityMediaSourceLease::try_new(
            bytes.len() as u64,
            digest(&bytes),
            future(),
            Box::pin(Cursor::new(bytes)),
        )
        .unwrap(),
    )
    .unwrap()
}

fn profile(id: &str) -> GenerationProfileRef {
    GenerationProfileRef::new(
        GenerationProfileId::try_new(id).unwrap(),
        GenerationProfileVersion::try_new(1).unwrap(),
    )
}

fn context(seed: u8, deadline: Instant) -> WorkflowNodeExecutionContext {
    WorkflowNodeExecutionContext {
        project_id: ProjectId::from_uuid(uuid(seed)).unwrap(),
        workflow_run_id: WorkflowRunId::from_uuid(uuid(seed + 20)).unwrap(),
        node_execution_id: WorkflowNodeExecutionId::from_uuid(uuid(seed + 40)).unwrap(),
        deadline: NodeCapabilityExecutionDeadline::at(deadline),
        cancellation: NodeCapabilityExecutionCancellation::active(),
    }
}

fn text(value: &str) -> WorkflowTextValue {
    WorkflowTextValue::try_new([WorkflowTextPart::Literal(value.to_owned())]).unwrap()
}

fn digest(bytes: &[u8]) -> NodeCapabilityMediaContentDigest {
    NodeCapabilityMediaContentDigest::from_bytes(Sha256::digest(bytes).into())
}

fn future() -> Instant {
    Instant::now() + Duration::from_secs(5)
}

fn uuid(seed: u8) -> Uuid {
    let mut bytes = [seed; 16];
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}
