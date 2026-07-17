use sha2::{Digest, Sha256};

use crate::workflow_graph::WorkflowAggregate;

use super::{
    WorkflowApplicationError, WorkflowCreateCommandHash, WorkflowCreateReceipt,
    WorkflowCreateRequestId,
};

impl WorkflowCreateReceipt {
    /// Captures exact creation replay evidence and its integrity fingerprint.
    pub fn new(
        request_id: WorkflowCreateRequestId,
        command_hash: WorkflowCreateCommandHash,
        created_workflow: WorkflowAggregate,
    ) -> Result<Self, WorkflowApplicationError> {
        if !created_workflow.nodes().is_empty() || !created_workflow.input_bindings().is_empty() {
            return Err(WorkflowApplicationError::WorkflowPersistenceFailure);
        }
        let result_fingerprint = fingerprint_created_workflow(&created_workflow);
        Ok(Self { request_id, command_hash, created_workflow, result_fingerprint })
    }
    /// Restores only replay evidence whose snapshot fingerprint is exact.
    pub fn try_restore(
        request_id: WorkflowCreateRequestId,
        command_hash: WorkflowCreateCommandHash,
        created_workflow: WorkflowAggregate,
        result_fingerprint: [u8; 32],
    ) -> Result<Self, WorkflowApplicationError> {
        let receipt = Self::new(request_id, command_hash, created_workflow)?;
        if receipt.result_fingerprint != result_fingerprint {
            return Err(WorkflowApplicationError::WorkflowPersistenceFailure);
        }
        Ok(receipt)
    }
    /// Returns the stable request identity.
    #[must_use]
    pub const fn request_id(&self) -> WorkflowCreateRequestId {
        self.request_id
    }
    /// Returns the canonical command hash.
    #[must_use]
    pub const fn command_hash(&self) -> WorkflowCreateCommandHash {
        self.command_hash
    }
    /// Returns the exact created snapshot.
    #[must_use]
    pub const fn created_workflow(&self) -> &WorkflowAggregate {
        &self.created_workflow
    }
    /// Returns the exact result fingerprint bytes.
    #[must_use]
    pub const fn result_fingerprint(&self) -> [u8; 32] {
        self.result_fingerprint
    }
}

fn fingerprint_created_workflow(workflow: &WorkflowAggregate) -> [u8; 32] {
    let mut bytes = Vec::new();
    append_hash_bytes(&mut bytes, b"oh-my-dream/workflow-create-result/v1");
    bytes.extend_from_slice(&workflow.schema_version.get().to_be_bytes());
    bytes.extend_from_slice(workflow.id.as_uuid().as_bytes());
    bytes.extend_from_slice(workflow.project_id.as_uuid().as_bytes());
    bytes.extend_from_slice(&workflow.revision.get().to_be_bytes());
    bytes.extend_from_slice(&workflow.created_at.as_utc_milliseconds().to_be_bytes());
    bytes.extend_from_slice(&workflow.updated_at.as_utc_milliseconds().to_be_bytes());
    bytes.extend_from_slice(&0_u64.to_be_bytes());
    bytes.extend_from_slice(&0_u64.to_be_bytes());
    Sha256::digest(bytes).into()
}

fn append_hash_bytes(target: &mut Vec<u8>, value: &[u8]) {
    target.extend_from_slice(&(value.len() as u32).to_be_bytes());
    target.extend_from_slice(value);
}
