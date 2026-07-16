use engine::{
    node_capability::{
        NodeCapabilityContractId, NodeCapabilityContractRef, NodeCapabilityContractVersion,
        NodeCapabilityInputKey, NodeCapabilityInputRoleKey, NodeCapabilityNormalizedParameters,
        NodeCapabilityOutputKey, WorkflowInputItemId, WorkflowNodeExecutionId,
    },
    workflow::{
        WorkflowApplicationError, WorkflowExecutionPlan, WorkflowPlannedInputBinding,
        WorkflowPlannedNode, WorkflowRunScope,
    },
    workflow_graph::{
        WorkflowId, WorkflowInputBinding, WorkflowInputItemEntity, WorkflowNodeId,
        WorkflowOrderedInputItems, WorkflowRevision,
    },
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::super::persistence;

#[derive(Serialize, Deserialize)]
pub(super) struct PlanPayload {
    workflow_id: Uuid,
    revision: u64,
    selected_node_id: Option<Uuid>,
    nodes: Vec<PlannedNodePayload>,
}

#[derive(Serialize, Deserialize)]
struct PlannedNodePayload {
    node_id: Uuid,
    execution_id: Uuid,
    contract_id: String,
    contract_major: u16,
    contract_minor: u16,
    normalized_parameters: Vec<u8>,
    bindings: Vec<PlannedBindingPayload>,
}

#[derive(Serialize, Deserialize)]
struct PlannedBindingPayload {
    input_key: String,
    ordered: bool,
    items: Vec<PlannedItemPayload>,
}

#[derive(Serialize, Deserialize)]
struct PlannedItemPayload {
    item_id: Uuid,
    source_node_id: Uuid,
    source_output_key: String,
    role_key: Option<String>,
}

pub(super) fn encode_plan(plan: &WorkflowExecutionPlan) -> PlanPayload {
    PlanPayload {
        workflow_id: plan.workflow_id().as_uuid(),
        revision: plan.workflow_revision().get(),
        selected_node_id: plan.scope().selected_node_id().map(WorkflowNodeId::as_uuid),
        nodes: plan
            .nodes()
            .iter()
            .map(|node| PlannedNodePayload {
                node_id: node.node_id.as_uuid(),
                execution_id: node.node_execution_id.as_uuid(),
                contract_id: node.capability_contract.id().as_str().to_owned(),
                contract_major: node.capability_contract.version().major(),
                contract_minor: node.capability_contract.version().minor(),
                normalized_parameters: node.normalized_parameters.canonical_bytes(),
                bindings: node.input_bindings.iter().map(encode_binding).collect(),
            })
            .collect(),
    }
}

pub(super) fn decode_plan(
    payload: PlanPayload,
) -> Result<WorkflowExecutionPlan, WorkflowApplicationError> {
    let scope = payload
        .selected_node_id
        .map(WorkflowNodeId::from_uuid)
        .transpose()?
        .map_or(WorkflowRunScope::WholeWorkflow, WorkflowRunScope::ThroughNode);
    let nodes = payload.nodes.into_iter().map(decode_node).collect::<Result<Vec<_>, _>>()?;
    WorkflowExecutionPlan::try_new(
        WorkflowId::from_uuid(payload.workflow_id)?,
        WorkflowRevision::new(payload.revision)?,
        scope,
        nodes,
    )
    .map_err(Into::into)
}

fn decode_node(node: PlannedNodePayload) -> Result<WorkflowPlannedNode, WorkflowApplicationError> {
    Ok(WorkflowPlannedNode {
        node_id: WorkflowNodeId::from_uuid(node.node_id)?,
        node_execution_id: WorkflowNodeExecutionId::from_uuid(node.execution_id)
            .ok_or_else(persistence)?,
        capability_contract: NodeCapabilityContractRef::new(
            NodeCapabilityContractId::new(node.contract_id).map_err(|_| persistence())?,
            NodeCapabilityContractVersion::new(node.contract_major, node.contract_minor)
                .map_err(|_| persistence())?,
        ),
        normalized_parameters: NodeCapabilityNormalizedParameters::try_from_canonical_bytes(
            &node.normalized_parameters,
        )
        .map_err(|_| persistence())?,
        input_bindings: node
            .bindings
            .into_iter()
            .map(decode_binding)
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn encode_binding(binding: &WorkflowPlannedInputBinding) -> PlannedBindingPayload {
    PlannedBindingPayload {
        input_key: binding.input_key.as_str().to_owned(),
        ordered: matches!(binding.binding, WorkflowInputBinding::OrderedReferences { .. }),
        items: binding
            .binding
            .items()
            .map(|item| PlannedItemPayload {
                item_id: item.id.as_uuid(),
                source_node_id: item.source_node_id.as_uuid(),
                source_output_key: item.source_output_key.as_str().to_owned(),
                role_key: item.input_role_key.as_ref().map(|key| key.as_str().to_owned()),
            })
            .collect(),
    }
}

fn decode_binding(
    binding: PlannedBindingPayload,
) -> Result<WorkflowPlannedInputBinding, WorkflowApplicationError> {
    let items = binding
        .items
        .into_iter()
        .map(|item| {
            Ok(WorkflowInputItemEntity {
                id: WorkflowInputItemId::from_uuid(item.item_id).ok_or_else(persistence)?,
                source_node_id: WorkflowNodeId::from_uuid(item.source_node_id)?,
                source_output_key: NodeCapabilityOutputKey::new(item.source_output_key)
                    .map_err(|_| persistence())?,
                input_role_key: item
                    .role_key
                    .map(NodeCapabilityInputRoleKey::new)
                    .transpose()
                    .map_err(|_| persistence())?,
            })
        })
        .collect::<Result<Vec<_>, WorkflowApplicationError>>()?;
    let value = if binding.ordered {
        WorkflowInputBinding::ordered_references(WorkflowOrderedInputItems::try_new(items)?)
    } else {
        let [item] = items.try_into().map_err(|_| persistence())?;
        WorkflowInputBinding::try_single(item)?
    };
    Ok(WorkflowPlannedInputBinding {
        input_key: NodeCapabilityInputKey::new(binding.input_key).map_err(|_| persistence())?,
        binding: value,
    })
}
