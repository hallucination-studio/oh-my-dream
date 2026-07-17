use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use engine::node_capability::*;
use engine::workflow_graph::{WorkflowId, WorkflowNodeId, WorkflowRevision};
use nodes::*;
use projects::project::domain::ProjectId;
use uuid::Uuid;

mod c5_support;
use c5_support::GenerationProfileAlwaysAvailableFakeImpl;

#[tokio::test]
async fn text_to_image_preserves_context_origin_profile_and_exact_operation() {
    let starter = NodeCapabilityGenerationTaskStarterFakeImpl::default();
    let capability = TextToImageCapabilityImpl::try_new(
        catalog(),
        GenerationProfileAlwaysAvailableFakeImpl,
        starter.clone(),
    )
    .unwrap();
    let profile = profile_for("image.generate_from_text");
    let context = execution_context(1);
    let origin = execution_origin(1, "image.generate_from_text");
    let request = execution_request(
        &capability,
        &profile,
        context.clone(),
        origin.clone(),
        "prompt",
        "draw a moon",
        Some((
            "aspect_ratio",
            NodeCapabilityParameterValue::Choice(
                NodeCapabilityChoiceKey::new("landscape_16_9").unwrap(),
            ),
        )),
    );

    assert_eq!(
        capability.execute_node_capability(request).await.unwrap(),
        WorkflowNodeCapabilityExecutionOutcome::WaitingForGenerationTask
    );
    let requests = starter.requests();
    let started = requests.first().unwrap();
    assert_context(started.context(), &context);
    assert_eq!(started.origin(), &origin);
    assert_eq!(started.profile_ref(), &profile);
    assert_eq!(started.primary_output_key().as_str(), "image");
    assert!(started.input_assets().is_empty());
    match started.request() {
        NodeCapabilityGenerationTaskRequest::Image { prompt, aspect_ratio } => {
            assert_eq!(prompt.parts(), [WorkflowTextPart::Literal("draw a moon".into())]);
            assert_eq!(*aspect_ratio, ImageAspectRatio::LandscapeSixteenByNine);
        }
        _ => panic!("expected Image task request"),
    }
}

#[tokio::test]
async fn text_to_speech_preserves_context_origin_profile_and_exact_operation() {
    let starter = NodeCapabilityGenerationTaskStarterFakeImpl::default();
    let capability = TextToSpeechCapabilityImpl::try_new(
        catalog(),
        GenerationProfileAlwaysAvailableFakeImpl,
        starter.clone(),
    )
    .unwrap();
    let profile = profile_for("audio.synthesize_speech_from_text");
    let context = execution_context(2);
    let origin = execution_origin(2, "audio.synthesize_speech_from_text");
    let request = execution_request(
        &capability,
        &profile,
        context.clone(),
        origin.clone(),
        "text",
        "hello world",
        None,
    );

    assert_eq!(
        capability.execute_node_capability(request).await.unwrap(),
        WorkflowNodeCapabilityExecutionOutcome::WaitingForGenerationTask
    );
    let requests = starter.requests();
    let started = requests.first().unwrap();
    assert_context(started.context(), &context);
    assert_eq!(started.origin(), &origin);
    assert_eq!(started.profile_ref(), &profile);
    assert_eq!(started.primary_output_key().as_str(), "audio");
    assert!(started.input_assets().is_empty());
    match started.request() {
        NodeCapabilityGenerationTaskRequest::Voice { text } => {
            assert_eq!(text.parts(), [WorkflowTextPart::Literal("hello world".into())]);
        }
        _ => panic!("expected Voice task request"),
    }
}

fn execution_request(
    capability: &impl WorkflowNodeCapabilityInterface,
    profile: &GenerationProfileRef,
    context: WorkflowNodeExecutionContext,
    origin: WorkflowNodeExecutionOrigin,
    input_key: &str,
    text: &str,
    extra_parameter: Option<(&str, NodeCapabilityParameterValue)>,
) -> NodeCapabilityExecutionRequest {
    let mut parameters = BTreeMap::from([(
        NodeCapabilityParameterKey::new("generation_profile_ref").unwrap(),
        NodeCapabilityParameterValue::GenerationProfile(
            profile.to_node_capability_parameter_value().unwrap(),
        ),
    )]);
    if let Some((key, value)) = extra_parameter {
        parameters.insert(NodeCapabilityParameterKey::new(key).unwrap(), value);
    }
    let parameters = NodeCapabilityParameterSet::try_from_map(parameters).unwrap();
    let inputs = WorkflowNodeInputSet::try_new(
        capability.node_capability_contract(),
        BTreeMap::from([(
            NodeCapabilityInputKey::new(input_key).unwrap(),
            WorkflowNodeInputValue::Single(WorkflowRuntimeInputItem {
                input_item_id: WorkflowInputItemId::from_uuid(uuid(40)).unwrap(),
                input_role_key: None,
                value: WorkflowRuntimeValue::Text(
                    WorkflowTextValue::try_new([WorkflowTextPart::Literal(text.into())]).unwrap(),
                ),
            }),
        )]),
    )
    .unwrap();
    NodeCapabilityExecutionRequest {
        context,
        origin,
        normalized_parameters: capability.normalize_node_parameters(&parameters).unwrap(),
        inputs,
    }
}

fn assert_context(actual: &WorkflowNodeExecutionContext, expected: &WorkflowNodeExecutionContext) {
    assert_eq!(actual.project_id, expected.project_id);
    assert_eq!(actual.workflow_run_id, expected.workflow_run_id);
    assert_eq!(actual.node_execution_id, expected.node_execution_id);
    assert_eq!(actual.deadline, expected.deadline);
    assert_eq!(actual.cancellation.is_cancelled(), expected.cancellation.is_cancelled());
}

fn execution_context(seed: u8) -> WorkflowNodeExecutionContext {
    WorkflowNodeExecutionContext {
        project_id: ProjectId::from_uuid(uuid(seed)).unwrap(),
        workflow_run_id: WorkflowRunId::from_uuid(uuid(seed + 30)).unwrap(),
        node_execution_id: WorkflowNodeExecutionId::from_uuid(uuid(seed + 60)).unwrap(),
        deadline: NodeCapabilityExecutionDeadline::at(Instant::now() + Duration::from_secs(5)),
        cancellation: NodeCapabilityExecutionCancellation::active(),
    }
}

fn execution_origin(seed: u8, contract_id: &str) -> WorkflowNodeExecutionOrigin {
    WorkflowNodeExecutionOrigin::new(
        WorkflowId::from_uuid(uuid(seed + 90)).unwrap(),
        WorkflowRevision::new(u64::from(seed) + 1).unwrap(),
        WorkflowNodeId::from_uuid(uuid(seed + 120)).unwrap(),
        contract_ref(contract_id),
    )
}

fn catalog() -> Arc<GenerationProfileCatalog> {
    Arc::new(GenerationProfileCatalog::frozen_mvp().unwrap())
}

fn profile_for(contract_id: &str) -> GenerationProfileRef {
    catalog().list_active_generation_profiles_for_capability(&contract_ref(contract_id))[0]
        .profile_ref()
        .clone()
}

fn contract_ref(id: &str) -> NodeCapabilityContractRef {
    NodeCapabilityContractRef::new(
        NodeCapabilityContractId::new(id).unwrap(),
        NodeCapabilityContractVersion::new(1, 0).unwrap(),
    )
}

fn uuid(seed: u8) -> Uuid {
    let mut bytes = [seed; 16];
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}
