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

#[tokio::test]
async fn text_to_image_provider_fake_impl_satisfies_provider_contract() {
    let provider = TextToImageProviderFakeImpl::try_new().unwrap();
    let payload = provider
        .generate_image_from_text(TextToImageProviderRequest::new(
            profile_ref(),
            execution_context(5, future_instant()),
            text("draw a moon"),
            ImageAspectRatio::Square,
        ))
        .await
        .unwrap();
    assert_eq!(payload.mime_type(), NodeCapabilityMediaMimeType::ImagePng);
    assert_eq!(payload.facts(), NodeCapabilityDeclaredMediaFacts::try_image(32, 32).unwrap());
    assert_eq!(read_stream(payload.into_source()).await, vec![1; 16]);
}

#[tokio::test]
async fn image_to_video_provider_fake_impl_satisfies_provider_contract() {
    let provider = ImageToVideoProviderFakeImpl::try_new().unwrap();
    let image_bytes = vec![4; 12];
    let image = NodeCapabilityReadableImageInput::try_new(
        image_reference(6, digest(&image_bytes)),
        NodeCapabilityMediaMimeType::ImagePng,
        NodeCapabilityDeclaredMediaFacts::try_image(32, 32).unwrap(),
        source(image_bytes, future_instant()),
    )
    .unwrap();
    let payload = provider
        .generate_video_from_image(ImageToVideoProviderRequest::new(
            profile_ref(),
            execution_context(7, future_instant()),
            image,
            Some(text("slow camera")),
            ImageToVideoDurationSeconds::Five,
        ))
        .await
        .unwrap();
    assert_eq!(payload.mime_type(), NodeCapabilityMediaMimeType::VideoMp4);
    assert_eq!(read_stream(payload.into_source()).await, vec![2; 24]);
}

#[tokio::test]
async fn text_to_speech_provider_fake_impl_satisfies_provider_contract() {
    let provider = TextToSpeechProviderFakeImpl::try_new().unwrap();
    let payload = provider
        .synthesize_speech_from_text(TextToSpeechProviderRequest::new(
            profile_ref(),
            execution_context(8, future_instant()),
            text("hello"),
        ))
        .await
        .unwrap();
    assert_eq!(payload.mime_type(), NodeCapabilityMediaMimeType::AudioMpeg);
    assert_eq!(read_stream(payload.into_source()).await, vec![3; 12]);

    let failure = provider
        .synthesize_speech_from_text(TextToSpeechProviderRequest::new(
            profile_ref(),
            execution_context(9, Instant::now()),
            text("late"),
        ))
        .await;
    let Err(failure) = failure else { panic!("expired provider request unexpectedly succeeded") };
    assert_eq!(failure.category(), NodeCapabilityProviderFailureCategory::DeadlineExceeded);
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

async fn read_stream(source: NodeCapabilityMediaSourceLease) -> Vec<u8> {
    let mut stream = source.try_take_stream().unwrap();
    let mut bytes = Vec::new();
    stream.read_to_end(&mut bytes).await.unwrap();
    bytes
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
