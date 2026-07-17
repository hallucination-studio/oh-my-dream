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

mod c5_support;
use c5_support::GenerationProfileAlwaysAvailableFakeImpl;
#[path = "c5_support/faults.rs"]
mod fault_support;
use fault_support::uuid;

enum ProviderOutcome {
    Image(Vec<u8>),
    Failure(NodeCapabilityProviderFailure),
    MustNotBeCalled,
}

struct FaultTextToImageProviderImpl(ProviderOutcome);

#[async_trait]
impl TextToImageProviderInterface for FaultTextToImageProviderImpl {
    async fn generate_image_from_text(
        &self,
        request: TextToImageProviderRequest,
    ) -> Result<GeneratedImagePayload, NodeCapabilityProviderFailure> {
        match &self.0 {
            ProviderOutcome::Image(bytes) => Ok(GeneratedImagePayload::try_new(
                NodeCapabilityDeclaredMediaFacts::try_image(32, 32).unwrap(),
                source(bytes.clone(), request.context().deadline.monotonic_instant()),
            )
            .unwrap()),
            ProviderOutcome::Failure(error) => Err(error.clone()),
            ProviderOutcome::MustNotBeCalled => panic!("provider must not be called"),
        }
    }
}

enum WriterOutcome {
    MatchingImage,
    WrongKind,
    WrongFingerprint,
    Failure(NodeCapabilityMediaBoundaryError),
}

struct FaultProducedMediaWriterImpl(WriterOutcome);

#[async_trait]
impl NodeCapabilityProducedMediaWriterInterface for FaultProducedMediaWriterImpl {
    async fn write_node_output_media(
        &self,
        request: NodeCapabilityProducedMediaWriteRequest,
    ) -> Result<NodeCapabilityProducedMediaReference, NodeCapabilityMediaBoundaryError> {
        let digest = request.payload().digest();
        let asset_id =
            WorkflowManagedAssetIdBoundaryValue::from_bytes(uuid(200).into_bytes()).unwrap();
        match &self.0 {
            WriterOutcome::MatchingImage => {
                Ok(NodeCapabilityProducedMediaReference::Image(WorkflowManagedImageRef::new(
                    asset_id,
                    WorkflowManagedContentFingerprint::from_bytes(digest.as_bytes()),
                )))
            }
            WriterOutcome::WrongKind => {
                Ok(NodeCapabilityProducedMediaReference::Video(WorkflowManagedVideoRef::new(
                    asset_id,
                    WorkflowManagedContentFingerprint::from_bytes(digest.as_bytes()),
                )))
            }
            WriterOutcome::WrongFingerprint => {
                Ok(NodeCapabilityProducedMediaReference::Image(WorkflowManagedImageRef::new(
                    asset_id,
                    WorkflowManagedContentFingerprint::from_bytes([9; 32]),
                )))
            }
            WriterOutcome::Failure(error) => Err(error.clone()),
        }
    }
}

#[tokio::test]
async fn classified_malformed_provider_payload_propagates_as_invalid_response() {
    let provider_failure = NodeCapabilityProviderFailure::try_new(
        NodeCapabilityProviderFailureCategory::InvalidResponse,
        false,
        Instant::now(),
        None,
    )
    .unwrap();
    let error = execute(
        ProviderOutcome::Failure(provider_failure.clone()),
        WriterOutcome::MatchingImage,
        active_request(1),
    )
    .await
    .unwrap_err();
    assert_execution_error(
        &error,
        NodeCapabilityExecutionStage::CallProvider,
        NodeCapabilityExecutionTarget::Capability,
        NodeCapabilityExecutionFailure::Provider(provider_failure),
        1,
    );
}

#[tokio::test]
async fn media_write_failure_and_output_conflict_keep_exact_output_target() {
    for failure in
        [NodeCapabilityMediaFailure::StorageFailed, NodeCapabilityMediaFailure::OutputConflict]
    {
        let error = execute(
            ProviderOutcome::Image(vec![1; 16]),
            WriterOutcome::Failure(NodeCapabilityMediaBoundaryError::Media(failure)),
            active_request(2),
        )
        .await
        .unwrap_err();
        assert_execution_error(
            &error,
            NodeCapabilityExecutionStage::WriteManagedMedia,
            NodeCapabilityExecutionTarget::Output(output_key()),
            NodeCapabilityExecutionFailure::Media(failure),
            2,
        );
    }
}

#[tokio::test]
async fn returned_reference_kind_and_fingerprint_are_validated_after_media_write() {
    for (writer, failure) in [
        (
            WriterOutcome::WrongKind,
            NodeCapabilityMediaFailure::KindMismatch {
                expected: WorkflowDataType::Image,
                observed: WorkflowDataType::Video,
            },
        ),
        (WriterOutcome::WrongFingerprint, NodeCapabilityMediaFailure::DigestMismatch),
    ] {
        let error = execute(ProviderOutcome::Image(vec![2; 16]), writer, active_request(3))
            .await
            .unwrap_err();
        assert_execution_error(
            &error,
            NodeCapabilityExecutionStage::WriteManagedMedia,
            NodeCapabilityExecutionTarget::Output(output_key()),
            NodeCapabilityExecutionFailure::Media(failure),
            3,
        );
    }
}

#[tokio::test]
async fn cancellation_and_deadline_stop_before_provider_dispatch() {
    let cancelled = active_request(4);
    cancelled.context.cancellation.cancel();
    let error = execute(ProviderOutcome::MustNotBeCalled, WriterOutcome::MatchingImage, cancelled)
        .await
        .unwrap_err();
    assert_execution_error(
        &error,
        NodeCapabilityExecutionStage::CallProvider,
        NodeCapabilityExecutionTarget::Capability,
        NodeCapabilityExecutionFailure::Cancelled,
        4,
    );

    let mut expired = active_request(5);
    expired.context.deadline = NodeCapabilityExecutionDeadline::at(Instant::now());
    let error = execute(ProviderOutcome::MustNotBeCalled, WriterOutcome::MatchingImage, expired)
        .await
        .unwrap_err();
    assert_execution_error(
        &error,
        NodeCapabilityExecutionStage::CallProvider,
        NodeCapabilityExecutionTarget::Capability,
        NodeCapabilityExecutionFailure::DeadlineExceeded,
        5,
    );
}

#[tokio::test]
async fn every_provider_failure_category_is_preserved_without_adapter_detail() {
    let categories = [
        NodeCapabilityProviderFailureCategory::InvalidSemanticRequest,
        NodeCapabilityProviderFailureCategory::AuthenticationFailed,
        NodeCapabilityProviderFailureCategory::PermissionDenied,
        NodeCapabilityProviderFailureCategory::ContentPolicyRejected,
        NodeCapabilityProviderFailureCategory::RateLimited,
        NodeCapabilityProviderFailureCategory::ProviderUnavailable,
        NodeCapabilityProviderFailureCategory::DeadlineExceeded,
        NodeCapabilityProviderFailureCategory::ProviderRejected,
        NodeCapabilityProviderFailureCategory::InvalidResponse,
        NodeCapabilityProviderFailureCategory::DownloadRejected,
        NodeCapabilityProviderFailureCategory::AmbiguousSubmission,
    ];
    for (index, category) in categories.into_iter().enumerate() {
        let seed = index as u8 + 10;
        let failure =
            NodeCapabilityProviderFailure::try_new(category, false, Instant::now(), None).unwrap();
        let error = execute(
            ProviderOutcome::Failure(failure.clone()),
            WriterOutcome::MatchingImage,
            active_request(seed),
        )
        .await
        .unwrap_err();
        assert_execution_error(
            &error,
            NodeCapabilityExecutionStage::CallProvider,
            NodeCapabilityExecutionTarget::Capability,
            NodeCapabilityExecutionFailure::Provider(failure),
            seed,
        );
    }
}

#[tokio::test]
async fn every_media_failure_and_writer_interruption_is_preserved() {
    let failures = [
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
    for (index, failure) in failures.into_iter().enumerate() {
        let seed = index as u8 + 30;
        let error = execute(
            ProviderOutcome::Image(vec![3; 16]),
            WriterOutcome::Failure(NodeCapabilityMediaBoundaryError::Media(failure)),
            active_request(seed),
        )
        .await
        .unwrap_err();
        assert_execution_error(
            &error,
            NodeCapabilityExecutionStage::WriteManagedMedia,
            NodeCapabilityExecutionTarget::Output(output_key()),
            NodeCapabilityExecutionFailure::Media(failure),
            seed,
        );
    }
    assert_writer_interruptions_are_preserved().await;
}

async fn assert_writer_interruptions_are_preserved() {
    for (seed, boundary, failure) in [
        (
            50,
            NodeCapabilityMediaBoundaryError::Cancelled,
            NodeCapabilityExecutionFailure::Cancelled,
        ),
        (
            51,
            NodeCapabilityMediaBoundaryError::DeadlineExceeded,
            NodeCapabilityExecutionFailure::DeadlineExceeded,
        ),
    ] {
        let error = execute(
            ProviderOutcome::Image(vec![4; 16]),
            WriterOutcome::Failure(boundary),
            active_request(seed),
        )
        .await
        .unwrap_err();
        assert_execution_error(
            &error,
            NodeCapabilityExecutionStage::WriteManagedMedia,
            NodeCapabilityExecutionTarget::Output(output_key()),
            failure,
            seed,
        );
    }
}

async fn execute(
    provider: ProviderOutcome,
    writer: WriterOutcome,
    request: NodeCapabilityExecutionRequest,
) -> Result<WorkflowNodeCapabilityExecutionOutcome, NodeCapabilityExecutionError> {
    capability(provider, writer).execute_node_capability(request).await
}

fn capability(
    provider: ProviderOutcome,
    writer: WriterOutcome,
) -> TextToImageCapabilityImpl<
    GenerationProfileAlwaysAvailableFakeImpl,
    FaultTextToImageProviderImpl,
    FaultProducedMediaWriterImpl,
> {
    TextToImageCapabilityImpl::try_new(
        catalog(),
        GenerationProfileAlwaysAvailableFakeImpl,
        FaultTextToImageProviderImpl(provider),
        FaultProducedMediaWriterImpl(writer),
    )
    .unwrap()
}

fn active_request(seed: u8) -> NodeCapabilityExecutionRequest {
    let capability = capability(ProviderOutcome::MustNotBeCalled, WriterOutcome::MatchingImage);
    let profile = profile();
    let parameters = NodeCapabilityParameterSet::try_from_map(BTreeMap::from([(
        NodeCapabilityParameterKey::new("generation_profile_ref").unwrap(),
        NodeCapabilityParameterValue::GenerationProfile(
            profile.to_node_capability_parameter_value().unwrap(),
        ),
    )]))
    .unwrap();
    let inputs = WorkflowNodeInputSet::try_new(
        capability.node_capability_contract(),
        BTreeMap::from([(
            NodeCapabilityInputKey::new("prompt").unwrap(),
            WorkflowNodeInputValue::Single(WorkflowRuntimeInputItem {
                input_item_id: WorkflowInputItemId::from_uuid(uuid(seed)).unwrap(),
                input_role_key: None,
                value: WorkflowRuntimeValue::Text(
                    WorkflowTextValue::try_new([WorkflowTextPart::Literal("draw".into())]).unwrap(),
                ),
            }),
        )]),
    )
    .unwrap();
    NodeCapabilityExecutionRequest {
        context: WorkflowNodeExecutionContext {
            project_id: ProjectId::from_uuid(uuid(seed)).unwrap(),
            workflow_run_id: WorkflowRunId::from_uuid(uuid(seed + 30)).unwrap(),
            node_execution_id: WorkflowNodeExecutionId::from_uuid(uuid(seed + 60)).unwrap(),
            deadline: NodeCapabilityExecutionDeadline::at(Instant::now() + Duration::from_secs(5)),
            cancellation: NodeCapabilityExecutionCancellation::active(),
        },
        origin: WorkflowNodeExecutionOrigin::new(
            WorkflowId::from_uuid(uuid(seed + 90)).unwrap(),
            WorkflowRevision::new(u64::from(seed) + 1).unwrap(),
            WorkflowNodeId::from_uuid(uuid(seed + 120)).unwrap(),
            contract_ref(),
        ),
        normalized_parameters: capability.normalize_node_parameters(&parameters).unwrap(),
        inputs,
    }
}

fn assert_execution_error(
    error: &NodeCapabilityExecutionError,
    stage: NodeCapabilityExecutionStage,
    target: NodeCapabilityExecutionTarget,
    failure: NodeCapabilityExecutionFailure,
    seed: u8,
) {
    assert_eq!(error.contract_ref(), &contract_ref());
    assert_eq!(
        error.node_execution_id(),
        WorkflowNodeExecutionId::from_uuid(uuid(seed + 60)).unwrap()
    );
    assert_eq!(error.stage(), stage);
    assert_eq!(error.target(), &target);
    assert_eq!(error.failure(), &failure);
}

fn source(bytes: Vec<u8>, deadline: Instant) -> NodeCapabilityMediaSourceLease {
    NodeCapabilityMediaSourceLease::try_new(
        bytes.len() as u64,
        NodeCapabilityMediaContentDigest::from_bytes(Sha256::digest(&bytes).into()),
        deadline,
        Box::pin(Cursor::new(bytes)),
    )
    .unwrap()
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
        NodeCapabilityContractId::new("image.generate_from_text").unwrap(),
        NodeCapabilityContractVersion::new(1, 0).unwrap(),
    )
}
fn output_key() -> NodeCapabilityOutputKey {
    NodeCapabilityOutputKey::new("image").unwrap()
}
