//! Exact committed Workflow mutation result and replay integrity evidence.

use sha2::{Digest, Sha256};

use super::{
    WorkflowAggregate, WorkflowApplyMutationCommand, WorkflowGraphError, WorkflowInputBinding,
    WorkflowInputItemEntity, WorkflowMutationCommandHash, WorkflowMutationRequestId,
};

/// SHA-256 fingerprint of one exact committed Workflow snapshot.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct WorkflowMutationResultFingerprint([u8; 32]);

impl WorkflowMutationResultFingerprint {
    /// Returns exact fingerprint bytes without choosing a text encoding.
    #[must_use]
    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Durable evidence for exact idempotent Workflow mutation replay.
#[derive(Clone, Debug, PartialEq)]
pub struct WorkflowMutationReceipt {
    request_id: WorkflowMutationRequestId,
    command_hash: WorkflowMutationCommandHash,
    committed_workflow: WorkflowAggregate,
    result_fingerprint: WorkflowMutationResultFingerprint,
}

impl WorkflowMutationReceipt {
    /// Captures the exact committed snapshot and computes its integrity fingerprint.
    #[must_use]
    pub fn new(
        command: &WorkflowApplyMutationCommand,
        committed_workflow: WorkflowAggregate,
    ) -> Self {
        let result_fingerprint = fingerprint_workflow(&committed_workflow);
        Self {
            request_id: command.request_id(),
            command_hash: command.command_hash(),
            committed_workflow,
            result_fingerprint,
        }
    }

    /// Restores a receipt only when its persisted snapshot fingerprint is exact.
    pub fn try_restore(
        request_id: WorkflowMutationRequestId,
        command_hash: WorkflowMutationCommandHash,
        committed_workflow: WorkflowAggregate,
        result_fingerprint_bytes: [u8; 32],
    ) -> Result<Self, WorkflowGraphError> {
        let result_fingerprint = WorkflowMutationResultFingerprint(result_fingerprint_bytes);
        if fingerprint_workflow(&committed_workflow) != result_fingerprint {
            return Err(WorkflowGraphError::PersistenceFailure);
        }
        Ok(Self { request_id, command_hash, committed_workflow, result_fingerprint })
    }

    /// Returns the prior snapshot only for the exact same request and command content.
    pub fn replay_matching_command(
        &self,
        command: &WorkflowApplyMutationCommand,
    ) -> Result<&WorkflowAggregate, WorkflowGraphError> {
        if self.request_id != command.request_id() || self.command_hash != command.command_hash() {
            return Err(WorkflowGraphError::MutationIdempotencyConflict);
        }
        Ok(&self.committed_workflow)
    }

    /// Returns the idempotency request identity.
    #[must_use]
    pub const fn request_id(&self) -> WorkflowMutationRequestId {
        self.request_id
    }

    /// Returns the canonical command content hash.
    #[must_use]
    pub const fn command_hash(&self) -> WorkflowMutationCommandHash {
        self.command_hash
    }

    /// Returns the exact committed snapshot retained for replay.
    #[must_use]
    pub const fn committed_workflow(&self) -> &WorkflowAggregate {
        &self.committed_workflow
    }

    /// Returns the committed snapshot integrity fingerprint.
    #[must_use]
    pub const fn result_fingerprint(&self) -> WorkflowMutationResultFingerprint {
        self.result_fingerprint
    }
}

impl WorkflowAggregate {
    /// Returns the frozen canonical Workflow snapshot fingerprint.
    #[must_use]
    pub fn canonical_fingerprint(&self) -> [u8; 32] {
        fingerprint_workflow(self).as_bytes()
    }

    /// Returns a timestamp- and revision-independent canonical graph fingerprint.
    #[must_use]
    pub fn canonical_graph_fingerprint(&self) -> [u8; 32] {
        let mut bytes = Vec::new();
        append_bytes(&mut bytes, b"oh-my-dream/workflow-graph-result/v1");
        bytes.extend_from_slice(&self.schema_version.get().to_be_bytes());
        append_graph(&mut bytes, self);
        Sha256::digest(bytes).into()
    }
}

pub(crate) fn fingerprint_workflow(
    workflow: &WorkflowAggregate,
) -> WorkflowMutationResultFingerprint {
    let mut bytes = Vec::new();
    append_bytes(&mut bytes, b"oh-my-dream/workflow-mutation-result/v1");
    bytes.extend_from_slice(&workflow.schema_version.get().to_be_bytes());
    bytes.extend_from_slice(workflow.id.as_uuid().as_bytes());
    bytes.extend_from_slice(workflow.project_id.as_uuid().as_bytes());
    bytes.extend_from_slice(&workflow.revision.get().to_be_bytes());
    bytes.extend_from_slice(&workflow.created_at.as_utc_milliseconds().to_be_bytes());
    bytes.extend_from_slice(&workflow.updated_at.as_utc_milliseconds().to_be_bytes());
    append_graph(&mut bytes, workflow);
    WorkflowMutationResultFingerprint(Sha256::digest(bytes).into())
}

fn append_graph(bytes: &mut Vec<u8>, workflow: &WorkflowAggregate) {
    append_u64(bytes, workflow.nodes().len() as u64);
    for node in workflow.nodes().values() {
        bytes.extend_from_slice(node.id.as_uuid().as_bytes());
        append_contract_ref(bytes, &node.capability_contract);
        append_bytes(bytes, &node.parameter_set.canonical_bytes());
        bytes.extend_from_slice(&node.canvas_position.x().to_bits().to_be_bytes());
        bytes.extend_from_slice(&node.canvas_position.y().to_bits().to_be_bytes());
    }
    append_u64(bytes, workflow.input_bindings().len() as u64);
    for (target, binding) in workflow.input_bindings() {
        bytes.extend_from_slice(target.node_id.as_uuid().as_bytes());
        append_bytes(bytes, target.input_key.as_str().as_bytes());
        append_binding(bytes, binding);
    }
}

fn append_binding(bytes: &mut Vec<u8>, binding: &WorkflowInputBinding) {
    match binding {
        WorkflowInputBinding::Single { item } => {
            bytes.push(0);
            append_item(bytes, item);
        }
        WorkflowInputBinding::OrderedReferences { items } => {
            bytes.push(1);
            append_u64(bytes, items.as_slice().len() as u64);
            for item in items.as_slice() {
                append_item(bytes, item);
            }
        }
    }
}

fn append_item(bytes: &mut Vec<u8>, item: &WorkflowInputItemEntity) {
    bytes.extend_from_slice(item.id.as_uuid().as_bytes());
    bytes.extend_from_slice(item.source_node_id.as_uuid().as_bytes());
    append_bytes(bytes, item.source_output_key.as_str().as_bytes());
    match &item.input_role_key {
        Some(role) => {
            bytes.push(1);
            append_bytes(bytes, role.as_str().as_bytes());
        }
        None => bytes.push(0),
    }
}

fn append_contract_ref(
    bytes: &mut Vec<u8>,
    contract_ref: &crate::node_capability::NodeCapabilityContractRef,
) {
    append_bytes(bytes, contract_ref.id().as_str().as_bytes());
    bytes.extend_from_slice(&contract_ref.version().major().to_be_bytes());
    bytes.extend_from_slice(&contract_ref.version().minor().to_be_bytes());
}

fn append_bytes(target: &mut Vec<u8>, value: &[u8]) {
    target.extend_from_slice(&(value.len() as u32).to_be_bytes());
    target.extend_from_slice(value);
}

fn append_u64(target: &mut Vec<u8>, value: u64) {
    target.extend_from_slice(&value.to_be_bytes());
}
