//! Closed Workflow graph mutation command values.

use crate::node_capability::{
    NodeCapabilityContractRef, NodeCapabilityInputRoleKey, NodeCapabilityParameterSet,
    WorkflowInputItemId,
};

use super::{
    WorkflowCanvasPosition, WorkflowGraphConstructionError, WorkflowId, WorkflowInputItemEntity,
    WorkflowInputTarget, WorkflowMutationCommandHash, WorkflowMutationRequestId, WorkflowNodeId,
    WorkflowRevision,
};

/// Adds one node with an exact capability and complete parameters.
#[derive(Clone, Debug, PartialEq)]
pub struct WorkflowAddNodeAction {
    /// New Workflow-local node identity.
    pub new_node_id: WorkflowNodeId,
    /// Exact selected capability contract.
    pub capability_contract: NodeCapabilityContractRef,
    /// Complete replacement parameter set.
    pub parameter_set: NodeCapabilityParameterSet,
    /// Initial persisted canvas position.
    pub canvas_position: WorkflowCanvasPosition,
}

/// Removes one existing node and all incident bindings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowRemoveNodeAction {
    /// Existing node identity.
    pub node_id: WorkflowNodeId,
}

/// Replaces all parameters on one existing node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowReplaceNodeParametersAction {
    /// Existing node identity.
    pub node_id: WorkflowNodeId,
    /// Complete replacement parameter set.
    pub parameter_set: NodeCapabilityParameterSet,
}

/// Selects an exact replacement capability and complete parameters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowSelectNodeCapabilityAction {
    /// Existing node identity.
    pub node_id: WorkflowNodeId,
    /// Exact replacement contract.
    pub capability_contract: NodeCapabilityContractRef,
    /// Complete replacement parameter set.
    pub parameter_set: NodeCapabilityParameterSet,
}

/// Replaces one node's persisted canvas position.
#[derive(Clone, Debug, PartialEq)]
pub struct WorkflowMoveNodeAction {
    /// Existing node identity.
    pub node_id: WorkflowNodeId,
    /// Replacement position.
    pub canvas_position: WorkflowCanvasPosition,
}

/// Binds one unoccupied single-value input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowBindSingleInputAction {
    /// Exact target node and input.
    pub target: WorkflowInputTarget,
    /// New stable role-free edge item.
    pub new_item: WorkflowInputItemEntity,
}

/// Inserts one role-bearing item into an ordered-reference binding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowInsertReferenceItemAction {
    /// Exact target node and input.
    pub target: WorkflowInputTarget,
    /// New stable role-bearing edge item.
    pub new_item: WorkflowInputItemEntity,
    /// Insertion index in `0..=len`.
    pub insertion_index: u32,
}

/// Reorders one existing stable reference item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowMoveReferenceItemAction {
    /// Exact ordered-reference target.
    pub target: WorkflowInputTarget,
    /// Existing stable item identity.
    pub input_item_id: WorkflowInputItemId,
    /// Index after first removing the item.
    pub insertion_index_after_removal: u32,
}

/// Removes one exact stable input item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowRemoveInputItemAction {
    /// Exact target binding.
    pub target: WorkflowInputTarget,
    /// Existing stable item identity.
    pub input_item_id: WorkflowInputItemId,
}

/// Replaces the role of one ordered-reference item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowSetInputItemRoleAction {
    /// Exact ordered-reference target.
    pub target: WorkflowInputTarget,
    /// Existing stable item identity.
    pub input_item_id: WorkflowInputItemId,
    /// Replacement capability-owned role.
    pub input_role_key: NodeCapabilityInputRoleKey,
}

/// Closed ten-action Workflow mutation union in frozen tag order.
#[derive(Clone, Debug, PartialEq)]
pub enum WorkflowMutationAction {
    /// Tag 0: add node.
    AddNode(WorkflowAddNodeAction),
    /// Tag 1: remove node.
    RemoveNode(WorkflowRemoveNodeAction),
    /// Tag 2: replace node parameters.
    ReplaceNodeParameters(WorkflowReplaceNodeParametersAction),
    /// Tag 3: select exact node capability.
    SelectNodeCapability(WorkflowSelectNodeCapabilityAction),
    /// Tag 4: move node.
    MoveNode(WorkflowMoveNodeAction),
    /// Tag 5: bind single input.
    BindSingleInput(WorkflowBindSingleInputAction),
    /// Tag 6: insert ordered reference item.
    InsertReferenceItem(WorkflowInsertReferenceItemAction),
    /// Tag 7: move ordered reference item.
    MoveReferenceItem(WorkflowMoveReferenceItemAction),
    /// Tag 8: remove input item.
    RemoveInputItem(WorkflowRemoveInputItemAction),
    /// Tag 9: set ordered item role.
    SetInputItemRole(WorkflowSetInputItemRoleAction),
}

/// One bounded ordered all-or-nothing Workflow mutation command.
#[derive(Clone, Debug, PartialEq)]
pub struct WorkflowApplyMutationCommand {
    request_id: WorkflowMutationRequestId,
    workflow_id: WorkflowId,
    base_revision: WorkflowRevision,
    actions: Vec<WorkflowMutationAction>,
    command_hash: WorkflowMutationCommandHash,
}

impl WorkflowApplyMutationCommand {
    /// Builds a command only for a non-empty list of at most 128 locally valid actions.
    pub fn try_new(
        request_id: WorkflowMutationRequestId,
        workflow_id: WorkflowId,
        base_revision: WorkflowRevision,
        actions: Vec<WorkflowMutationAction>,
    ) -> Result<Self, WorkflowGraphConstructionError> {
        if actions.is_empty() || actions.len() > 128 {
            return Err(WorkflowGraphConstructionError::ActionLimitExceeded);
        }
        validate_action_item_roles(&actions)?;
        let command_hash =
            WorkflowMutationCommandHash::compute(workflow_id, base_revision, &actions);
        Ok(Self { request_id, workflow_id, base_revision, actions, command_hash })
    }

    /// Returns the idempotency request identity.
    #[must_use]
    pub const fn request_id(&self) -> WorkflowMutationRequestId {
        self.request_id
    }
    /// Returns the target Workflow identity.
    #[must_use]
    pub const fn workflow_id(&self) -> WorkflowId {
        self.workflow_id
    }
    /// Returns the required current revision.
    #[must_use]
    pub const fn base_revision(&self) -> WorkflowRevision {
        self.base_revision
    }
    /// Returns actions in semantic application order.
    #[must_use]
    pub fn actions(&self) -> &[WorkflowMutationAction] {
        &self.actions
    }
    /// Returns the canonical content hash excluding request identity.
    #[must_use]
    pub const fn command_hash(&self) -> WorkflowMutationCommandHash {
        self.command_hash
    }
}

fn validate_action_item_roles(
    actions: &[WorkflowMutationAction],
) -> Result<(), WorkflowGraphConstructionError> {
    for action in actions {
        match action {
            WorkflowMutationAction::BindSingleInput(action)
                if action.new_item.input_role_key.is_some() =>
            {
                return Err(WorkflowGraphConstructionError::BindingShapeMismatch);
            }
            WorkflowMutationAction::InsertReferenceItem(action)
                if action.new_item.input_role_key.is_none() =>
            {
                return Err(WorkflowGraphConstructionError::BindingShapeMismatch);
            }
            _ => {}
        }
    }
    Ok(())
}
