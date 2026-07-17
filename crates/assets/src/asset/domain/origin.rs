//! Imported and exact Workflow-node Asset provenance.

use super::{
    AssetDomainError, AssetImportId, AssetOriginGenerationProfileRef,
    AssetOriginNodeCapabilityContractRef, AssetOriginNodeOutputKey, AssetOriginSourceAssetIds,
    AssetOriginWorkflowId, AssetOriginWorkflowNodeExecutionId, AssetOriginWorkflowNodeId,
    AssetOriginWorkflowRevision, AssetOriginWorkflowRunId, AssetOriginalFileName,
};

/// Exact Workflow producer coordinates translated into Asset-owned values.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct AssetWorkflowNodeOrigin {
    workflow_id: AssetOriginWorkflowId,
    workflow_revision: AssetOriginWorkflowRevision,
    workflow_run_id: AssetOriginWorkflowRunId,
    workflow_node_id: AssetOriginWorkflowNodeId,
    node_execution_id: AssetOriginWorkflowNodeExecutionId,
    capability_contract_ref: AssetOriginNodeCapabilityContractRef,
}

impl AssetWorkflowNodeOrigin {
    /// Combines the exact already validated Workflow producer coordinates.
    #[must_use]
    pub const fn new(
        workflow_id: AssetOriginWorkflowId,
        workflow_revision: AssetOriginWorkflowRevision,
        workflow_run_id: AssetOriginWorkflowRunId,
        workflow_node_id: AssetOriginWorkflowNodeId,
        node_execution_id: AssetOriginWorkflowNodeExecutionId,
        capability_contract_ref: AssetOriginNodeCapabilityContractRef,
    ) -> Self {
        Self {
            workflow_id,
            workflow_revision,
            workflow_run_id,
            workflow_node_id,
            node_execution_id,
            capability_contract_ref,
        }
    }

    /// Returns the translated Workflow identity.
    #[must_use]
    pub const fn workflow_id(&self) -> AssetOriginWorkflowId {
        self.workflow_id
    }
    /// Returns the translated Workflow revision.
    #[must_use]
    pub const fn workflow_revision(&self) -> AssetOriginWorkflowRevision {
        self.workflow_revision
    }
    /// Returns the translated Run identity.
    #[must_use]
    pub const fn workflow_run_id(&self) -> AssetOriginWorkflowRunId {
        self.workflow_run_id
    }
    /// Returns the translated Workflow node identity.
    #[must_use]
    pub const fn workflow_node_id(&self) -> AssetOriginWorkflowNodeId {
        self.workflow_node_id
    }
    /// Returns the translated node-execution identity.
    #[must_use]
    pub const fn node_execution_id(&self) -> AssetOriginWorkflowNodeExecutionId {
        self.node_execution_id
    }
    /// Returns the translated exact capability contract reference.
    #[must_use]
    pub const fn capability_contract_ref(&self) -> &AssetOriginNodeCapabilityContractRef {
        &self.capability_contract_ref
    }
}

/// Durable idempotency identity of one node-produced media output slot.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct AssetNodeOutputKey {
    workflow_run_id: AssetOriginWorkflowRunId,
    node_execution_id: AssetOriginWorkflowNodeExecutionId,
    output_key: AssetOriginNodeOutputKey,
    ordinal: u32,
}

impl AssetNodeOutputKey {
    /// Creates one exact output slot key.
    #[must_use]
    pub const fn new(
        workflow_run_id: AssetOriginWorkflowRunId,
        node_execution_id: AssetOriginWorkflowNodeExecutionId,
        output_key: AssetOriginNodeOutputKey,
        ordinal: u32,
    ) -> Self {
        Self { workflow_run_id, node_execution_id, output_key, ordinal }
    }

    /// Returns the Run identity.
    #[must_use]
    pub const fn workflow_run_id(&self) -> AssetOriginWorkflowRunId {
        self.workflow_run_id
    }
    /// Returns the node-execution identity.
    #[must_use]
    pub const fn node_execution_id(&self) -> AssetOriginWorkflowNodeExecutionId {
        self.node_execution_id
    }
    /// Returns the exact output key.
    #[must_use]
    pub const fn output_key(&self) -> &AssetOriginNodeOutputKey {
        &self.output_key
    }
    /// Returns the output ordinal.
    #[must_use]
    pub const fn ordinal(&self) -> u32 {
        self.ordinal
    }
}

/// Exact semantic production source for one node-produced Asset.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum AssetNodeOutputProduction {
    /// Provider generated without source Assets.
    ProviderGenerated {
        /// Provider-independent profile selection.
        generation_profile_ref: AssetOriginGenerationProfileRef,
    },
    /// Deterministically derived from an ordered non-empty source set.
    DeterministicDerived {
        /// Source Assets in supplied provenance order.
        source_asset_ids: AssetOriginSourceAssetIds,
    },
    /// Provider derived from source Assets using one profile.
    ProviderDerived {
        /// Source Assets in supplied provenance order.
        source_asset_ids: AssetOriginSourceAssetIds,
        /// Provider-independent profile selection.
        generation_profile_ref: AssetOriginGenerationProfileRef,
    },
}

/// Validated imported origin fields.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct AssetImportedOrigin {
    import_id: AssetImportId,
    original_file_name: AssetOriginalFileName,
}

/// Validated Workflow-node output origin fields.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct AssetWorkflowNodeOutputOrigin {
    producer: AssetWorkflowNodeOrigin,
    production: AssetNodeOutputProduction,
    output_key: AssetNodeOutputKey,
}

impl AssetImportedOrigin {
    /// Returns the import idempotency identity.
    #[must_use]
    pub const fn import_id(&self) -> AssetImportId {
        self.import_id
    }
    /// Returns the final original file name without a path.
    #[must_use]
    pub const fn original_file_name(&self) -> &AssetOriginalFileName {
        &self.original_file_name
    }
}

impl AssetWorkflowNodeOutputOrigin {
    /// Returns exact translated producer coordinates.
    #[must_use]
    pub const fn producer(&self) -> &AssetWorkflowNodeOrigin {
        &self.producer
    }
    /// Returns provider/deterministic production semantics.
    #[must_use]
    pub const fn production(&self) -> &AssetNodeOutputProduction {
        &self.production
    }
    /// Returns the durable output-slot idempotency key.
    #[must_use]
    pub const fn output_key(&self) -> &AssetNodeOutputKey {
        &self.output_key
    }
}

/// Closed provenance of one logical Asset.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum AssetOrigin {
    /// Trusted user import without a source path.
    Imported(AssetImportedOrigin),
    /// Exact Workflow-node output coordinates and production semantics.
    WorkflowNodeOutput(AssetWorkflowNodeOutputOrigin),
}

impl AssetOrigin {
    /// Creates imported provenance from an ID and final file name only.
    #[must_use]
    pub const fn imported(
        import_id: AssetImportId,
        original_file_name: AssetOriginalFileName,
    ) -> Self {
        Self::Imported(AssetImportedOrigin { import_id, original_file_name })
    }

    /// Creates node-output provenance only when producer and output slot coordinates agree.
    pub fn workflow_node_output(
        producer: AssetWorkflowNodeOrigin,
        production: AssetNodeOutputProduction,
        output_key: AssetNodeOutputKey,
    ) -> Result<Self, AssetDomainError> {
        if producer.workflow_run_id != output_key.workflow_run_id
            || producer.node_execution_id != output_key.node_execution_id
        {
            return Err(AssetDomainError::InvalidOrigin);
        }
        Ok(Self::WorkflowNodeOutput(AssetWorkflowNodeOutputOrigin {
            producer,
            production,
            output_key,
        }))
    }
}
