use engine::{
    node_capability::{
        NodeCapabilityContractId, NodeCapabilityContractRef, NodeCapabilityContractVersion,
        NodeCapabilityInputKey, NodeCapabilityInputRoleKey, NodeCapabilityOutputKey,
        NodeCapabilityParameterSet, WorkflowInputItemId, WorkflowNodeCapabilityRegistry,
    },
    workflow::WorkflowApplicationError,
    workflow_graph::{
        WorkflowAggregate, WorkflowAggregateRestoreData, WorkflowCanvasPosition, WorkflowCreatedAt,
        WorkflowId, WorkflowInputBinding, WorkflowInputItemEntity, WorkflowInputTarget,
        WorkflowNodeEntity, WorkflowNodeId, WorkflowOrderedInputItems, WorkflowRevision,
        WorkflowSchemaVersion, WorkflowUpdatedAt,
    },
};
use projects::project::domain::ProjectId;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::persistence;

#[derive(Serialize, Deserialize)]
struct WorkflowPayload {
    schema_version: u16,
    workflow_id: Uuid,
    project_id: Uuid,
    revision: u64,
    created_at: i64,
    updated_at: i64,
    nodes: Vec<NodePayload>,
    bindings: Vec<BindingPayload>,
}

#[derive(Serialize, Deserialize)]
struct NodePayload {
    node_id: Uuid,
    contract_id: String,
    contract_major: u16,
    contract_minor: u16,
    parameters: Vec<u8>,
    x: f64,
    y: f64,
}

#[derive(Serialize, Deserialize)]
struct BindingPayload {
    target_node_id: Uuid,
    input_key: String,
    ordered: bool,
    items: Vec<InputItemPayload>,
}

#[derive(Serialize, Deserialize)]
struct InputItemPayload {
    input_item_id: Uuid,
    source_node_id: Uuid,
    source_output_key: String,
    input_role_key: Option<String>,
}

pub(super) fn encode_workflow(
    workflow: &WorkflowAggregate,
) -> Result<Vec<u8>, WorkflowApplicationError> {
    let payload = WorkflowPayload {
        schema_version: workflow.schema_version.get(),
        workflow_id: workflow.id.as_uuid(),
        project_id: workflow.project_id.as_uuid(),
        revision: workflow.revision.get(),
        created_at: workflow.created_at.as_utc_milliseconds(),
        updated_at: workflow.updated_at.as_utc_milliseconds(),
        nodes: workflow.nodes().values().map(encode_node).collect(),
        bindings: workflow
            .input_bindings()
            .iter()
            .map(|(target, binding)| encode_binding(target, binding))
            .collect(),
    };
    let bytes = serde_json::to_vec(&payload).map_err(|_| persistence())?;
    if bytes.len() > 1_048_576 { Err(persistence()) } else { Ok(bytes) }
}

pub(super) fn decode_workflow(
    bytes: &[u8],
    capabilities: &WorkflowNodeCapabilityRegistry,
) -> Result<WorkflowAggregate, WorkflowApplicationError> {
    if bytes.len() > 1_048_576 {
        return Err(persistence());
    }
    let payload: WorkflowPayload = serde_json::from_slice(bytes).map_err(|_| persistence())?;
    let nodes = payload.nodes.into_iter().map(decode_node).collect::<Result<Vec<_>, _>>()?;
    let bindings =
        payload.bindings.into_iter().map(decode_binding).collect::<Result<Vec<_>, _>>()?;
    WorkflowAggregate::try_restore(
        WorkflowAggregateRestoreData {
            schema_version: WorkflowSchemaVersion::new(payload.schema_version)?,
            id: WorkflowId::from_uuid(payload.workflow_id)?,
            project_id: ProjectId::from_uuid(payload.project_id).ok_or_else(persistence)?,
            revision: WorkflowRevision::new(payload.revision)?,
            created_at: WorkflowCreatedAt::from_utc_milliseconds(payload.created_at)?,
            updated_at: WorkflowUpdatedAt::from_utc_milliseconds(payload.updated_at)?,
            nodes,
            input_bindings: bindings,
        },
        capabilities,
    )
    .map_err(Into::into)
}

fn encode_node(node: &WorkflowNodeEntity) -> NodePayload {
    NodePayload {
        node_id: node.id.as_uuid(),
        contract_id: node.capability_contract.id().as_str().to_owned(),
        contract_major: node.capability_contract.version().major(),
        contract_minor: node.capability_contract.version().minor(),
        parameters: node.parameter_set.canonical_bytes(),
        x: node.canvas_position.x(),
        y: node.canvas_position.y(),
    }
}

fn decode_node(node: NodePayload) -> Result<WorkflowNodeEntity, WorkflowApplicationError> {
    Ok(WorkflowNodeEntity {
        id: WorkflowNodeId::from_uuid(node.node_id)?,
        capability_contract: NodeCapabilityContractRef::new(
            NodeCapabilityContractId::new(node.contract_id).map_err(|_| persistence())?,
            NodeCapabilityContractVersion::new(node.contract_major, node.contract_minor)
                .map_err(|_| persistence())?,
        ),
        parameter_set: NodeCapabilityParameterSet::try_from_canonical_bytes(&node.parameters)
            .map_err(|_| persistence())?,
        canvas_position: WorkflowCanvasPosition::try_new(node.x, node.y)?,
    })
}

fn encode_binding(target: &WorkflowInputTarget, binding: &WorkflowInputBinding) -> BindingPayload {
    BindingPayload {
        target_node_id: target.node_id.as_uuid(),
        input_key: target.input_key.as_str().to_owned(),
        ordered: matches!(binding, WorkflowInputBinding::OrderedReferences { .. }),
        items: binding.items().map(encode_item).collect(),
    }
}

fn encode_item(item: &WorkflowInputItemEntity) -> InputItemPayload {
    InputItemPayload {
        input_item_id: item.id.as_uuid(),
        source_node_id: item.source_node_id.as_uuid(),
        source_output_key: item.source_output_key.as_str().to_owned(),
        input_role_key: item.input_role_key.as_ref().map(|key| key.as_str().to_owned()),
    }
}

fn decode_binding(
    binding: BindingPayload,
) -> Result<(WorkflowInputTarget, WorkflowInputBinding), WorkflowApplicationError> {
    let target = WorkflowInputTarget {
        node_id: WorkflowNodeId::from_uuid(binding.target_node_id)?,
        input_key: NodeCapabilityInputKey::new(binding.input_key).map_err(|_| persistence())?,
    };
    let items = binding.items.into_iter().map(decode_item).collect::<Result<Vec<_>, _>>()?;
    let value = if binding.ordered {
        WorkflowInputBinding::ordered_references(
            WorkflowOrderedInputItems::try_new(items).map_err(WorkflowApplicationError::from)?,
        )
    } else {
        let [item] = items.try_into().map_err(|_| persistence())?;
        WorkflowInputBinding::try_single(item).map_err(WorkflowApplicationError::from)?
    };
    Ok((target, value))
}

fn decode_item(
    item: InputItemPayload,
) -> Result<WorkflowInputItemEntity, WorkflowApplicationError> {
    Ok(WorkflowInputItemEntity {
        id: WorkflowInputItemId::from_uuid(item.input_item_id).ok_or_else(persistence)?,
        source_node_id: WorkflowNodeId::from_uuid(item.source_node_id)?,
        source_output_key: NodeCapabilityOutputKey::new(item.source_output_key)
            .map_err(|_| persistence())?,
        input_role_key: item
            .input_role_key
            .map(NodeCapabilityInputRoleKey::new)
            .transpose()
            .map_err(|_| persistence())?,
    })
}
