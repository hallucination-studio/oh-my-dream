use std::{collections::BTreeMap, sync::Arc};

use async_trait::async_trait;
use engine::{
    node_capability::{
        NodeCapabilityContract, NodeCapabilityContractId, NodeCapabilityContractRef,
        NodeCapabilityContractVersion, NodeCapabilityExecutionError, NodeCapabilityExecutionKind,
        NodeCapabilityExecutionRequest, NodeCapabilityInputBindingContract,
        NodeCapabilityInputContract, NodeCapabilityInputKey, NodeCapabilityNormalizedParameters,
        NodeCapabilityOutputContract, NodeCapabilityOutputKey, NodeCapabilityParameterError,
        NodeCapabilityParameterSet, NodeCapabilityReadinessIssue, NodeCapabilityReadinessRequest,
        WorkflowDataType, WorkflowInputItemId, WorkflowNodeCapabilityInterface,
        WorkflowNodeCapabilityRegistry, WorkflowNodeOutputSet,
    },
    workflow_graph::{
        WorkflowAggregate, WorkflowAggregateRestoreData, WorkflowCanvasPosition, WorkflowCreatedAt,
        WorkflowGraphError, WorkflowId, WorkflowInputBinding, WorkflowInputItemEntity,
        WorkflowInputTarget, WorkflowNodeEntity, WorkflowNodeId, WorkflowRevision,
        WorkflowSchemaVersion, WorkflowUpdatedAt,
    },
};
use projects::project::domain::ProjectId;
use uuid::Uuid;

#[test]
fn reconstruction_preserves_fan_out_and_derives_indexes() {
    let registry = registry();
    let source = node(1, "media.source");
    let first_target = node(2, "media.consume");
    let second_target = node(3, "media.consume");
    let first_item = item(11, source.id);
    let second_item = item(12, source.id);
    let aggregate = WorkflowAggregate::try_restore(
        restore_data(
            vec![source, first_target.clone(), second_target.clone()],
            vec![
                binding(first_target.id, first_item.clone()),
                binding(second_target.id, second_item.clone()),
            ],
        ),
        &registry,
    )
    .unwrap();

    assert_eq!(
        aggregate.derive_outgoing_input_item_ids()[&source_id()],
        [first_item.id, second_item.id]
    );
    assert_eq!(aggregate.derive_incoming_input_item_ids()[&first_target.id], [first_item.id]);
    assert_eq!(aggregate.input_bindings().len(), 2);
}

#[test]
fn reconstruction_rejects_duplicate_item_identity_and_type_mismatch() {
    let registry = registry();
    let source = node(1, "media.source");
    let first_target = node(2, "media.consume");
    let second_target = node(3, "media.consume");
    let repeated = item(11, source.id);
    let duplicate = WorkflowAggregate::try_restore(
        restore_data(
            vec![source.clone(), first_target.clone(), second_target.clone()],
            vec![binding(first_target.id, repeated.clone()), binding(second_target.id, repeated)],
        ),
        &registry,
    );
    assert_eq!(duplicate.unwrap_err(), WorkflowGraphError::DuplicateInputItem);

    let text_source = node(4, "text.source");
    let mismatch = WorkflowAggregate::try_restore(
        restore_data(
            vec![text_source.clone(), first_target.clone()],
            vec![binding(first_target.id, item(13, text_source.id))],
        ),
        &registry,
    );
    assert_eq!(mismatch.unwrap_err(), WorkflowGraphError::DataTypeMismatch);
}

#[test]
fn reconstruction_rejects_cycles() {
    let registry = registry();
    let first = node(2, "media.consume");
    let second = node(3, "media.consume");
    let result = WorkflowAggregate::try_restore(
        restore_data(
            vec![first.clone(), second.clone()],
            vec![binding(first.id, item(11, second.id)), binding(second.id, item(12, first.id))],
        ),
        &registry,
    );
    assert_eq!(result.unwrap_err(), WorkflowGraphError::Cycle);
}

fn registry() -> WorkflowNodeCapabilityRegistry {
    let implementations: Vec<Arc<dyn WorkflowNodeCapabilityInterface>> = vec![
        Arc::new(FakeCapabilityImpl::new("media.source", WorkflowDataType::Image, false)),
        Arc::new(FakeCapabilityImpl::new("text.source", WorkflowDataType::Text, false)),
        Arc::new(FakeCapabilityImpl::new("media.consume", WorkflowDataType::Image, true)),
    ];
    WorkflowNodeCapabilityRegistry::try_new(implementations).unwrap()
}

fn restore_data(
    nodes: Vec<WorkflowNodeEntity>,
    input_bindings: Vec<(WorkflowInputTarget, WorkflowInputBinding)>,
) -> WorkflowAggregateRestoreData {
    WorkflowAggregateRestoreData {
        schema_version: WorkflowSchemaVersion::CURRENT,
        id: WorkflowId::from_uuid(uuid(20)).unwrap(),
        project_id: ProjectId::from_uuid(uuid(21)).unwrap(),
        revision: WorkflowRevision::new(1).unwrap(),
        created_at: WorkflowCreatedAt::from_utc_milliseconds(100).unwrap(),
        updated_at: WorkflowUpdatedAt::from_utc_milliseconds(100).unwrap(),
        nodes,
        input_bindings,
    }
}

fn node(seed: u8, capability_id: &str) -> WorkflowNodeEntity {
    WorkflowNodeEntity {
        id: WorkflowNodeId::from_uuid(uuid(seed)).unwrap(),
        capability_contract: capability_ref(capability_id),
        parameter_set: NodeCapabilityParameterSet::try_from_map(BTreeMap::new()).unwrap(),
        canvas_position: WorkflowCanvasPosition::try_new(0.0, 0.0).unwrap(),
    }
}

fn binding(
    target_node_id: WorkflowNodeId,
    item: WorkflowInputItemEntity,
) -> (WorkflowInputTarget, WorkflowInputBinding) {
    (
        WorkflowInputTarget {
            node_id: target_node_id,
            input_key: NodeCapabilityInputKey::new("input").unwrap(),
        },
        WorkflowInputBinding::try_single(item).unwrap(),
    )
}

fn item(seed: u8, source_node_id: WorkflowNodeId) -> WorkflowInputItemEntity {
    WorkflowInputItemEntity {
        id: WorkflowInputItemId::from_uuid(uuid(seed)).unwrap(),
        source_node_id,
        source_output_key: NodeCapabilityOutputKey::new("output").unwrap(),
        input_role_key: None,
    }
}

fn source_id() -> WorkflowNodeId {
    WorkflowNodeId::from_uuid(uuid(1)).unwrap()
}

struct FakeCapabilityImpl {
    contract: NodeCapabilityContract,
}

impl FakeCapabilityImpl {
    fn new(id: &str, output_type: WorkflowDataType, accepts_input: bool) -> Self {
        let inputs = accepts_input
            .then(|| {
                NodeCapabilityInputContract::new(
                    NodeCapabilityInputKey::new("input").unwrap(),
                    NodeCapabilityInputBindingContract::RequiredSingleValue {
                        data_type: WorkflowDataType::Image,
                    },
                )
                .unwrap()
            })
            .into_iter()
            .collect();
        Self {
            contract: NodeCapabilityContract::try_new(
                capability_ref(id),
                Vec::new(),
                inputs,
                vec![NodeCapabilityOutputContract::new(
                    NodeCapabilityOutputKey::new("output").unwrap(),
                    output_type,
                    true,
                )],
                NodeCapabilityExecutionKind::PureValue,
            )
            .unwrap(),
        }
    }
}

#[async_trait]
impl WorkflowNodeCapabilityInterface for FakeCapabilityImpl {
    fn node_capability_contract(&self) -> &NodeCapabilityContract {
        &self.contract
    }

    fn normalize_node_parameters(
        &self,
        parameters: &NodeCapabilityParameterSet,
    ) -> Result<NodeCapabilityNormalizedParameters, NodeCapabilityParameterError> {
        self.contract.normalize_node_parameters(parameters)
    }

    async fn check_node_external_readiness(
        &self,
        _request: NodeCapabilityReadinessRequest,
    ) -> Vec<NodeCapabilityReadinessIssue> {
        Vec::new()
    }

    async fn execute_node_capability(
        &self,
        _request: NodeCapabilityExecutionRequest,
    ) -> Result<WorkflowNodeOutputSet, NodeCapabilityExecutionError> {
        unreachable!("graph reconstruction never executes a capability")
    }
}

fn capability_ref(id: &str) -> NodeCapabilityContractRef {
    NodeCapabilityContractRef::new(
        NodeCapabilityContractId::new(id).unwrap(),
        NodeCapabilityContractVersion::new(1, 0).unwrap(),
    )
}

fn uuid(seed: u8) -> Uuid {
    Uuid::from_bytes([seed, 0, 0, 0, 0, 0, 0x40, 0, 0x80, 0, 0, 0, 0, 0, 0, seed])
}
