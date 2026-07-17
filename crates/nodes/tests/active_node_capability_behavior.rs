use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use engine::node_capability::*;
use engine::workflow_graph::{WorkflowId, WorkflowNodeId, WorkflowRevision};
use nodes::*;
use projects::project::domain::ProjectId;
use sha2::{Digest, Sha256};
use uuid::Uuid;

mod c5_support;
use c5_support::GenerationProfileAlwaysAvailableFakeImpl;
#[path = "c5_support/active_capability_cases.rs"]
mod active_capability_cases;
use active_capability_cases::configured_asset_capability;

struct ActiveCapabilityBehaviorCase {
    capability: Arc<dyn WorkflowNodeCapabilityInterface>,
    parameters: NodeCapabilityParameterSet,
    inputs: WorkflowNodeInputSet,
    expected_output_key: NodeCapabilityOutputKey,
    expected_output_type: WorkflowDataType,
    waits_for_generation_task: bool,
}

#[tokio::test]
async fn all_seven_active_implementations_pass_the_same_execution_behavior_suite() {
    let cases = active_behavior_cases();
    let normalized = cases
        .iter()
        .map(|case| case.capability.normalize_node_parameters(&case.parameters).unwrap())
        .collect::<Vec<_>>();
    for (index, case) in cases.iter().enumerate() {
        let foreign_index = if index < 4 { 4 } else { 0 };
        assert_active_capability_behavior(
            case,
            normalized[index].clone(),
            normalized[foreign_index].clone(),
            cases[foreign_index].inputs.clone(),
            index as u8 + 1,
        )
        .await;
    }
}

async fn assert_active_capability_behavior(
    case: &ActiveCapabilityBehaviorCase,
    normalized: NodeCapabilityNormalizedParameters,
    foreign_normalized: NodeCapabilityNormalizedParameters,
    foreign_inputs: WorkflowNodeInputSet,
    seed: u8,
) {
    let request =
        execution_request(case.capability.as_ref(), normalized.clone(), case.inputs.clone(), seed);
    let outcome = case.capability.execute_node_capability(request).await.unwrap();
    if case.waits_for_generation_task {
        assert_eq!(outcome, WorkflowNodeCapabilityExecutionOutcome::WaitingForGenerationTask);
    } else {
        let output = outcome.into_completed_outputs().unwrap();
        let value = output.get(&case.expected_output_key).unwrap().clone();
        assert_eq!(value.data_type(), case.expected_output_type);
        let expected = WorkflowNodeOutputSet::try_new(
            case.capability.node_capability_contract(),
            BTreeMap::from([(case.expected_output_key.clone(), value)]),
        )
        .unwrap();
        assert_eq!(output, expected);
    }

    let mut invalid = execution_request(
        case.capability.as_ref(),
        normalized.clone(),
        case.inputs.clone(),
        seed.wrapping_add(30),
    );
    invalid.origin = WorkflowNodeExecutionOrigin::new(
        invalid.origin.workflow_id(),
        invalid.origin.workflow_revision(),
        invalid.origin.workflow_node_id(),
        contract_ref("video.generate_from_text"),
    );
    assert_invalid_invocation(case.capability.execute_node_capability(invalid).await);
    let invalid_parameters = execution_request(
        case.capability.as_ref(),
        foreign_normalized,
        case.inputs.clone(),
        seed.wrapping_add(31),
    );
    assert_invalid_invocation(case.capability.execute_node_capability(invalid_parameters).await);
    let invalid_inputs = execution_request(
        case.capability.as_ref(),
        normalized,
        foreign_inputs,
        seed.wrapping_add(32),
    );
    assert_invalid_invocation(case.capability.execute_node_capability(invalid_inputs).await);
}

fn assert_invalid_invocation(
    result: Result<WorkflowNodeCapabilityExecutionOutcome, NodeCapabilityExecutionError>,
) {
    let error = result.unwrap_err();
    assert_eq!(error.stage(), NodeCapabilityExecutionStage::ResolveInputs);
    assert_eq!(error.target(), &NodeCapabilityExecutionTarget::Capability);
    assert_eq!(error.failure(), &NodeCapabilityExecutionFailure::InvalidCapabilityInvocation);
}

fn active_behavior_cases() -> Vec<ActiveCapabilityBehaviorCase> {
    let mut cases = vec![literal_case()];
    cases.extend([
        asset_case(WorkflowDataType::Image, "image.read_asset", "image", 2),
        asset_case(WorkflowDataType::Video, "video.read_asset", "video", 3),
        asset_case(WorkflowDataType::Audio, "audio.read_asset", "audio", 4),
    ]);
    cases.extend([text_to_image_case(), image_to_video_case(), text_to_speech_case()]);
    cases
}

fn literal_case() -> ActiveCapabilityBehaviorCase {
    let capability = Arc::new(ProvideLiteralTextCapabilityImpl::try_new().unwrap());
    ActiveCapabilityBehaviorCase {
        inputs: WorkflowNodeInputSet::try_new(
            capability.node_capability_contract(),
            BTreeMap::new(),
        )
        .unwrap(),
        parameters: parameter_set("text", NodeCapabilityParameterValue::Text("hello".into())),
        capability,
        expected_output_key: output_key("text"),
        expected_output_type: WorkflowDataType::Text,
        waits_for_generation_task: false,
    }
}

fn asset_case(
    kind: WorkflowDataType,
    contract_id: &str,
    output: &str,
    seed: u8,
) -> ActiveCapabilityBehaviorCase {
    let bytes = vec![seed; 16];
    let fingerprint = WorkflowManagedContentFingerprint::from_bytes(Sha256::digest(&bytes).into());
    let asset_id =
        WorkflowManagedAssetIdBoundaryValue::from_bytes(uuid(seed).into_bytes()).unwrap();
    let capability =
        configured_asset_capability(kind, project_id(seed), asset_id, fingerprint, bytes);
    assert_eq!(capability.node_capability_contract().contract_ref(), &contract_ref(contract_id));
    ActiveCapabilityBehaviorCase {
        inputs: WorkflowNodeInputSet::try_new(
            capability.node_capability_contract(),
            BTreeMap::new(),
        )
        .unwrap(),
        parameters: parameter_set(
            "asset_id",
            NodeCapabilityParameterValue::ManagedAsset(
                NodeCapabilityManagedAssetIdParameterValue::new(asset_id),
            ),
        ),
        capability,
        expected_output_key: output_key(output),
        expected_output_type: kind,
        waits_for_generation_task: false,
    }
}

fn text_to_image_case() -> ActiveCapabilityBehaviorCase {
    let capability = Arc::new(
        TextToImageCapabilityImpl::try_new(
            catalog(),
            GenerationProfileAlwaysAvailableFakeImpl,
            NodeCapabilityGenerationTaskStarterFakeImpl::default(),
        )
        .unwrap(),
    );
    generation_text_case(
        capability,
        profile_for("image.generate_from_text"),
        "prompt",
        "image",
        WorkflowDataType::Image,
    )
}

fn text_to_speech_case() -> ActiveCapabilityBehaviorCase {
    let capability = Arc::new(
        TextToSpeechCapabilityImpl::try_new(
            catalog(),
            GenerationProfileAlwaysAvailableFakeImpl,
            NodeCapabilityGenerationTaskStarterFakeImpl::default(),
        )
        .unwrap(),
    );
    generation_text_case(
        capability,
        profile_for("audio.synthesize_speech_from_text"),
        "text",
        "audio",
        WorkflowDataType::Audio,
    )
}

fn generation_text_case(
    capability: Arc<dyn WorkflowNodeCapabilityInterface>,
    profile_ref: GenerationProfileRef,
    input: &str,
    output: &str,
    output_type: WorkflowDataType,
) -> ActiveCapabilityBehaviorCase {
    ActiveCapabilityBehaviorCase {
        inputs: text_inputs(capability.as_ref(), input, "semantic text", 21),
        parameters: profile_parameters(profile_ref),
        capability,
        expected_output_key: output_key(output),
        expected_output_type: output_type,
        waits_for_generation_task: true,
    }
}

fn image_to_video_case() -> ActiveCapabilityBehaviorCase {
    let bytes = vec![22; 16];
    let reference = WorkflowManagedImageRef::new(
        WorkflowManagedAssetIdBoundaryValue::from_bytes(uuid(22).into_bytes()).unwrap(),
        WorkflowManagedContentFingerprint::from_bytes(Sha256::digest(&bytes).into()),
    );
    let reader = NodeCapabilityManagedMediaReaderFakeImpl::default();
    reader
        .register_managed_media(
            project_id(6),
            NodeCapabilityManagedMediaReference::Image(reference),
            NodeCapabilityMediaMimeType::ImagePng,
            NodeCapabilityDeclaredMediaFacts::try_image(32, 32).unwrap(),
            bytes,
        )
        .unwrap();
    let capability = Arc::new(
        ImageToVideoCapabilityImpl::try_new(
            catalog(),
            GenerationProfileAlwaysAvailableFakeImpl,
            reader,
            NodeCapabilityGenerationTaskStarterFakeImpl::default(),
        )
        .unwrap(),
    );
    let inputs = WorkflowNodeInputSet::try_new(
        capability.node_capability_contract(),
        BTreeMap::from([
            (
                input_key("image"),
                WorkflowNodeInputValue::Single(input_item(
                    WorkflowRuntimeValue::Image(reference),
                    22,
                )),
            ),
            (
                input_key("prompt"),
                WorkflowNodeInputValue::Single(input_item(text_value("camera"), 23)),
            ),
        ]),
    )
    .unwrap();
    ActiveCapabilityBehaviorCase {
        capability,
        parameters: profile_parameters(profile_for("video.generate_from_image")),
        inputs,
        expected_output_key: output_key("video"),
        expected_output_type: WorkflowDataType::Video,
        waits_for_generation_task: true,
    }
}

fn execution_request(
    capability: &dyn WorkflowNodeCapabilityInterface,
    normalized_parameters: NodeCapabilityNormalizedParameters,
    inputs: WorkflowNodeInputSet,
    seed: u8,
) -> NodeCapabilityExecutionRequest {
    NodeCapabilityExecutionRequest {
        context: WorkflowNodeExecutionContext {
            project_id: project_id(seed),
            workflow_run_id: WorkflowRunId::from_uuid(uuid(seed.wrapping_add(30))).unwrap(),
            node_execution_id: WorkflowNodeExecutionId::from_uuid(uuid(seed.wrapping_add(60)))
                .unwrap(),
            deadline: NodeCapabilityExecutionDeadline::at(Instant::now() + Duration::from_secs(5)),
            cancellation: NodeCapabilityExecutionCancellation::active(),
        },
        origin: WorkflowNodeExecutionOrigin::new(
            WorkflowId::from_uuid(uuid(seed.wrapping_add(90))).unwrap(),
            WorkflowRevision::new(u64::from(seed) + 1).unwrap(),
            WorkflowNodeId::from_uuid(uuid(seed.wrapping_add(120))).unwrap(),
            capability.node_capability_contract().contract_ref().clone(),
        ),
        normalized_parameters,
        inputs,
    }
}

fn text_inputs(
    capability: &dyn WorkflowNodeCapabilityInterface,
    key: &str,
    value: &str,
    seed: u8,
) -> WorkflowNodeInputSet {
    WorkflowNodeInputSet::try_new(
        capability.node_capability_contract(),
        BTreeMap::from([(
            input_key(key),
            WorkflowNodeInputValue::Single(input_item(text_value(value), seed)),
        )]),
    )
    .unwrap()
}

fn input_item(value: WorkflowRuntimeValue, seed: u8) -> WorkflowRuntimeInputItem {
    WorkflowRuntimeInputItem {
        input_item_id: WorkflowInputItemId::from_uuid(uuid(seed)).unwrap(),
        input_role_key: None,
        value,
    }
}

fn text_value(value: &str) -> WorkflowRuntimeValue {
    WorkflowRuntimeValue::Text(
        WorkflowTextValue::try_new([WorkflowTextPart::Literal(value.into())]).unwrap(),
    )
}

fn profile_parameters(profile_ref: GenerationProfileRef) -> NodeCapabilityParameterSet {
    parameter_set(
        "generation_profile_ref",
        NodeCapabilityParameterValue::GenerationProfile(
            profile_ref.to_node_capability_parameter_value().unwrap(),
        ),
    )
}

fn parameter_set(key: &str, value: NodeCapabilityParameterValue) -> NodeCapabilityParameterSet {
    NodeCapabilityParameterSet::try_from_map(BTreeMap::from([(
        NodeCapabilityParameterKey::new(key).unwrap(),
        value,
    )]))
    .unwrap()
}

fn catalog() -> Arc<GenerationProfileCatalog> {
    Arc::new(GenerationProfileCatalog::frozen_mvp().unwrap())
}
fn profile_for(id: &str) -> GenerationProfileRef {
    catalog().list_active_generation_profiles_for_capability(&contract_ref(id))[0]
        .profile_ref()
        .clone()
}
fn contract_ref(id: &str) -> NodeCapabilityContractRef {
    NodeCapabilityContractRef::new(
        NodeCapabilityContractId::new(id).unwrap(),
        NodeCapabilityContractVersion::new(1, 0).unwrap(),
    )
}
fn input_key(value: &str) -> NodeCapabilityInputKey {
    NodeCapabilityInputKey::new(value).unwrap()
}
fn output_key(value: &str) -> NodeCapabilityOutputKey {
    NodeCapabilityOutputKey::new(value).unwrap()
}
fn project_id(seed: u8) -> ProjectId {
    ProjectId::from_uuid(uuid(seed)).unwrap()
}
fn uuid(seed: u8) -> Uuid {
    let mut bytes = [seed; 16];
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}
