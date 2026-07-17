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
        WorkflowInputItemId, WorkflowNodeCapabilityExecutionOutcome,
        WorkflowNodeCapabilityInterface, WorkflowNodeCapabilityRegistry,
    },
    workflow_graph::{
        WorkflowAggregate, WorkflowAggregateRestoreData, WorkflowApplyMutationCommand,
        WorkflowCanvasPosition, WorkflowCreatedAt, WorkflowId, WorkflowInputBinding,
        WorkflowInputItemEntity, WorkflowInputTarget, WorkflowMutationAction,
        WorkflowMutationRequestId, WorkflowNodeEntity, WorkflowNodeId, WorkflowRevision,
        WorkflowSchemaVersion, WorkflowUpdatedAt,
    },
};
use projects::project::domain::ProjectId;
use uuid::Uuid;

pub(crate) fn empty_aggregate(registry: &WorkflowNodeCapabilityRegistry) -> WorkflowAggregate {
    restore_aggregate(registry, Vec::new(), Vec::new())
}

pub(crate) fn restore_aggregate(
    registry: &WorkflowNodeCapabilityRegistry,
    nodes: Vec<WorkflowNodeEntity>,
    input_bindings: Vec<(WorkflowInputTarget, WorkflowInputBinding)>,
) -> WorkflowAggregate {
    WorkflowAggregate::try_restore(
        WorkflowAggregateRestoreData {
            schema_version: WorkflowSchemaVersion::CURRENT,
            id: workflow_id(),
            project_id: ProjectId::from_uuid(uuid(2)).unwrap(),
            revision: WorkflowRevision::new(1).unwrap(),
            created_at: WorkflowCreatedAt::from_utc_milliseconds(100).unwrap(),
            updated_at: WorkflowUpdatedAt::from_utc_milliseconds(100).unwrap(),
            nodes,
            input_bindings,
        },
        registry,
    )
    .unwrap()
}

pub(crate) fn command(actions: Vec<WorkflowMutationAction>) -> WorkflowApplyMutationCommand {
    WorkflowApplyMutationCommand::try_new(
        WorkflowMutationRequestId::from_uuid(uuid(8)).unwrap(),
        workflow_id(),
        WorkflowRevision::new(1).unwrap(),
        actions,
    )
    .unwrap()
}

pub(crate) fn registry() -> WorkflowNodeCapabilityRegistry {
    let implementations: Vec<Arc<dyn WorkflowNodeCapabilityInterface>> = vec![
        Arc::new(FakeCapabilityImpl::new("image.source", Vec::new())),
        Arc::new(FakeCapabilityImpl::new("image.single", vec![single_input()])),
        Arc::new(FakeCapabilityImpl::new("image.references", vec![reference_input()])),
    ];
    WorkflowNodeCapabilityRegistry::try_new(implementations).unwrap()
}

fn single_input() -> NodeCapabilityInputContract {
    NodeCapabilityInputContract::new(
        NodeCapabilityInputKey::new("input").unwrap(),
        NodeCapabilityInputBindingContract::RequiredSingleValue {
            data_type: WorkflowDataType::Image,
        },
    )
    .unwrap()
}

fn reference_input() -> NodeCapabilityInputContract {
    NodeCapabilityInputContract::new(
        NodeCapabilityInputKey::new("references").unwrap(),
        NodeCapabilityInputBindingContract::OrderedReferences {
            minimum_items: 1,
            maximum_items: Some(4),
            accepted_data_types_by_role: BTreeMap::from([
                accepted_role("subject"),
                accepted_role("style"),
            ]),
        },
    )
    .unwrap()
}

fn accepted_role(role: &str) -> (NodeCapabilityInputRoleKey, WorkflowAcceptedDataTypeSet) {
    (
        NodeCapabilityInputRoleKey::new(role).unwrap(),
        WorkflowAcceptedDataTypeSet::try_new([WorkflowDataType::Image]).unwrap(),
    )
}

struct FakeCapabilityImpl {
    contract: NodeCapabilityContract,
}

impl FakeCapabilityImpl {
    fn new(id: &str, inputs: Vec<NodeCapabilityInputContract>) -> Self {
        Self {
            contract: NodeCapabilityContract::try_new(
                capability_ref(id),
                Vec::new(),
                inputs,
                vec![NodeCapabilityOutputContract::new(
                    NodeCapabilityOutputKey::new("image").unwrap(),
                    WorkflowDataType::Image,
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
    ) -> Result<WorkflowNodeCapabilityExecutionOutcome, NodeCapabilityExecutionError> {
        unreachable!("mutation tests never execute capabilities")
    }
}

pub(crate) fn empty_parameters() -> NodeCapabilityParameterSet {
    NodeCapabilityParameterSet::try_from_map(BTreeMap::new()).unwrap()
}

pub(crate) fn node(seed: u8, capability_id: &str) -> WorkflowNodeEntity {
    WorkflowNodeEntity {
        id: node_id(seed),
        capability_contract: capability_ref(capability_id),
        parameter_set: empty_parameters(),
        canvas_position: position(0.0),
    }
}

pub(crate) fn target(node_id: WorkflowNodeId, input_key: &str) -> WorkflowInputTarget {
    WorkflowInputTarget { node_id, input_key: NodeCapabilityInputKey::new(input_key).unwrap() }
}

pub(crate) fn input_item(
    seed: u8,
    source_node_id: WorkflowNodeId,
    role: Option<&str>,
) -> WorkflowInputItemEntity {
    WorkflowInputItemEntity {
        id: WorkflowInputItemId::from_uuid(uuid(seed)).unwrap(),
        source_node_id,
        source_output_key: NodeCapabilityOutputKey::new("image").unwrap(),
        input_role_key: role.map(|value| NodeCapabilityInputRoleKey::new(value).unwrap()),
    }
}

pub(crate) fn position(x: f64) -> WorkflowCanvasPosition {
    WorkflowCanvasPosition::try_new(x, 0.0).unwrap()
}

fn workflow_id() -> WorkflowId {
    WorkflowId::from_uuid(uuid(1)).unwrap()
}

pub(crate) fn node_id(seed: u8) -> WorkflowNodeId {
    WorkflowNodeId::from_uuid(uuid(seed)).unwrap()
}

pub(crate) fn capability_ref(id: &str) -> NodeCapabilityContractRef {
    NodeCapabilityContractRef::new(
        NodeCapabilityContractId::new(id).unwrap(),
        NodeCapabilityContractVersion::new(1, 0).unwrap(),
    )
}

fn uuid(seed: u8) -> Uuid {
    Uuid::from_bytes([seed, 0, 0, 0, 0, 0, 0x40, 0, 0x80, 0, 0, 0, 0, 0, 0, seed])
}
