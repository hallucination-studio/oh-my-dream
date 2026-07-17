use std::io::Cursor;
use std::time::{Duration, Instant};

use engine::node_capability::{
    NodeCapabilityExecutionCancellation, NodeCapabilityExecutionDeadline,
    NodeCapabilityMediaFailure, NodeCapabilityOutputKey, WorkflowManagedAssetIdBoundaryValue,
    WorkflowManagedContentFingerprint, WorkflowManagedImageRef, WorkflowManagedVideoRef,
    WorkflowNodeExecutionContext, WorkflowNodeExecutionId, WorkflowNodeExecutionOrigin,
    WorkflowRunId,
};
use engine::workflow_graph::{WorkflowId, WorkflowNodeId, WorkflowRevision};
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
    let conflicting_reference =
        WorkflowManagedVideoRef::new(reference.asset_id(), reference.content_fingerprint());
    let conflict = reader.register_managed_media(
        owning_project_id,
        NodeCapabilityManagedMediaReference::Video(conflicting_reference),
        NodeCapabilityMediaMimeType::VideoMp4,
        NodeCapabilityDeclaredMediaFacts::try_video(32, 32, 1_000, false).unwrap(),
        bytes.clone(),
    );
    assert_eq!(conflict, Err(NodeCapabilityMediaValueError::InvalidMediaFacts));
    assert_managed_media_reader_contract(
        &reader,
        owning_project_id,
        other_project_id,
        reference,
        bytes,
    )
    .await;
    assert_managed_media_selection_contract(&reader, owning_project_id, reference).await;
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
            NodeCapabilityManagedMediaReadSelection::ExactReference(
                NodeCapabilityManagedMediaReference::Image(reference),
            ),
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
            NodeCapabilityManagedMediaReadSelection::ExactReference(
                NodeCapabilityManagedMediaReference::Video(WorkflowManagedVideoRef::new(
                    reference.asset_id(),
                    reference.content_fingerprint(),
                )),
            ),
            future_instant(),
        ))
        .await;
    let Err(wrong_kind) = wrong_kind else { panic!("wrong media kind unexpectedly succeeded") };
    assert_eq!(
        wrong_kind,
        NodeCapabilityMediaBoundaryError::Media(NodeCapabilityMediaFailure::KindMismatch {
            expected: engine::node_capability::WorkflowDataType::Video,
            observed: engine::node_capability::WorkflowDataType::Image,
        })
    );

    let readable = reader
        .read_managed_media(NodeCapabilityManagedMediaReadRequest::new(
            owning_project_id,
            NodeCapabilityManagedMediaReadSelection::ExactReference(
                NodeCapabilityManagedMediaReference::Image(reference),
            ),
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

async fn assert_managed_media_selection_contract(
    reader: &impl NodeCapabilityManagedMediaReaderInterface,
    owning_project_id: ProjectId,
    reference: WorkflowManagedImageRef,
) {
    let by_asset_id = reader
        .read_managed_media(NodeCapabilityManagedMediaReadRequest::new(
            owning_project_id,
            NodeCapabilityManagedMediaReadSelection::AssetId(
                NodeCapabilityAssetIdMediaReadSelection::new(
                    reference.asset_id(),
                    NodeCapabilityMediaKind::Image,
                ),
            ),
            future_instant(),
        ))
        .await
        .unwrap();
    let NodeCapabilityReadableMediaInput::Image(by_asset_id) = by_asset_id else {
        panic!("Asset-ID selection returned the wrong type")
    };
    assert_eq!(by_asset_id.media_reference(), reference);

    let stale_reference = WorkflowManagedImageRef::new(
        reference.asset_id(),
        WorkflowManagedContentFingerprint::from_bytes([99; 32]),
    );
    let stale = reader
        .read_managed_media(NodeCapabilityManagedMediaReadRequest::new(
            owning_project_id,
            NodeCapabilityManagedMediaReadSelection::ExactReference(
                NodeCapabilityManagedMediaReference::Image(stale_reference),
            ),
            future_instant(),
        ))
        .await;
    let Err(stale) = stale else { panic!("stale fingerprint unexpectedly resolved") };
    assert_eq!(
        stale,
        NodeCapabilityMediaBoundaryError::Media(NodeCapabilityMediaFailure::DigestMismatch)
    );

    let expired = reader
        .read_managed_media(NodeCapabilityManagedMediaReadRequest::new(
            owning_project_id,
            NodeCapabilityManagedMediaReadSelection::ExactReference(
                NodeCapabilityManagedMediaReference::Image(reference),
            ),
            Instant::now(),
        ))
        .await;
    let Err(expired) = expired else { panic!("expired read unexpectedly succeeded") };
    assert_eq!(expired, NodeCapabilityMediaBoundaryError::DeadlineExceeded);
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

    let origin_conflict = writer
        .write_node_output_media(write_request_with_origin_seed(
            context.clone(),
            key.clone(),
            vec![8; 16],
            5,
        ))
        .await;
    assert_eq!(
        origin_conflict.unwrap_err(),
        NodeCapabilityMediaBoundaryError::Media(NodeCapabilityMediaFailure::OutputConflict)
    );

    let conflict = writer.write_node_output_media(write_request(context, key, vec![9; 16])).await;
    assert_eq!(
        conflict.unwrap_err(),
        NodeCapabilityMediaBoundaryError::Media(NodeCapabilityMediaFailure::OutputConflict)
    );
    assert_writer_interruptions_and_digest_validation(writer).await;
}

async fn assert_writer_interruptions_and_digest_validation(
    writer: &impl NodeCapabilityProducedMediaWriterInterface,
) {
    let cancelled_context = execution_context(70, future_instant());
    cancelled_context.cancellation.cancel();
    let cancelled_key = output_key(&cancelled_context);
    let cancelled = writer
        .write_node_output_media(write_request(cancelled_context, cancelled_key, vec![1; 16]))
        .await;
    assert_eq!(cancelled.unwrap_err(), NodeCapabilityMediaBoundaryError::Cancelled);

    let expired_context = execution_context(71, Instant::now());
    let expired_key = output_key(&expired_context);
    let expired = writer
        .write_node_output_media(write_request(expired_context, expired_key, vec![1; 16]))
        .await;
    assert_eq!(expired.unwrap_err(), NodeCapabilityMediaBoundaryError::DeadlineExceeded);

    let context = execution_context(72, future_instant());
    let key = output_key(&context);
    let mismatched = writer
        .write_node_output_media(write_request_with_digest(context, key, vec![1; 16], digest(&[9])))
        .await;
    assert_eq!(
        mismatched.unwrap_err(),
        NodeCapabilityMediaBoundaryError::Media(NodeCapabilityMediaFailure::DigestMismatch)
    );
}

fn write_request(
    context: WorkflowNodeExecutionContext,
    key: NodeCapabilityProducedMediaOutputKey,
    bytes: Vec<u8>,
) -> NodeCapabilityProducedMediaWriteRequest {
    write_request_with_origin_seed(context, key, bytes, 4)
}

fn write_request_with_origin_seed(
    context: WorkflowNodeExecutionContext,
    key: NodeCapabilityProducedMediaOutputKey,
    bytes: Vec<u8>,
    origin_seed: u8,
) -> NodeCapabilityProducedMediaWriteRequest {
    let payload = GeneratedImagePayload::try_new(
        NodeCapabilityDeclaredMediaFacts::try_image(32, 32).unwrap(),
        source(bytes, future_instant()),
    )
    .unwrap();
    NodeCapabilityProducedMediaWriteRequest::try_new(
        context,
        execution_origin(origin_seed),
        key,
        NodeCapabilityProducedMediaDisplayName::try_new("Generated image").unwrap(),
        NodeCapabilityProducedMediaProvenance::provider_generated(profile_ref()),
        NodeCapabilityProducedMediaPayload::GeneratedImage(payload),
    )
    .unwrap()
}

fn write_request_with_digest(
    context: WorkflowNodeExecutionContext,
    key: NodeCapabilityProducedMediaOutputKey,
    bytes: Vec<u8>,
    declared_digest: NodeCapabilityMediaContentDigest,
) -> NodeCapabilityProducedMediaWriteRequest {
    let source = NodeCapabilityMediaSourceLease::try_new(
        bytes.len() as u64,
        declared_digest,
        future_instant(),
        Box::pin(Cursor::new(bytes)),
    )
    .unwrap();
    let payload = GeneratedImagePayload::try_new(
        NodeCapabilityDeclaredMediaFacts::try_image(32, 32).unwrap(),
        source,
    )
    .unwrap();
    NodeCapabilityProducedMediaWriteRequest::try_new(
        context,
        execution_origin(4),
        key,
        NodeCapabilityProducedMediaDisplayName::try_new("Generated image").unwrap(),
        NodeCapabilityProducedMediaProvenance::provider_generated(profile_ref()),
        NodeCapabilityProducedMediaPayload::GeneratedImage(payload),
    )
    .unwrap()
}

fn execution_origin(seed: u8) -> WorkflowNodeExecutionOrigin {
    WorkflowNodeExecutionOrigin::new(
        WorkflowId::from_uuid(uuid(seed.wrapping_add(90))).unwrap(),
        WorkflowRevision::new(u64::from(seed) + 1).unwrap(),
        WorkflowNodeId::from_uuid(uuid(seed.wrapping_add(120))).unwrap(),
        engine::node_capability::NodeCapabilityContractRef::new(
            engine::node_capability::NodeCapabilityContractId::new("image.generate_from_text")
                .unwrap(),
            engine::node_capability::NodeCapabilityContractVersion::new(1, 0).unwrap(),
        ),
    )
}

fn profile_ref() -> GenerationProfileRef {
    GenerationProfileRef::new(
        GenerationProfileId::try_new("profile.image.standard").unwrap(),
        GenerationProfileVersion::try_new(1).unwrap(),
    )
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
