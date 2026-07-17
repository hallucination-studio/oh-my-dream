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
async fn task_start_failures_remain_closed_and_target_the_capability() {
    for (index, failure) in [
        NodeCapabilityGenerationTaskStartFailure::InvalidRequest,
        NodeCapabilityGenerationTaskStartFailure::Conflict,
        NodeCapabilityGenerationTaskStartFailure::Unavailable,
        NodeCapabilityGenerationTaskStartFailure::Persistence,
    ]
    .into_iter()
    .enumerate()
    {
        let capability = TextToImageCapabilityImpl::try_new(
            catalog(),
            GenerationProfileAlwaysAvailableFakeImpl,
            NodeCapabilityGenerationTaskStarterFakeImpl::failing(failure),
        )
        .unwrap();
        let error = capability
            .execute_node_capability(request(
                &capability,
                index as u8 + 1,
                active_context(index as u8 + 1),
            ))
            .await
            .unwrap_err();
        assert_eq!(error.stage(), NodeCapabilityExecutionStage::StartGenerationTask);
        assert_eq!(error.target(), &NodeCapabilityExecutionTarget::Capability);
        assert_eq!(error.failure(), &NodeCapabilityExecutionFailure::GenerationTaskStart(failure));
    }
}

fn request(
    capability: &impl WorkflowNodeCapabilityInterface,
    seed: u8,
    context: WorkflowNodeExecutionContext,
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
            NodeCapabilityInputKey::new("prompt").unwrap(),
            WorkflowNodeInputValue::Single(WorkflowRuntimeInputItem {
                input_item_id: WorkflowInputItemId::from_uuid(uuid(seed + 10)).unwrap(),
                input_role_key: None,
                value: WorkflowRuntimeValue::Text(
                    WorkflowTextValue::try_new([WorkflowTextPart::Literal("draw".into())]).unwrap(),
                ),
            }),
        )]),
    )
    .unwrap();
    NodeCapabilityExecutionRequest {
        context,
        origin: WorkflowNodeExecutionOrigin::new(
            WorkflowId::from_uuid(uuid(seed + 90)).unwrap(),
            WorkflowRevision::new(1).unwrap(),
            WorkflowNodeId::from_uuid(uuid(seed + 120)).unwrap(),
            contract_ref(),
        ),
        normalized_parameters: capability.normalize_node_parameters(&parameters).unwrap(),
        inputs,
    }
}

fn active_context(seed: u8) -> WorkflowNodeExecutionContext {
    WorkflowNodeExecutionContext {
        project_id: ProjectId::from_uuid(uuid(seed)).unwrap(),
        workflow_run_id: WorkflowRunId::from_uuid(uuid(seed + 30)).unwrap(),
        node_execution_id: WorkflowNodeExecutionId::from_uuid(uuid(seed + 60)).unwrap(),
        deadline: NodeCapabilityExecutionDeadline::at(Instant::now() + Duration::from_secs(5)),
        cancellation: NodeCapabilityExecutionCancellation::active(),
    }
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
fn uuid(seed: u8) -> Uuid {
    let mut bytes = [seed; 16];
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}
