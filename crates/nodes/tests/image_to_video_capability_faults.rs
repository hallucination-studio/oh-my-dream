use std::collections::BTreeMap;
use std::io::Cursor;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use engine::node_capability::*;
use engine::workflow_graph::{WorkflowId, WorkflowNodeId, WorkflowRevision};
use nodes::*;
use projects::project::domain::ProjectId;
use sha2::{Digest, Sha256};
use uuid::Uuid;

mod c5_support;
use c5_support::GenerationProfileAlwaysAvailableFakeImpl;

enum ReaderOutcome {
    Failure(NodeCapabilityMediaBoundaryError),
    CancelAndReturn(NodeCapabilityExecutionCancellation),
    DelayAndReturn,
    MustNotBeCalled,
}

struct FaultManagedImageReaderImpl {
    outcome: ReaderOutcome,
    image: WorkflowManagedImageRef,
    bytes: Vec<u8>,
}

#[async_trait]
impl NodeCapabilityManagedMediaReaderInterface for FaultManagedImageReaderImpl {
    async fn read_managed_media(
        &self,
        _request: NodeCapabilityManagedMediaReadRequest,
    ) -> Result<NodeCapabilityReadableMediaInput, NodeCapabilityMediaBoundaryError> {
        match &self.outcome {
            ReaderOutcome::Failure(error) => return Err(error.clone()),
            ReaderOutcome::CancelAndReturn(cancellation) => cancellation.cancel(),
            ReaderOutcome::DelayAndReturn => tokio::time::sleep(Duration::from_millis(20)).await,
            ReaderOutcome::MustNotBeCalled => panic!("reader must not be called"),
        }
        let source = NodeCapabilityMediaSourceLease::try_new(
            self.bytes.len() as u64,
            digest(&self.bytes),
            future_instant(),
            Box::pin(Cursor::new(self.bytes.clone())),
        )
        .unwrap();
        Ok(NodeCapabilityReadableMediaInput::Image(
            NodeCapabilityReadableImageInput::try_new(
                self.image,
                NodeCapabilityMediaMimeType::ImagePng,
                NodeCapabilityDeclaredMediaFacts::try_image(32, 32).unwrap(),
                source,
            )
            .unwrap(),
        ))
    }
}

#[tokio::test]
async fn every_image_reader_failure_keeps_resolve_inputs_and_image_target() {
    let media_failures = [
        NodeCapabilityMediaFailure::Unavailable,
        NodeCapabilityMediaFailure::KindMismatch {
            expected: WorkflowDataType::Image,
            observed: WorkflowDataType::Video,
        },
        NodeCapabilityMediaFailure::InvalidMedia,
        NodeCapabilityMediaFailure::SizeLimitExceeded,
        NodeCapabilityMediaFailure::DigestMismatch,
        NodeCapabilityMediaFailure::OutputConflict,
        NodeCapabilityMediaFailure::StorageFailed,
        NodeCapabilityMediaFailure::InspectionFailed,
        NodeCapabilityMediaFailure::FinalizationFailed,
    ];
    for (index, failure) in media_failures.into_iter().enumerate() {
        let seed = index as u8 + 1;
        let error = execute_reader_outcome(
            ReaderOutcome::Failure(NodeCapabilityMediaBoundaryError::Media(failure)),
            request_context(seed, future_instant(), NodeCapabilityExecutionCancellation::active()),
        )
        .await;
        assert_reader_error(&error, NodeCapabilityExecutionFailure::Media(failure), seed);
    }
    for (seed, boundary, failure) in [
        (
            20,
            NodeCapabilityMediaBoundaryError::Cancelled,
            NodeCapabilityExecutionFailure::Cancelled,
        ),
        (
            21,
            NodeCapabilityMediaBoundaryError::DeadlineExceeded,
            NodeCapabilityExecutionFailure::DeadlineExceeded,
        ),
    ] {
        let error = execute_reader_outcome(
            ReaderOutcome::Failure(boundary),
            request_context(seed, future_instant(), NodeCapabilityExecutionCancellation::active()),
        )
        .await;
        assert_reader_error(&error, failure, seed);
    }
}

#[tokio::test]
async fn cancellation_and_deadline_are_observed_after_image_reader_await() {
    let cancellation = NodeCapabilityExecutionCancellation::active();
    let cancelled = execute_reader_outcome(
        ReaderOutcome::CancelAndReturn(cancellation.clone()),
        request_context(30, future_instant(), cancellation),
    )
    .await;
    assert_reader_error(&cancelled, NodeCapabilityExecutionFailure::Cancelled, 30);

    let deadline = execute_reader_outcome(
        ReaderOutcome::DelayAndReturn,
        request_context(31, near_deadline(), NodeCapabilityExecutionCancellation::active()),
    )
    .await;
    assert_reader_error(&deadline, NodeCapabilityExecutionFailure::DeadlineExceeded, 31);
}

#[tokio::test]
async fn cancellation_precedes_deadline_before_image_reader_dispatch() {
    let cancellation = NodeCapabilityExecutionCancellation::active();
    cancellation.cancel();
    let error = execute_reader_outcome(
        ReaderOutcome::MustNotBeCalled,
        request_context(32, Instant::now(), cancellation),
    )
    .await;
    assert_reader_error(&error, NodeCapabilityExecutionFailure::Cancelled, 32);
}

async fn execute_reader_outcome(
    outcome: ReaderOutcome,
    context: WorkflowNodeExecutionContext,
) -> NodeCapabilityExecutionError {
    let image = image_reference(40);
    let capability = ImageToVideoCapabilityImpl::try_new(
        catalog(),
        GenerationProfileAlwaysAvailableFakeImpl,
        FaultManagedImageReaderImpl { outcome, image, bytes: vec![4; 16] },
        ImageToVideoProviderFakeImpl::try_new().unwrap(),
        NodeCapabilityProducedMediaWriterFakeImpl::default(),
    )
    .unwrap();
    let request = execution_request(&capability, context, image);
    capability.execute_node_capability(request).await.unwrap_err()
}

fn execution_request(
    capability: &impl WorkflowNodeCapabilityInterface,
    context: WorkflowNodeExecutionContext,
    image: WorkflowManagedImageRef,
) -> NodeCapabilityExecutionRequest {
    let parameters = NodeCapabilityParameterSet::try_from_map(BTreeMap::from([(
        NodeCapabilityParameterKey::new("generation_profile_ref").unwrap(),
        NodeCapabilityParameterValue::GenerationProfile(
            profile().to_node_capability_parameter_value().unwrap(),
        ),
    )]))
    .unwrap();
    let inputs = WorkflowNodeInputSet::try_new(
        capability.node_capability_contract(),
        BTreeMap::from([(
            NodeCapabilityInputKey::new("image").unwrap(),
            WorkflowNodeInputValue::Single(WorkflowRuntimeInputItem {
                input_item_id: WorkflowInputItemId::from_uuid(uuid(50)).unwrap(),
                input_role_key: None,
                value: WorkflowRuntimeValue::Image(image),
            }),
        )]),
    )
    .unwrap();
    NodeCapabilityExecutionRequest {
        origin: WorkflowNodeExecutionOrigin::new(
            WorkflowId::from_uuid(uuid(90)).unwrap(),
            WorkflowRevision::new(1).unwrap(),
            WorkflowNodeId::from_uuid(uuid(120)).unwrap(),
            contract_ref(),
        ),
        normalized_parameters: capability.normalize_node_parameters(&parameters).unwrap(),
        context,
        inputs,
    }
}

fn assert_reader_error(
    error: &NodeCapabilityExecutionError,
    failure: NodeCapabilityExecutionFailure,
    seed: u8,
) {
    assert_eq!(error.contract_ref(), &contract_ref());
    assert_eq!(
        error.node_execution_id(),
        WorkflowNodeExecutionId::from_uuid(uuid(seed + 60)).unwrap()
    );
    assert_eq!(error.stage(), NodeCapabilityExecutionStage::ResolveInputs);
    assert_eq!(error.failure(), &failure);
    assert_eq!(
        error.target(),
        &NodeCapabilityExecutionTarget::Input(NodeCapabilityInputKey::new("image").unwrap())
    );
}

fn request_context(
    seed: u8,
    deadline: Instant,
    cancellation: NodeCapabilityExecutionCancellation,
) -> WorkflowNodeExecutionContext {
    WorkflowNodeExecutionContext {
        project_id: ProjectId::from_uuid(uuid(seed)).unwrap(),
        workflow_run_id: WorkflowRunId::from_uuid(uuid(seed + 30)).unwrap(),
        node_execution_id: WorkflowNodeExecutionId::from_uuid(uuid(seed + 60)).unwrap(),
        deadline: NodeCapabilityExecutionDeadline::at(deadline),
        cancellation,
    }
}
fn image_reference(seed: u8) -> WorkflowManagedImageRef {
    let bytes = vec![4; 16];
    WorkflowManagedImageRef::new(
        WorkflowManagedAssetIdBoundaryValue::from_bytes(uuid(seed).into_bytes()).unwrap(),
        WorkflowManagedContentFingerprint::from_bytes(digest(&bytes).as_bytes()),
    )
}
fn digest(bytes: &[u8]) -> NodeCapabilityMediaContentDigest {
    NodeCapabilityMediaContentDigest::from_bytes(Sha256::digest(bytes).into())
}
fn catalog() -> Arc<GenerationProfileCatalog> {
    Arc::new(GenerationProfileCatalog::frozen_mvp().unwrap())
}
fn profile() -> GenerationProfileRef {
    catalog().list_active_generation_profiles_for_capability(&contract_ref())[0]
        .profile_ref()
        .clone()
}
fn contract_ref() -> NodeCapabilityContractRef {
    NodeCapabilityContractRef::new(
        NodeCapabilityContractId::new("video.generate_from_image").unwrap(),
        NodeCapabilityContractVersion::new(1, 0).unwrap(),
    )
}
fn future_instant() -> Instant {
    Instant::now() + Duration::from_secs(5)
}
fn near_deadline() -> Instant {
    Instant::now() + Duration::from_millis(10)
}
fn uuid(seed: u8) -> Uuid {
    let mut bytes = [seed; 16];
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}
