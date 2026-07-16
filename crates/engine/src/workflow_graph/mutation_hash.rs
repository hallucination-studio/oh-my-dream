//! Frozen canonical Workflow mutation command hashing.

use sha2::{Digest, Sha256};

use super::{
    WorkflowId, WorkflowInputItemEntity, WorkflowInputTarget, WorkflowMutationAction,
    WorkflowRevision,
};

/// SHA-256 identity of canonical mutation command content.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct WorkflowMutationCommandHash([u8; 32]);

impl WorkflowMutationCommandHash {
    pub(super) fn compute(
        workflow_id: WorkflowId,
        base_revision: WorkflowRevision,
        actions: &[WorkflowMutationAction],
    ) -> Self {
        let mut bytes = Vec::new();
        append_bytes(&mut bytes, b"oh-my-dream/workflow-mutation/v1");
        bytes.extend_from_slice(workflow_id.as_uuid().as_bytes());
        bytes.extend_from_slice(&base_revision.get().to_be_bytes());
        append_u32(&mut bytes, actions.len() as u32);
        for action in actions {
            append_action(&mut bytes, action);
        }
        Self(Sha256::digest(bytes).into())
    }

    /// Restores exact persisted SHA-256 bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Returns the exact digest bytes without selecting a text encoding.
    #[must_use]
    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

fn append_action(bytes: &mut Vec<u8>, action: &WorkflowMutationAction) {
    match action {
        WorkflowMutationAction::AddNode(value) => {
            bytes.push(0);
            append_node_id(bytes, value.new_node_id);
            append_contract_ref(bytes, &value.capability_contract);
            append_parameters(bytes, &value.parameter_set);
            append_position(bytes, value.canvas_position);
        }
        WorkflowMutationAction::RemoveNode(value) => {
            bytes.push(1);
            append_node_id(bytes, value.node_id);
        }
        WorkflowMutationAction::ReplaceNodeParameters(value) => {
            bytes.push(2);
            append_node_id(bytes, value.node_id);
            append_parameters(bytes, &value.parameter_set);
        }
        WorkflowMutationAction::SelectNodeCapability(value) => {
            bytes.push(3);
            append_node_id(bytes, value.node_id);
            append_contract_ref(bytes, &value.capability_contract);
            append_parameters(bytes, &value.parameter_set);
        }
        WorkflowMutationAction::MoveNode(value) => {
            bytes.push(4);
            append_node_id(bytes, value.node_id);
            append_position(bytes, value.canvas_position);
        }
        WorkflowMutationAction::BindSingleInput(value) => {
            bytes.push(5);
            append_target(bytes, &value.target);
            append_item(bytes, &value.new_item);
        }
        WorkflowMutationAction::InsertReferenceItem(value) => {
            bytes.push(6);
            append_target(bytes, &value.target);
            append_item(bytes, &value.new_item);
            append_u32(bytes, value.insertion_index);
        }
        WorkflowMutationAction::MoveReferenceItem(value) => {
            bytes.push(7);
            append_target(bytes, &value.target);
            bytes.extend_from_slice(value.input_item_id.as_uuid().as_bytes());
            append_u32(bytes, value.insertion_index_after_removal);
        }
        WorkflowMutationAction::RemoveInputItem(value) => {
            bytes.push(8);
            append_target(bytes, &value.target);
            bytes.extend_from_slice(value.input_item_id.as_uuid().as_bytes());
        }
        WorkflowMutationAction::SetInputItemRole(value) => {
            bytes.push(9);
            append_target(bytes, &value.target);
            bytes.extend_from_slice(value.input_item_id.as_uuid().as_bytes());
            append_bytes(bytes, value.input_role_key.as_str().as_bytes());
        }
    }
}

fn append_contract_ref(
    bytes: &mut Vec<u8>,
    value: &crate::node_capability::NodeCapabilityContractRef,
) {
    append_bytes(bytes, value.id().as_str().as_bytes());
    bytes.extend_from_slice(&value.version().major().to_be_bytes());
    bytes.extend_from_slice(&value.version().minor().to_be_bytes());
}

fn append_parameters(
    bytes: &mut Vec<u8>,
    value: &crate::node_capability::NodeCapabilityParameterSet,
) {
    append_bytes(bytes, &value.canonical_bytes());
}

fn append_position(bytes: &mut Vec<u8>, value: super::WorkflowCanvasPosition) {
    bytes.extend_from_slice(&value.x().to_bits().to_be_bytes());
    bytes.extend_from_slice(&value.y().to_bits().to_be_bytes());
}

fn append_target(bytes: &mut Vec<u8>, value: &WorkflowInputTarget) {
    append_node_id(bytes, value.node_id);
    append_bytes(bytes, value.input_key.as_str().as_bytes());
}

fn append_item(bytes: &mut Vec<u8>, value: &WorkflowInputItemEntity) {
    bytes.extend_from_slice(value.id.as_uuid().as_bytes());
    append_node_id(bytes, value.source_node_id);
    append_bytes(bytes, value.source_output_key.as_str().as_bytes());
    if let Some(role) = &value.input_role_key {
        append_bytes(bytes, role.as_str().as_bytes());
    }
}

fn append_node_id(bytes: &mut Vec<u8>, value: super::WorkflowNodeId) {
    bytes.extend_from_slice(value.as_uuid().as_bytes());
}

fn append_bytes(target: &mut Vec<u8>, value: &[u8]) {
    append_u32(target, value.len() as u32);
    target.extend_from_slice(value);
}

fn append_u32(target: &mut Vec<u8>, value: u32) {
    target.extend_from_slice(&value.to_be_bytes());
}
