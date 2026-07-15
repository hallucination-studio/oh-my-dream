use std::io::Cursor;
use std::time::{Duration, Instant};

use engine::node_capability::{
    NodeCapabilityExecutionCancellation, NodeCapabilityExecutionDeadline,
    NodeCapabilityMediaFailure, NodeCapabilityOutputKey, NodeCapabilityProviderFailureCategory,
    WorkflowManagedAssetIdBoundaryValue, WorkflowManagedContentFingerprint,
    WorkflowManagedImageRef, WorkflowManagedVideoRef, WorkflowNodeExecutionContext,
    WorkflowNodeExecutionId, WorkflowRunId, WorkflowTextPart, WorkflowTextValue,
};
use nodes::*;
use projects::project::domain::ProjectId;
use sha2::{Digest, Sha256};
use tokio::io::AsyncReadExt;
use uuid::Uuid;

#[tokio::test]
async fn managed_media_reader_fake_impl_satisfies_reader_contract() {
    let reader = NodeCapabilityManagedMediaReaderFakeImpl::default();
    let owning_project_id = project_id(1);
    let other_project_id = project_id(2);
    let bytes = vec![7; 20];
    let reference = image_reference(3, digest(&bytes));
    reader
        .register_managed_media(
            owning_project_id,
            NodeCapabilityManagedMediaReference::Image(reference),
            NodeCapabilityMediaMimeType::ImagePng,
            NodeCapabilityDeclaredMediaFacts::try_image(32, 32).unwrap(),
            bytes.clone(),
        )
        .unwrap();
    assert_managed_media_reader_contract(
        &reader,
        owning_project_id,
        other_project_id,
        reference,
        bytes,
    )
    .await;
}

async fn assert_managed_media_reader_contract(
    reader: &impl NodeCapabilityManagedMediaReaderInterface,
    owning_project_id: ProjectId,
    other_project_id: ProjectId,
    reference: WorkflowManagedImageRef,
    bytes: Vec<u8>,
) {
    let missing = reader
        .read_managed_media(NodeCapabilityManagedMediaReadRequest::new(
            other_project_id,
            NodeCapabilityManagedMediaReference::Image(reference),
            future_instant(),
        ))
        .await;
    let Err(missing) = missing else { panic!("other Project unexpectedly read media") };
    assert_eq!(
        missing,
        NodeCapabilityMediaBoundaryError::Media(NodeCapabilityMediaFailure::Unavailable)
    );
    let wrong_kind = reader
        .read_managed_media(NodeCapabilityManagedMediaReadRequest::new(
            owning_project_id,
            NodeCapabilityManagedMediaReference::Video(WorkflowManagedVideoRef::new(
                reference.asset_id(),
                reference.content_fingerprint(),
            )),
            future_instant(),
        ))
        .await;
    let Err(wrong_kind) = wrong_kind else { panic!("wrong media kind unexpectedly succeeded") };
    assert_eq!(
        wrong_kind,
        NodeCapabilityMediaBoundaryError::Media(NodeCapabilityMediaFailure::KindMismatch)
    );

    let readable = reader
        .read_managed_media(NodeCapabilityManagedMediaReadRequest::new(
            owning_project_id,
            NodeCapabilityManagedMediaReference::Image(reference),
            future_instant(),
        ))
        .await
        .unwrap();
    let NodeCapabilityReadableMediaInput::Image(readable) = readable else {
        panic!("reader returned the wrong typed media")
    };
    assert_eq!(readable.media_reference(), reference);
    assert_eq!(readable.mime_type(), NodeCapabilityMediaMimeType::ImagePng);
    assert_eq!(read_stream(readable.into_source()).await, bytes);
}

#[tokio::test]
async fn produced_media_writer_fake_impl_satisfies_writer_contract() {
    let writer = NodeCapabilityProducedMediaWriterFakeImpl::default();
    assert_produced_media_writer_contract(&writer).await;
}

async fn assert_produced_media_writer_contract(
    writer: &impl NodeCapabilityProducedMediaWriterInterface,
) {
    let context = execution_context(4, future_instant());
    let key = output_key(&context);
    let first = writer
        .write_node_output_media(write_request(context.clone(), key.clone(), vec![8; 16]))
        .await
        .unwrap();
    let replay = writer
        .write_node_output_media(write_request(context.clone(), key.clone(), vec![8; 16]))
        .await
        .unwrap();
    assert_eq!(first, replay);

    let other_key = NodeCapabilityProducedMediaOutputKey::new(
        context.workflow_run_id,
        context.node_execution_id,
        NodeCapabilityOutputKey::new("image").unwrap(),
        1,
    );
    let other_output = writer
        .write_node_output_media(write_request(context.clone(), other_key, vec![8; 16]))
        .await
        .unwrap();
    assert_ne!(first, other_output);

    let mut other_project_context = context.clone();
    other_project_context.project_id = project_id(99);
    let other_project_output = writer
        .write_node_output_media(write_request(other_project_context, key.clone(), vec![8; 16]))
        .await
        .unwrap();
    assert_ne!(first, other_project_output);

    let conflict = writer.write_node_output_media(write_request(context, key, vec![9; 16])).await;
    assert_eq!(
        conflict.unwrap_err(),
        NodeCapabilityMediaBoundaryError::Media(NodeCapabilityMediaFailure::OutputConflict)
    );
}

#[tokio::test]
async fn text_to_image_provider_fake_impl_satisfies_provider_contract() {
    let provider = TextToImageProviderFakeImpl::try_new().unwrap();
    assert_text_to_image_provider_contract(&provider).await;
}

async fn assert_text_to_image_provider_contract(provider: &impl TextToImageProviderInterface) {
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
    assert_image_to_video_provider_contract(&provider).await;
}

async fn assert_image_to_video_provider_contract(provider: &impl ImageToVideoProviderInterface) {
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
    assert_text_to_speech_provider_contract(&provider).await;
}

async fn assert_text_to_speech_provider_contract(provider: &impl TextToSpeechProviderInterface) {
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

fn write_request(
    context: WorkflowNodeExecutionContext,
    key: NodeCapabilityProducedMediaOutputKey,
    bytes: Vec<u8>,
) -> NodeCapabilityProducedMediaWriteRequest {
    let payload = GeneratedImagePayload::try_new(
        NodeCapabilityDeclaredMediaFacts::try_image(32, 32).unwrap(),
        source(bytes, future_instant()),
    )
    .unwrap();
    NodeCapabilityProducedMediaWriteRequest::try_new(
        context,
        key,
        NodeCapabilityProducedMediaDisplayName::try_new("Generated image").unwrap(),
        NodeCapabilityProducedMediaProvenance::provider_generated(profile_ref()),
        NodeCapabilityProducedMediaPayload::GeneratedImage(payload),
    )
    .unwrap()
}

fn output_key(context: &WorkflowNodeExecutionContext) -> NodeCapabilityProducedMediaOutputKey {
    NodeCapabilityProducedMediaOutputKey::new(
        context.workflow_run_id,
        context.node_execution_id,
        NodeCapabilityOutputKey::new("image").unwrap(),
        0,
    )
}

fn execution_context(seed: u8, deadline: Instant) -> WorkflowNodeExecutionContext {
    WorkflowNodeExecutionContext {
        project_id: project_id(seed),
        workflow_run_id: WorkflowRunId::from_uuid(uuid(seed.wrapping_add(30))).unwrap(),
        node_execution_id: WorkflowNodeExecutionId::from_uuid(uuid(seed.wrapping_add(60))).unwrap(),
        deadline: NodeCapabilityExecutionDeadline::at(deadline),
        cancellation: NodeCapabilityExecutionCancellation::active(),
    }
}

fn project_id(seed: u8) -> ProjectId {
    ProjectId::from_uuid(uuid(seed)).unwrap()
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
