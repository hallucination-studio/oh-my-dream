use std::{collections::BTreeMap, sync::Arc};

use async_trait::async_trait;
use engine::{
    node_capability::{
        NodeCapabilityContract, NodeCapabilityContractId, NodeCapabilityContractRef,
        NodeCapabilityContractVersion, NodeCapabilityExecutionError, NodeCapabilityExecutionKind,
        NodeCapabilityExecutionRequest, NodeCapabilityInputBindingContract,
        NodeCapabilityInputContract, NodeCapabilityInputKey, NodeCapabilityInputRoleKey,
        NodeCapabilityNormalizedParameters, NodeCapabilityOutputContract, NodeCapabilityOutputKey,
        NodeCapabilityParameterError, NodeCapabilityParameterSet, NodeCapabilityReadinessIssue,
        NodeCapabilityReadinessRequest, WorkflowAcceptedDataTypeSet, WorkflowDataType,
        WorkflowInputItemId, WorkflowNodeCapabilityInterface, WorkflowNodeCapabilityRegistry,
        WorkflowNodeOutputSet,
    },
    workflow_graph::{
        WorkflowAggregate, WorkflowAggregateRestoreData, WorkflowCanvasPosition, WorkflowCreatedAt,
        WorkflowGraphError, WorkflowId, WorkflowInputBinding, WorkflowInputItemEntity,
        WorkflowInputTarget, WorkflowNodeEntity, WorkflowNodeId, WorkflowOrderedInputItems,
        WorkflowRevision, WorkflowSchemaVersion, WorkflowUpdatedAt,
    },
};
use projects::project::domain::ProjectId;
use uuid::Uuid;

#[test]
fn nine_mixed_media_references_preserve_identity_role_and_order_on_reconstruction() {
    let registry = registry();
    let (nodes, items) = mixed_media_graph();
    let expected =
        items.iter().map(|item| (item.id, item.input_role_key.clone())).collect::<Vec<_>>();
    let aggregate = WorkflowAggregate::try_restore(restore_data(nodes, items), &registry).unwrap();
    let target = input_target();
    let restored = aggregate.input_bindings()[&target]
        .items()
        .map(|item| (item.id, item.input_role_key.clone()))
        .collect::<Vec<_>>();

    assert_eq!(restored, expected);
}

#[test]
fn reconstruction_rejects_reference_cardinality_and_undeclared_roles() {
    let registry = registry();
    let (nodes, mut items) = mixed_media_graph();
    items.pop();
    assert_eq!(
        WorkflowAggregate::try_restore(restore_data(nodes.clone(), items), &registry).unwrap_err(),
        WorkflowGraphError::CardinalityViolation
    );

    let (_, mut items) = mixed_media_graph();
    items[0].input_role_key = Some(NodeCapabilityInputRoleKey::new("style").unwrap());
    assert_eq!(
        WorkflowAggregate::try_restore(restore_data(nodes, items), &registry).unwrap_err(),
        WorkflowGraphError::RoleViolation
    );
}

fn mixed_media_graph() -> (Vec<WorkflowNodeEntity>, Vec<WorkflowInputItemEntity>) {
    let kinds = [
        ("image.source", "subject"),
        ("video.source", "motion"),
        ("audio.source", "audio_guidance"),
    ];
    let mut nodes = Vec::new();
    let mut items = Vec::new();
    for index in 0_u8..9 {
        let (capability_id, role) = kinds[usize::from(index % 3)];
        let source = node(index + 1, capability_id);
        items.push(WorkflowInputItemEntity {
            id: WorkflowInputItemId::from_uuid(uuid(index + 30)).unwrap(),
            source_node_id: source.id,
            source_output_key: NodeCapabilityOutputKey::new("output").unwrap(),
            input_role_key: Some(NodeCapabilityInputRoleKey::new(role).unwrap()),
        });
        nodes.push(source);
    }
    nodes.push(node(20, "media.consume_mixed_references"));
    (nodes, items)
}

fn restore_data(
    nodes: Vec<WorkflowNodeEntity>,
    items: Vec<WorkflowInputItemEntity>,
) -> WorkflowAggregateRestoreData {
    WorkflowAggregateRestoreData {
        schema_version: WorkflowSchemaVersion::CURRENT,
        id: WorkflowId::from_uuid(uuid(21)).unwrap(),
        project_id: ProjectId::from_uuid(uuid(22)).unwrap(),
        revision: WorkflowRevision::new(1).unwrap(),
        created_at: WorkflowCreatedAt::from_utc_milliseconds(100).unwrap(),
        updated_at: WorkflowUpdatedAt::from_utc_milliseconds(100).unwrap(),
        nodes,
        input_bindings: vec![(
            input_target(),
            WorkflowInputBinding::ordered_references(
                WorkflowOrderedInputItems::try_new(items).unwrap(),
            ),
        )],
    }
}

fn input_target() -> WorkflowInputTarget {
    WorkflowInputTarget {
        node_id: WorkflowNodeId::from_uuid(uuid(20)).unwrap(),
        input_key: NodeCapabilityInputKey::new("references").unwrap(),
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

fn registry() -> WorkflowNodeCapabilityRegistry {
    let implementations: Vec<Arc<dyn WorkflowNodeCapabilityInterface>> = vec![
        Arc::new(FakeCapability::source("image.source", WorkflowDataType::Image)),
        Arc::new(FakeCapability::source("video.source", WorkflowDataType::Video)),
        Arc::new(FakeCapability::source("audio.source", WorkflowDataType::Audio)),
        Arc::new(FakeCapability::mixed_consumer()),
    ];
    WorkflowNodeCapabilityRegistry::try_new(implementations).unwrap()
}

struct FakeCapability {
    contract: NodeCapabilityContract,
}

impl FakeCapability {
    fn source(id: &str, output_type: WorkflowDataType) -> Self {
        Self::new(id, Vec::new(), output_type)
    }

    fn mixed_consumer() -> Self {
        let role_types = [
            ("subject", WorkflowDataType::Image),
            ("motion", WorkflowDataType::Video),
            ("audio_guidance", WorkflowDataType::Audio),
        ];
        let accepted_data_types_by_role = role_types
            .into_iter()
            .map(|(role, data_type)| {
                (
                    NodeCapabilityInputRoleKey::new(role).unwrap(),
                    WorkflowAcceptedDataTypeSet::try_new([data_type]).unwrap(),
                )
            })
            .collect();
        let input = NodeCapabilityInputContract::new(
            NodeCapabilityInputKey::new("references").unwrap(),
            NodeCapabilityInputBindingContract::OrderedReferences {
                minimum_items: 9,
                maximum_items: Some(9),
                accepted_data_types_by_role,
            },
        )
        .unwrap();
        Self::new("media.consume_mixed_references", vec![input], WorkflowDataType::Image)
    }

    fn new(
        id: &str,
        inputs: Vec<NodeCapabilityInputContract>,
        output_type: WorkflowDataType,
    ) -> Self {
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
impl WorkflowNodeCapabilityInterface for FakeCapability {
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
        unreachable!("checkpoint tests never execute capabilities")
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
