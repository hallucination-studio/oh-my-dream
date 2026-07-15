//! Workflow nodes and first-class directed input items.

use crate::node_capability::{
    NodeCapabilityContractRef, NodeCapabilityInputKey, NodeCapabilityInputRoleKey,
    NodeCapabilityOutputKey, NodeCapabilityParameterSet, WorkflowInputItemId,
};

use super::{WorkflowCanvasPosition, WorkflowGraphConstructionError, WorkflowNodeId};

/// One node in the editable Workflow graph.
#[derive(Clone, Debug, PartialEq)]
pub struct WorkflowNodeEntity {
    /// Workflow-local node identity.
    pub id: WorkflowNodeId,
    /// Exact selected capability contract.
    pub capability_contract: NodeCapabilityContractRef,
    /// Complete opaque capability-owned parameters.
    pub parameter_set: NodeCapabilityParameterSet,
    /// Reopen-only canvas position.
    pub canvas_position: WorkflowCanvasPosition,
}

/// One exact node input receiving a binding.
#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct WorkflowInputTarget {
    /// Existing target node.
    pub node_id: WorkflowNodeId,
    /// Declared capability input.
    pub input_key: NodeCapabilityInputKey,
}

/// Stable first-class edge item whose target is owned by its binding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowInputItemEntity {
    /// Stable identity retained across reorder.
    pub id: WorkflowInputItemId,
    /// Existing source node.
    pub source_node_id: WorkflowNodeId,
    /// Declared source output.
    pub source_output_key: NodeCapabilityOutputKey,
    /// Capability-owned role for ordered references only.
    pub input_role_key: Option<NodeCapabilityInputRoleKey>,
}

/// Non-empty ordered sequence whose vector position is authoritative.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowOrderedInputItems(Vec<WorkflowInputItemEntity>);

impl WorkflowOrderedInputItems {
    /// Creates a non-empty role-bearing ordered sequence.
    pub fn try_new(
        items: Vec<WorkflowInputItemEntity>,
    ) -> Result<Self, WorkflowGraphConstructionError> {
        if items.is_empty() {
            return Err(WorkflowGraphConstructionError::CardinalityViolation);
        }
        if items.iter().any(|item| item.input_role_key.is_none()) {
            return Err(WorkflowGraphConstructionError::BindingShapeMismatch);
        }
        Ok(Self(items))
    }

    /// Returns items in their authoritative semantic order.
    #[must_use]
    pub fn as_slice(&self) -> &[WorkflowInputItemEntity] {
        &self.0
    }
}

/// Exact single or ordered-reference input binding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkflowInputBinding {
    /// One role-free item.
    Single {
        /// Bound directed edge item.
        item: WorkflowInputItemEntity,
    },
    /// Non-empty role-bearing ordered items.
    OrderedReferences {
        /// Items in semantic order.
        items: WorkflowOrderedInputItems,
    },
}

impl WorkflowInputBinding {
    /// Creates a single binding only for a role-free item.
    pub fn try_single(
        item: WorkflowInputItemEntity,
    ) -> Result<Self, WorkflowGraphConstructionError> {
        if item.input_role_key.is_some() {
            return Err(WorkflowGraphConstructionError::BindingShapeMismatch);
        }
        Ok(Self::Single { item })
    }

    /// Creates an ordered-reference binding from a validated non-empty sequence.
    #[must_use]
    pub const fn ordered_references(items: WorkflowOrderedInputItems) -> Self {
        Self::OrderedReferences { items }
    }

    /// Iterates items in semantic order.
    pub fn items(&self) -> impl Iterator<Item = &WorkflowInputItemEntity> {
        match self {
            Self::Single { item } => std::slice::from_ref(item).iter(),
            Self::OrderedReferences { items } => items.as_slice().iter(),
        }
    }
}
