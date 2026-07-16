use std::io::Cursor;
use std::time::{Duration, Instant};

use engine::node_capability::{
    NodeCapabilityExecutionCancellation, NodeCapabilityExecutionDeadline,
    NodeCapabilityProviderFailureCategory, WorkflowManagedAssetIdBoundaryValue,
    WorkflowManagedContentFingerprint, WorkflowManagedImageRef, WorkflowNodeExecutionContext,
    WorkflowNodeExecutionId, WorkflowRunId, WorkflowTextPart, WorkflowTextValue,
};
use nodes::*;
use projects::project::domain::ProjectId;
use sha2::{Digest, Sha256};
use tokio::io::AsyncReadExt;
use uuid::Uuid;

pub async fn assert_text_to_image_provider_contract(provider: &impl TextToImageProviderInterface) {
    let context = execution_context(5, future_instant());
    let request = TextToImageProviderRequest::new(
        profile_ref(),
        context.clone(),
        text("draw a moon"),
        ImageAspectRatio::LandscapeFourByThree,
    );
    assert_eq!(request.profile_ref(), &profile_ref());
    assert_context(request.context(), &context);
    assert_eq!(request.prompt().parts(), text("draw a moon").parts());
    assert_eq!(request.aspect_ratio(), ImageAspectRatio::LandscapeFourByThree);
    let payload = provider.generate_image_from_text(request).await.unwrap();
    assert_payload(
        payload.mime_type(),
        payload.facts(),
        payload.into_source(),
        NodeCapabilityMediaMimeType::ImagePng,
        NodeCapabilityDeclaredMediaFacts::try_image(32, 32).unwrap(),
        vec![1; 16],
    )
    .await;
    assert_deadline_failure(
        provider
            .generate_image_from_text(TextToImageProviderRequest::new(
                profile_ref(),
                execution_context(6, Instant::now()),
                text("late"),
                ImageAspectRatio::Square,
            ))
            .await,
    );
}

pub async fn assert_image_to_video_provider_contract(
    provider: &impl ImageToVideoProviderInterface,
) {
    let image_bytes = vec![4; 12];
    let context = execution_context(7, future_instant());
    let request = ImageToVideoProviderRequest::new(
        profile_ref(),
        context.clone(),
        readable_image(image_bytes.clone()),
        Some(text("slow camera")),
        ImageToVideoDurationSeconds::Ten,
    );
    assert_eq!(request.profile_ref(), &profile_ref());
    assert_context(request.context(), &context);
    assert_eq!(request.image().media_reference(), image_reference(6, digest(&image_bytes)));
    assert_eq!(
        request.image().facts(),
        NodeCapabilityDeclaredMediaFacts::try_image(32, 32).unwrap()
    );
    assert_eq!(request.image().source().byte_length(), image_bytes.len() as u64);
    assert_eq!(request.image().source().digest(), digest(&image_bytes));
    assert_eq!(request.prompt().unwrap().parts(), text("slow camera").parts());
    assert_eq!(request.duration_seconds(), ImageToVideoDurationSeconds::Ten);
    let payload = provider.generate_video_from_image(request).await.unwrap();
    assert_payload(
        payload.mime_type(),
        payload.facts(),
        payload.into_source(),
        NodeCapabilityMediaMimeType::VideoMp4,
        NodeCapabilityDeclaredMediaFacts::try_video(32, 32, 5_000, false).unwrap(),
        vec![2; 24],
    )
    .await;
    assert_deadline_failure(
        provider
            .generate_video_from_image(ImageToVideoProviderRequest::new(
                profile_ref(),
                execution_context(8, Instant::now()),
                readable_image(image_bytes),
                None,
                ImageToVideoDurationSeconds::Five,
            ))
            .await,
    );
}

pub async fn assert_text_to_speech_provider_contract(
    provider: &impl TextToSpeechProviderInterface,
) {
    let context = execution_context(9, future_instant());
    let request = TextToSpeechProviderRequest::new(profile_ref(), context.clone(), text("hello"));
    assert_eq!(request.profile_ref(), &profile_ref());
    assert_context(request.context(), &context);
    assert_eq!(request.text().parts(), text("hello").parts());
    let payload = provider.synthesize_speech_from_text(request).await.unwrap();
    assert_payload(
        payload.mime_type(),
        payload.facts(),
        payload.into_source(),
        NodeCapabilityMediaMimeType::AudioMpeg,
        NodeCapabilityDeclaredMediaFacts::try_audio(1_000, 44_100, 1).unwrap(),
        vec![3; 12],
    )
    .await;
    assert_deadline_failure(
        provider
            .synthesize_speech_from_text(TextToSpeechProviderRequest::new(
                profile_ref(),
                execution_context(10, Instant::now()),
                text("late"),
            ))
            .await,
    );
}

async fn assert_payload(
    observed_mime: NodeCapabilityMediaMimeType,
    observed_facts: NodeCapabilityDeclaredMediaFacts,
    source: NodeCapabilityMediaSourceLease,
    expected_mime: NodeCapabilityMediaMimeType,
    expected_facts: NodeCapabilityDeclaredMediaFacts,
    expected_bytes: Vec<u8>,
) {
    assert_eq!(observed_mime, expected_mime);
    assert_eq!(observed_facts, expected_facts);
    assert_eq!(source.byte_length(), expected_bytes.len() as u64);
    assert_eq!(source.digest(), digest(&expected_bytes));
    let mut stream = source.try_take_stream().unwrap();
    let mut actual_bytes = Vec::new();
    stream.read_to_end(&mut actual_bytes).await.unwrap();
    assert_eq!(actual_bytes, expected_bytes);
}

fn assert_deadline_failure<T>(
    result: Result<T, engine::node_capability::NodeCapabilityProviderFailure>,
) {
    let error = result.err().unwrap();
    assert_eq!(error.category(), NodeCapabilityProviderFailureCategory::DeadlineExceeded);
    assert!(error.is_retryable());
    assert_eq!(error.safe_retry_at(), None);
}

fn assert_context(actual: &WorkflowNodeExecutionContext, expected: &WorkflowNodeExecutionContext) {
    assert_eq!(actual.project_id, expected.project_id);
    assert_eq!(actual.workflow_run_id, expected.workflow_run_id);
    assert_eq!(actual.node_execution_id, expected.node_execution_id);
    assert_eq!(actual.deadline.monotonic_instant(), expected.deadline.monotonic_instant());
    assert_eq!(actual.cancellation.is_cancelled(), expected.cancellation.is_cancelled());
}

fn readable_image(bytes: Vec<u8>) -> NodeCapabilityReadableImageInput {
    NodeCapabilityReadableImageInput::try_new(
        image_reference(6, digest(&bytes)),
        NodeCapabilityMediaMimeType::ImagePng,
        NodeCapabilityDeclaredMediaFacts::try_image(32, 32).unwrap(),
        source(bytes, future_instant()),
    )
    .unwrap()
}

fn execution_context(seed: u8, deadline: Instant) -> WorkflowNodeExecutionContext {
    WorkflowNodeExecutionContext {
        project_id: ProjectId::from_uuid(uuid(seed)).unwrap(),
        workflow_run_id: WorkflowRunId::from_uuid(uuid(seed.wrapping_add(30))).unwrap(),
        node_execution_id: WorkflowNodeExecutionId::from_uuid(uuid(seed.wrapping_add(60))).unwrap(),
        deadline: NodeCapabilityExecutionDeadline::at(deadline),
        cancellation: NodeCapabilityExecutionCancellation::active(),
    }
}

fn image_reference(seed: u8, digest: NodeCapabilityMediaContentDigest) -> WorkflowManagedImageRef {
    WorkflowManagedImageRef::new(
        WorkflowManagedAssetIdBoundaryValue::from_bytes(uuid(seed).into_bytes()).unwrap(),
        WorkflowManagedContentFingerprint::from_bytes(digest.as_bytes()),
    )
}

fn profile_ref() -> GenerationProfileRef {
    GenerationProfileRef::new(
        GenerationProfileId::try_new("openai.image").unwrap(),
        GenerationProfileVersion::try_new(1).unwrap(),
    )
}

fn text(value: &str) -> WorkflowTextValue {
    WorkflowTextValue::try_new([WorkflowTextPart::Literal(value.to_owned())]).unwrap()
}

fn source(bytes: Vec<u8>, deadline: Instant) -> NodeCapabilityMediaSourceLease {
    NodeCapabilityMediaSourceLease::try_new(
        bytes.len() as u64,
        digest(&bytes),
        deadline,
        Box::pin(Cursor::new(bytes)),
    )
    .unwrap()
}

fn digest(bytes: &[u8]) -> NodeCapabilityMediaContentDigest {
    NodeCapabilityMediaContentDigest::from_bytes(Sha256::digest(bytes).into())
}

fn future_instant() -> Instant {
    Instant::now() + Duration::from_secs(5)
}
fn uuid(seed: u8) -> Uuid {
    let mut bytes = [seed; 16];
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}
