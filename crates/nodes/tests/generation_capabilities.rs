use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use engine::node_capability::*;
use engine::workflow_graph::{WorkflowId, WorkflowNodeId, WorkflowRevision};
use nodes::*;
use projects::project::domain::ProjectId;
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Clone)]
struct AvailabilityFakeImpl {
    state: GenerationProfileAvailabilityState,
}

#[async_trait]
impl GenerationProfileAvailabilityReaderInterface for AvailabilityFakeImpl {
    async fn read_generation_profile_availability(
        &self,
        request: GenerationProfileAvailabilityRequest,
    ) -> Result<Vec<GenerationProfileAvailabilityObservation>, GenerationProfileError> {
        request
            .profile_refs()
            .iter()
            .cloned()
            .map(|profile_ref| {
                GenerationProfileAvailabilityObservation::try_new(
                    profile_ref,
                    self.state.clone(),
                    100,
                    1_000,
                )
            })
            .collect()
    }
}

#[test]
fn c4_capabilities_publish_exact_contracts_and_normalize_only_frozen_defaults() {
    let catalog = catalog();
    let available = availability(GenerationProfileAvailabilityState::Available);
    let image = TextToImageCapabilityImpl::try_new(
        catalog.clone(),
        available.clone(),
        TextToImageProviderFakeImpl::try_new().unwrap(),
        NodeCapabilityProducedMediaWriterFakeImpl::default(),
    )
    .unwrap();
    assert_contract(&image, "image.generate_from_text", "prompt", "image");
    let image_parameters = normalize_profile_only(&image, image_profile());
    assert_eq!(
        image_parameters.get(&NodeCapabilityParameterKey::new("aspect_ratio").unwrap()),
        Some(&NodeCapabilityParameterValue::Choice(
            NodeCapabilityChoiceKey::new("square").unwrap()
        ))
    );

    let video = ImageToVideoCapabilityImpl::try_new(
        catalog.clone(),
        available.clone(),
        NodeCapabilityManagedMediaReaderFakeImpl::default(),
        ImageToVideoProviderFakeImpl::try_new().unwrap(),
        NodeCapabilityProducedMediaWriterFakeImpl::default(),
    )
    .unwrap();
    assert_contract(&video, "video.generate_from_image", "image", "video");
    let video_parameters = normalize_profile_only(&video, video_profile());
    assert_eq!(
        video_parameters.get(&NodeCapabilityParameterKey::new("duration_seconds").unwrap()),
        Some(&NodeCapabilityParameterValue::UnsignedInteger(5))
    );

    let speech = TextToSpeechCapabilityImpl::try_new(
        catalog,
        available,
        TextToSpeechProviderFakeImpl::try_new().unwrap(),
        NodeCapabilityProducedMediaWriterFakeImpl::default(),
    )
    .unwrap();
    assert_contract(&speech, "audio.synthesize_speech_from_text", "text", "audio");
}

#[tokio::test]
async fn generation_profile_readiness_maps_available_unavailable_indeterminate_and_incompatible() {
    let available = text_to_image_with_state(GenerationProfileAvailabilityState::Available);
    assert!(readiness(&available, image_profile()).await.is_empty());

    let unavailable = text_to_image_with_state(GenerationProfileAvailabilityState::Unavailable {
        reason: GenerationProfileUnavailableReason::ProviderUnavailable,
        retry_after: None,
    });
    assert_eq!(
        readiness(&unavailable, image_profile()).await[0].category(),
        NodeCapabilityReadinessCategory::GenerationProfileUnavailable
    );

    let indeterminate =
        text_to_image_with_state(GenerationProfileAvailabilityState::Indeterminate {
            reason: GenerationProfileAvailabilityIndeterminateReason::NetworkOffline,
        });
    assert_eq!(
        readiness(&indeterminate, image_profile()).await[0].category(),
        NodeCapabilityReadinessCategory::GenerationProfileAvailabilityIndeterminate
    );
    assert_eq!(
        readiness(&available, video_profile()).await[0].category(),
        NodeCapabilityReadinessCategory::GenerationProfileIncompatible
    );
}

#[tokio::test]
async fn c4_capabilities_publish_one_typed_available_managed_output() {
    let image_capability = text_to_image_with_state(GenerationProfileAvailabilityState::Available);
    let image_parameters = normalize_profile_only(&image_capability, image_profile());
    let image_inputs = text_inputs(&image_capability, "prompt", "draw a moon", 1);
    let image_outputs = image_capability
        .execute_node_capability(execution_request(
            &image_capability,
            image_parameters,
            image_inputs,
            1,
        ))
        .await
        .unwrap()
        .into_completed_outputs()
        .unwrap();
    assert!(matches!(
        image_outputs.get(&NodeCapabilityOutputKey::new("image").unwrap()),
        Some(WorkflowRuntimeValue::Image(_))
    ));

    let speech_capability = TextToSpeechCapabilityImpl::try_new(
        catalog(),
        availability(GenerationProfileAvailabilityState::Available),
        TextToSpeechProviderFakeImpl::try_new().unwrap(),
        NodeCapabilityProducedMediaWriterFakeImpl::default(),
    )
    .unwrap();
    let speech_parameters = normalize_profile_only(&speech_capability, speech_profile());
    let speech_inputs = text_inputs(&speech_capability, "text", "hello", 2);
    let speech_outputs = speech_capability
        .execute_node_capability(execution_request(
            &speech_capability,
            speech_parameters,
            speech_inputs,
            2,
        ))
        .await
        .unwrap()
        .into_completed_outputs()
        .unwrap();
    assert!(matches!(
        speech_outputs.get(&NodeCapabilityOutputKey::new("audio").unwrap()),
        Some(WorkflowRuntimeValue::Audio(_))
    ));

    assert_image_to_video_output().await;
}

async fn assert_image_to_video_output() {
    let reader = NodeCapabilityManagedMediaReaderFakeImpl::default();
    let bytes = vec![4; 12];
    let reference = image_reference(3, digest(&bytes));
    reader
        .register_managed_media(
            project_id(3),
            NodeCapabilityManagedMediaReference::Image(reference),
            NodeCapabilityMediaMimeType::ImagePng,
            NodeCapabilityDeclaredMediaFacts::try_image(32, 32).unwrap(),
            bytes,
        )
        .unwrap();
    let capability = ImageToVideoCapabilityImpl::try_new(
        catalog(),
        availability(GenerationProfileAvailabilityState::Available),
        reader,
        ImageToVideoProviderFakeImpl::try_new().unwrap(),
        NodeCapabilityProducedMediaWriterFakeImpl::default(),
    )
    .unwrap();
    let parameters = normalize_profile_only(&capability, video_profile());
    let inputs = image_and_prompt_inputs(&capability, reference, 3);
    let outputs = capability
        .execute_node_capability(execution_request(&capability, parameters, inputs, 3))
        .await
        .unwrap()
        .into_completed_outputs()
        .unwrap();
    assert!(matches!(
        outputs.get(&NodeCapabilityOutputKey::new("video").unwrap()),
        Some(WorkflowRuntimeValue::Video(_))
    ));
}

fn text_to_image_with_state(
    state: GenerationProfileAvailabilityState,
) -> TextToImageCapabilityImpl<
    AvailabilityFakeImpl,
    TextToImageProviderFakeImpl,
    NodeCapabilityProducedMediaWriterFakeImpl,
> {
    TextToImageCapabilityImpl::try_new(
        catalog(),
        availability(state),
        TextToImageProviderFakeImpl::try_new().unwrap(),
        NodeCapabilityProducedMediaWriterFakeImpl::default(),
    )
    .unwrap()
}

async fn readiness(
    capability: &impl WorkflowNodeCapabilityInterface,
    profile_ref: GenerationProfileRef,
) -> Vec<NodeCapabilityReadinessIssue> {
    capability
        .check_node_external_readiness(NodeCapabilityReadinessRequest {
            project_id: project_id(1),
            normalized_parameters: normalize_profile_only(capability, profile_ref),
            deadline: NodeCapabilityReadinessDeadline::at(future_instant()),
        })
        .await
}

fn normalize_profile_only(
    capability: &impl WorkflowNodeCapabilityInterface,
    profile_ref: GenerationProfileRef,
) -> NodeCapabilityNormalizedParameters {
    capability
        .normalize_node_parameters(
            &NodeCapabilityParameterSet::try_from_map(BTreeMap::from([(
                NodeCapabilityParameterKey::new("generation_profile_ref").unwrap(),
                NodeCapabilityParameterValue::GenerationProfile(
                    profile_ref.to_node_capability_parameter_value().unwrap(),
                ),
            )]))
            .unwrap(),
        )
        .unwrap()
}

fn text_inputs(
    capability: &impl WorkflowNodeCapabilityInterface,
    key: &str,
    value: &str,
    seed: u8,
) -> WorkflowNodeInputSet {
    WorkflowNodeInputSet::try_new(
        capability.node_capability_contract(),
        BTreeMap::from([(
            NodeCapabilityInputKey::new(key).unwrap(),
            WorkflowNodeInputValue::Single(text_item(value, seed)),
        )]),
    )
    .unwrap()
}

fn image_and_prompt_inputs(
    capability: &impl WorkflowNodeCapabilityInterface,
    image: WorkflowManagedImageRef,
    seed: u8,
) -> WorkflowNodeInputSet {
    WorkflowNodeInputSet::try_new(
        capability.node_capability_contract(),
        BTreeMap::from([
            (
                NodeCapabilityInputKey::new("image").unwrap(),
                WorkflowNodeInputValue::Single(WorkflowRuntimeInputItem {
                    input_item_id: input_item_id(seed),
                    input_role_key: None,
                    value: WorkflowRuntimeValue::Image(image),
                }),
            ),
            (
                NodeCapabilityInputKey::new("prompt").unwrap(),
                WorkflowNodeInputValue::Single(text_item("slow camera", seed + 1)),
            ),
        ]),
    )
    .unwrap()
}

fn execution_request(
    capability: &impl WorkflowNodeCapabilityInterface,
    normalized_parameters: NodeCapabilityNormalizedParameters,
    inputs: WorkflowNodeInputSet,
    seed: u8,
) -> NodeCapabilityExecutionRequest {
    NodeCapabilityExecutionRequest {
        context: WorkflowNodeExecutionContext {
            project_id: project_id(seed),
            workflow_run_id: WorkflowRunId::from_uuid(uuid(seed + 20)).unwrap(),
            node_execution_id: WorkflowNodeExecutionId::from_uuid(uuid(seed + 40)).unwrap(),
            deadline: NodeCapabilityExecutionDeadline::at(future_instant()),
            cancellation: NodeCapabilityExecutionCancellation::active(),
        },
        origin: WorkflowNodeExecutionOrigin::new(
            WorkflowId::from_uuid(uuid(seed + 60)).unwrap(),
            WorkflowRevision::new(u64::from(seed) + 1).unwrap(),
            WorkflowNodeId::from_uuid(uuid(seed + 80)).unwrap(),
            capability.node_capability_contract().contract_ref().clone(),
        ),
        normalized_parameters,
        inputs,
    }
}

fn assert_contract(
    capability: &impl WorkflowNodeCapabilityInterface,
    id: &str,
    input: &str,
    output: &str,
) {
    let contract = capability.node_capability_contract();
    assert_eq!(contract.contract_ref().id().as_str(), id);
    assert!(contract.inputs().iter().any(|value| value.key().as_str() == input));
    assert_eq!(contract.outputs()[0].key().as_str(), output);
    assert_eq!(contract.parameters()[0].key().as_str(), "generation_profile_ref");
}

fn text_item(value: &str, seed: u8) -> WorkflowRuntimeInputItem {
    WorkflowRuntimeInputItem {
        input_item_id: input_item_id(seed),
        input_role_key: None,
        value: WorkflowRuntimeValue::Text(
            WorkflowTextValue::try_new([WorkflowTextPart::Literal(value.into())]).unwrap(),
        ),
    }
}

fn input_item_id(seed: u8) -> WorkflowInputItemId {
    WorkflowInputItemId::from_uuid(uuid(seed + 100)).unwrap()
}

fn catalog() -> Arc<GenerationProfileCatalog> {
    Arc::new(GenerationProfileCatalog::frozen_mvp().unwrap())
}

fn availability(state: GenerationProfileAvailabilityState) -> AvailabilityFakeImpl {
    AvailabilityFakeImpl { state }
}

fn image_profile() -> GenerationProfileRef {
    profile_for("image.generate_from_text")
}

fn video_profile() -> GenerationProfileRef {
    profile_for("video.generate_from_image")
}

fn speech_profile() -> GenerationProfileRef {
    profile_for("audio.synthesize_speech_from_text")
}

fn profile_for(capability_id: &str) -> GenerationProfileRef {
    catalog().list_active_generation_profiles_for_capability(&capability_ref(capability_id))[0]
        .profile_ref()
        .clone()
}

fn capability_ref(id: &str) -> NodeCapabilityContractRef {
    NodeCapabilityContractRef::new(
        NodeCapabilityContractId::new(id).unwrap(),
        NodeCapabilityContractVersion::new(1, 0).unwrap(),
    )
}

fn image_reference(seed: u8, digest: NodeCapabilityMediaContentDigest) -> WorkflowManagedImageRef {
    WorkflowManagedImageRef::new(
        WorkflowManagedAssetIdBoundaryValue::from_bytes(uuid(seed).into_bytes()).unwrap(),
        WorkflowManagedContentFingerprint::from_bytes(digest.as_bytes()),
    )
}

fn digest(bytes: &[u8]) -> NodeCapabilityMediaContentDigest {
    NodeCapabilityMediaContentDigest::from_bytes(Sha256::digest(bytes).into())
}

fn project_id(seed: u8) -> ProjectId {
    ProjectId::from_uuid(uuid(seed)).unwrap()
}

fn future_instant() -> Instant {
    Instant::now() + Duration::from_secs(4)
}

fn uuid(seed: u8) -> Uuid {
    let mut bytes = [seed; 16];
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}
