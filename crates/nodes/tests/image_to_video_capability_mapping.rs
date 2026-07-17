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

#[tokio::test]
async fn image_to_video_starts_with_exact_readable_snapshot_and_ordered_mapping() {
    let context = execution_context(1);
    let origin = execution_origin(1);
    let bytes = vec![7; 24];
    let image = image_reference(10, &bytes);
    let reader = NodeCapabilityManagedMediaReaderFakeImpl::default();
    reader
        .register_managed_media(
            context.project_id,
            NodeCapabilityManagedMediaReference::Image(image),
            NodeCapabilityMediaMimeType::ImagePng,
            NodeCapabilityDeclaredMediaFacts::try_image(32, 32).unwrap(),
            bytes,
        )
        .unwrap();
    let starter = NodeCapabilityGenerationTaskStarterFakeImpl::default();
    let capability = ImageToVideoCapabilityImpl::try_new(
        catalog(),
        GenerationProfileAlwaysAvailableFakeImpl,
        reader,
        starter.clone(),
    )
    .unwrap();
    let profile = profile();
    let request = execution_request(&capability, &profile, context.clone(), origin.clone(), image);

    assert_eq!(
        capability.execute_node_capability(request).await.unwrap(),
        WorkflowNodeCapabilityExecutionOutcome::WaitingForGenerationTask
    );
    let requests = starter.requests();
    let started = requests.first().unwrap();
    assert_eq!(started.context().project_id, context.project_id);
    assert_eq!(started.context().workflow_run_id, context.workflow_run_id);
    assert_eq!(started.context().node_execution_id, context.node_execution_id);
    assert_eq!(started.context().deadline, context.deadline);
    assert_eq!(started.origin(), &origin);
    assert_eq!(started.profile_ref(), &profile);
    assert_eq!(started.primary_output_key().as_str(), "video");
    assert_eq!(started.input_assets(), [NodeCapabilityGenerationTaskAssetSnapshot::image(image)]);
    match started.request() {
        NodeCapabilityGenerationTaskRequest::Video { input_image, prompt, duration_seconds } => {
            assert_eq!(*input_image, NodeCapabilityGenerationTaskAssetSnapshot::image(image));
            assert_eq!(
                prompt.as_ref().unwrap().parts(),
                [WorkflowTextPart::Literal("slow camera".into())]
            );
            assert_eq!(*duration_seconds, ImageToVideoDurationSeconds::Ten);
        }
        _ => panic!("expected Video task request"),
    }
}

fn execution_request(
    capability: &impl WorkflowNodeCapabilityInterface,
    profile: &GenerationProfileRef,
    context: WorkflowNodeExecutionContext,
    origin: WorkflowNodeExecutionOrigin,
    image: WorkflowManagedImageRef,
) -> NodeCapabilityExecutionRequest {
    let parameters = NodeCapabilityParameterSet::try_from_map(BTreeMap::from([
        (
            NodeCapabilityParameterKey::new("generation_profile_ref").unwrap(),
            NodeCapabilityParameterValue::GenerationProfile(
                profile.to_node_capability_parameter_value().unwrap(),
            ),
        ),
        (
            NodeCapabilityParameterKey::new("duration_seconds").unwrap(),
            NodeCapabilityParameterValue::UnsignedInteger(10),
        ),
    ]))
    .unwrap();
    let inputs = WorkflowNodeInputSet::try_new(
        capability.node_capability_contract(),
        BTreeMap::from([
            (
                NodeCapabilityInputKey::new("image").unwrap(),
                WorkflowNodeInputValue::Single(item(WorkflowRuntimeValue::Image(image), 20)),
            ),
            (
                NodeCapabilityInputKey::new("prompt").unwrap(),
                WorkflowNodeInputValue::Single(item(
                    WorkflowRuntimeValue::Text(
                        WorkflowTextValue::try_new([WorkflowTextPart::Literal(
                            "slow camera".into(),
                        )])
                        .unwrap(),
                    ),
                    21,
                )),
            ),
        ]),
    )
    .unwrap();
    NodeCapabilityExecutionRequest {
        context,
        origin,
        normalized_parameters: capability.normalize_node_parameters(&parameters).unwrap(),
        inputs,
    }
}

fn item(value: WorkflowRuntimeValue, seed: u8) -> WorkflowRuntimeInputItem {
    WorkflowRuntimeInputItem {
        input_item_id: WorkflowInputItemId::from_uuid(uuid(seed)).unwrap(),
        input_role_key: None,
        value,
    }
}

fn image_reference(seed: u8, bytes: &[u8]) -> WorkflowManagedImageRef {
    WorkflowManagedImageRef::new(
        WorkflowManagedAssetIdBoundaryValue::from_bytes(uuid(seed).into_bytes()).unwrap(),
        WorkflowManagedContentFingerprint::from_bytes(Sha256::digest(bytes).into()),
    )
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

fn execution_origin(seed: u8) -> WorkflowNodeExecutionOrigin {
    WorkflowNodeExecutionOrigin::new(
        WorkflowId::from_uuid(uuid(seed + 90)).unwrap(),
        WorkflowRevision::new(2).unwrap(),
        WorkflowNodeId::from_uuid(uuid(seed + 120)).unwrap(),
        contract_ref(),
    )
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

fn uuid(seed: u8) -> Uuid {
    let mut bytes = [seed; 16];
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}
