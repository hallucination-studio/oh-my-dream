use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use engine::node_capability::*;
use engine::workflow_graph::{WorkflowId, WorkflowNodeId, WorkflowRevision};
use nodes::{
    NodeCapabilityDeclaredMediaFacts, NodeCapabilityManagedMediaReaderFakeImpl,
    NodeCapabilityManagedMediaReference, NodeCapabilityMediaContentDigest,
    NodeCapabilityMediaMimeType, ProvideLiteralTextCapabilityImpl, ReadAudioAssetCapabilityImpl,
    ReadImageAssetCapabilityImpl, ReadVideoAssetCapabilityImpl,
};
use projects::project::domain::ProjectId;
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[tokio::test]
async fn literal_capability_normalizes_and_returns_exact_structured_text() {
    let capability = ProvideLiteralTextCapabilityImpl::try_new().unwrap();
    let normalized = normalize(&capability, NodeCapabilityParameterValue::Text("hello".into()));
    let outputs = capability
        .execute_node_capability(execution_request(&capability, normalized, 10))
        .await
        .unwrap()
        .into_completed_outputs()
        .unwrap();
    let value = outputs.get(&NodeCapabilityOutputKey::new("text").unwrap()).unwrap();
    let WorkflowRuntimeValue::Text(value) = value else { panic!("expected Text output") };
    assert_eq!(value.parts(), [WorkflowTextPart::Literal("hello".into())]);
}
#[tokio::test]
async fn image_asset_capability_checks_readiness_and_returns_the_resolved_reference() {
    let reader = NodeCapabilityManagedMediaReaderFakeImpl::default();
    let bytes = vec![5; 16];
    let reference = image_reference(20, digest(&bytes));
    reader
        .register_managed_media(
            project_id(20),
            NodeCapabilityManagedMediaReference::Image(reference),
            NodeCapabilityMediaMimeType::ImagePng,
            NodeCapabilityDeclaredMediaFacts::try_image(32, 32).unwrap(),
            bytes,
        )
        .unwrap();
    let capability = ReadImageAssetCapabilityImpl::try_new(reader).unwrap();
    let normalized = normalize(
        &capability,
        NodeCapabilityParameterValue::ManagedAsset(
            NodeCapabilityManagedAssetIdParameterValue::new(reference.asset_id()),
        ),
    );
    let readiness = capability
        .check_node_external_readiness(NodeCapabilityReadinessRequest {
            project_id: project_id(20),
            normalized_parameters: normalized.clone(),
            deadline: NodeCapabilityReadinessDeadline::at(future_instant()),
        })
        .await;
    assert!(readiness.is_empty());

    let outputs = capability
        .execute_node_capability(execution_request(&capability, normalized, 20))
        .await
        .unwrap()
        .into_completed_outputs()
        .unwrap();
    assert_eq!(
        outputs.get(&NodeCapabilityOutputKey::new("image").unwrap()),
        Some(&WorkflowRuntimeValue::Image(reference))
    );
}
#[tokio::test]
async fn asset_readiness_reports_unavailable_without_execution_or_fallback() {
    let capability =
        ReadVideoAssetCapabilityImpl::try_new(NodeCapabilityManagedMediaReaderFakeImpl::default())
            .unwrap();
    let asset_id = asset_id(30);
    let normalized = normalize(
        &capability,
        NodeCapabilityParameterValue::ManagedAsset(
            NodeCapabilityManagedAssetIdParameterValue::new(asset_id),
        ),
    );
    let issues = capability
        .check_node_external_readiness(NodeCapabilityReadinessRequest {
            project_id: project_id(30),
            normalized_parameters: normalized.clone(),
            deadline: NodeCapabilityReadinessDeadline::at(future_instant()),
        })
        .await;
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].category(), NodeCapabilityReadinessCategory::ManagedAssetUnavailable);
    let result =
        capability.execute_node_capability(execution_request(&capability, normalized, 30)).await;
    let Err(error) = result else { panic!("unavailable Asset unexpectedly executed") };
    assert_eq!(
        error.failure(),
        &NodeCapabilityExecutionFailure::Media(NodeCapabilityMediaFailure::Unavailable)
    );
    assert_eq!(
        error.target(),
        &NodeCapabilityExecutionTarget::Parameter(
            NodeCapabilityParameterKey::new("asset_id").unwrap()
        )
    );
}
#[tokio::test]
async fn asset_read_deadline_targets_the_asset_id_parameter() {
    let capability =
        ReadImageAssetCapabilityImpl::try_new(NodeCapabilityManagedMediaReaderFakeImpl::default())
            .unwrap();
    let normalized = normalize(
        &capability,
        NodeCapabilityParameterValue::ManagedAsset(
            NodeCapabilityManagedAssetIdParameterValue::new(asset_id(33)),
        ),
    );
    let mut expired = execution_request(&capability, normalized, 33);
    expired.context.deadline = NodeCapabilityExecutionDeadline::at(Instant::now());
    let result = capability.execute_node_capability(expired).await;
    let Err(error) = result else { panic!("expired Asset read unexpectedly executed") };
    assert_eq!(error.failure(), &NodeCapabilityExecutionFailure::DeadlineExceeded);
    assert_eq!(
        error.target(),
        &NodeCapabilityExecutionTarget::Parameter(
            NodeCapabilityParameterKey::new("asset_id").unwrap()
        )
    );
}
#[tokio::test]
async fn asset_read_capability_rejects_invalid_invocation_before_reader_access() {
    let literal = ProvideLiteralTextCapabilityImpl::try_new().unwrap();
    let wrong_parameters = normalize(&literal, NodeCapabilityParameterValue::Text("wrong".into()));
    let capability =
        ReadAudioAssetCapabilityImpl::try_new(NodeCapabilityManagedMediaReaderFakeImpl::default())
            .unwrap();
    let issues = capability
        .check_node_external_readiness(NodeCapabilityReadinessRequest {
            project_id: project_id(31),
            normalized_parameters: wrong_parameters.clone(),
            deadline: NodeCapabilityReadinessDeadline::at(future_instant()),
        })
        .await;
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].category(), NodeCapabilityReadinessCategory::InvalidCapabilityInvocation);
    assert_eq!(issues[0].target(), &NodeCapabilityReadinessTarget::Capability);

    let result = capability
        .execute_node_capability(execution_request(&capability, wrong_parameters, 31))
        .await;
    let Err(error) = result else { panic!("invalid direct invocation unexpectedly executed") };
    assert_eq!(error.failure(), &NodeCapabilityExecutionFailure::InvalidCapabilityInvocation);
    assert_eq!(error.target(), &NodeCapabilityExecutionTarget::Capability);
}

#[tokio::test]
async fn c3_capabilities_reject_an_origin_for_another_capability() {
    let capability = ProvideLiteralTextCapabilityImpl::try_new().unwrap();
    let normalized = normalize(&capability, NodeCapabilityParameterValue::Text("hello".into()));
    let mut request = execution_request(&capability, normalized, 31);
    request.origin = WorkflowNodeExecutionOrigin::new(
        request.origin.workflow_id(),
        request.origin.workflow_revision(),
        request.origin.workflow_node_id(),
        NodeCapabilityContractRef::new(
            NodeCapabilityContractId::new("image.read_asset").unwrap(),
            NodeCapabilityContractVersion::new(1, 0).unwrap(),
        ),
    );

    let result = capability.execute_node_capability(request).await;
    let Err(error) = result else { panic!("mismatched origin unexpectedly executed") };
    assert_eq!(error.failure(), &NodeCapabilityExecutionFailure::InvalidCapabilityInvocation);
    assert_eq!(error.stage(), NodeCapabilityExecutionStage::ResolveInputs);
    assert_eq!(error.target(), &NodeCapabilityExecutionTarget::Capability);
}
#[tokio::test]
async fn asset_readiness_preserves_kind_mismatch_and_deadline_indeterminate() {
    let reader = NodeCapabilityManagedMediaReaderFakeImpl::default();
    let bytes = vec![6; 16];
    let reference = image_reference(32, digest(&bytes));
    reader
        .register_managed_media(
            project_id(32),
            NodeCapabilityManagedMediaReference::Image(reference),
            NodeCapabilityMediaMimeType::ImagePng,
            NodeCapabilityDeclaredMediaFacts::try_image(32, 32).unwrap(),
            bytes,
        )
        .unwrap();
    let capability = ReadVideoAssetCapabilityImpl::try_new(reader).unwrap();
    let normalized = normalize(
        &capability,
        NodeCapabilityParameterValue::ManagedAsset(
            NodeCapabilityManagedAssetIdParameterValue::new(reference.asset_id()),
        ),
    );
    let mismatch = capability
        .check_node_external_readiness(NodeCapabilityReadinessRequest {
            project_id: project_id(32),
            normalized_parameters: normalized.clone(),
            deadline: NodeCapabilityReadinessDeadline::at(future_instant()),
        })
        .await;
    assert_eq!(mismatch[0].category(), NodeCapabilityReadinessCategory::ManagedAssetKindMismatch);
    assert_eq!(
        mismatch[0].media_kind_mismatch(),
        Some((WorkflowDataType::Video, WorkflowDataType::Image))
    );
    let indeterminate = capability
        .check_node_external_readiness(NodeCapabilityReadinessRequest {
            project_id: project_id(32),
            normalized_parameters: normalized,
            deadline: NodeCapabilityReadinessDeadline::at(Instant::now()),
        })
        .await;
    assert_eq!(
        indeterminate[0].category(),
        NodeCapabilityReadinessCategory::ManagedAssetReadinessIndeterminate
    );
}
#[tokio::test]
async fn asset_read_cancellation_targets_the_asset_id_parameter() {
    let capability =
        ReadImageAssetCapabilityImpl::try_new(NodeCapabilityManagedMediaReaderFakeImpl::default())
            .unwrap();
    let normalized = normalize(
        &capability,
        NodeCapabilityParameterValue::ManagedAsset(
            NodeCapabilityManagedAssetIdParameterValue::new(asset_id(33)),
        ),
    );
    let request = execution_request(&capability, normalized, 33);
    request.context.cancellation.cancel();
    let result = capability.execute_node_capability(request).await;
    let Err(error) = result else { panic!("cancelled Asset read unexpectedly executed") };
    assert_eq!(error.failure(), &NodeCapabilityExecutionFailure::Cancelled);
    assert_eq!(
        error.target(),
        &NodeCapabilityExecutionTarget::Parameter(
            NodeCapabilityParameterKey::new("asset_id").unwrap()
        )
    );
}
#[tokio::test]
async fn video_asset_capability_returns_matching_typed_reference() {
    let video_reader = NodeCapabilityManagedMediaReaderFakeImpl::default();
    let video_bytes = vec![7; 20];
    let video = WorkflowManagedVideoRef::new(
        asset_id(34),
        WorkflowManagedContentFingerprint::from_bytes(digest(&video_bytes).as_bytes()),
    );
    video_reader
        .register_managed_media(
            project_id(34),
            NodeCapabilityManagedMediaReference::Video(video),
            NodeCapabilityMediaMimeType::VideoMp4,
            NodeCapabilityDeclaredMediaFacts::try_video(32, 32, 5_000, false).unwrap(),
            video_bytes,
        )
        .unwrap();
    let video_capability = ReadVideoAssetCapabilityImpl::try_new(video_reader).unwrap();
    let video_parameters = normalize(
        &video_capability,
        NodeCapabilityParameterValue::ManagedAsset(
            NodeCapabilityManagedAssetIdParameterValue::new(video.asset_id()),
        ),
    );
    let video_outputs = video_capability
        .execute_node_capability(execution_request(&video_capability, video_parameters, 34))
        .await
        .unwrap()
        .into_completed_outputs()
        .unwrap();
    assert_eq!(
        video_outputs.get(&NodeCapabilityOutputKey::new("video").unwrap()),
        Some(&WorkflowRuntimeValue::Video(video))
    );
}
#[tokio::test]
async fn audio_asset_capability_returns_matching_typed_reference() {
    let audio_reader = NodeCapabilityManagedMediaReaderFakeImpl::default();
    let audio_bytes = vec![8; 12];
    let audio = WorkflowManagedAudioRef::new(
        asset_id(35),
        WorkflowManagedContentFingerprint::from_bytes(digest(&audio_bytes).as_bytes()),
    );
    audio_reader
        .register_managed_media(
            project_id(35),
            NodeCapabilityManagedMediaReference::Audio(audio),
            NodeCapabilityMediaMimeType::AudioMpeg,
            NodeCapabilityDeclaredMediaFacts::try_audio(1_000, 44_100, 1).unwrap(),
            audio_bytes,
        )
        .unwrap();
    let audio_capability = ReadAudioAssetCapabilityImpl::try_new(audio_reader).unwrap();
    let audio_parameters = normalize(
        &audio_capability,
        NodeCapabilityParameterValue::ManagedAsset(
            NodeCapabilityManagedAssetIdParameterValue::new(audio.asset_id()),
        ),
    );
    let audio_outputs = audio_capability
        .execute_node_capability(execution_request(&audio_capability, audio_parameters, 35))
        .await
        .unwrap()
        .into_completed_outputs()
        .unwrap();
    assert_eq!(
        audio_outputs.get(&NodeCapabilityOutputKey::new("audio").unwrap()),
        Some(&WorkflowRuntimeValue::Audio(audio))
    );
}

fn normalize(
    capability: &impl WorkflowNodeCapabilityInterface,
    value: NodeCapabilityParameterValue,
) -> NodeCapabilityNormalizedParameters {
    let key =
        if matches!(value, NodeCapabilityParameterValue::Text(_)) { "text" } else { "asset_id" };
    let parameters = NodeCapabilityParameterSet::try_from_map(BTreeMap::from([(
        NodeCapabilityParameterKey::new(key).unwrap(),
        value,
    )]))
    .unwrap();
    capability.normalize_node_parameters(&parameters).unwrap()
}

fn execution_request(
    capability: &impl WorkflowNodeCapabilityInterface,
    normalized_parameters: NodeCapabilityNormalizedParameters,
    seed: u8,
) -> NodeCapabilityExecutionRequest {
    NodeCapabilityExecutionRequest {
        context: WorkflowNodeExecutionContext {
            project_id: project_id(seed),
            workflow_run_id: WorkflowRunId::from_uuid(uuid(seed.wrapping_add(40))).unwrap(),
            node_execution_id: WorkflowNodeExecutionId::from_uuid(uuid(seed.wrapping_add(80)))
                .unwrap(),
            deadline: NodeCapabilityExecutionDeadline::at(future_instant()),
            cancellation: NodeCapabilityExecutionCancellation::active(),
        },
        origin: WorkflowNodeExecutionOrigin::new(
            WorkflowId::from_uuid(uuid(seed.wrapping_add(100))).unwrap(),
            WorkflowRevision::new(u64::from(seed) + 1).unwrap(),
            WorkflowNodeId::from_uuid(uuid(seed.wrapping_add(120))).unwrap(),
            capability.node_capability_contract().contract_ref().clone(),
        ),
        normalized_parameters,
        inputs: WorkflowNodeInputSet::try_new(
            capability.node_capability_contract(),
            BTreeMap::new(),
        )
        .unwrap(),
    }
}

fn image_reference(seed: u8, digest: NodeCapabilityMediaContentDigest) -> WorkflowManagedImageRef {
    WorkflowManagedImageRef::new(
        asset_id(seed),
        WorkflowManagedContentFingerprint::from_bytes(digest.as_bytes()),
    )
}

fn digest(bytes: &[u8]) -> NodeCapabilityMediaContentDigest {
    NodeCapabilityMediaContentDigest::from_bytes(Sha256::digest(bytes).into())
}

fn project_id(seed: u8) -> ProjectId {
    ProjectId::from_uuid(uuid(seed)).unwrap()
}
fn asset_id(seed: u8) -> WorkflowManagedAssetIdBoundaryValue {
    WorkflowManagedAssetIdBoundaryValue::from_bytes(uuid(seed).into_bytes()).unwrap()
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
