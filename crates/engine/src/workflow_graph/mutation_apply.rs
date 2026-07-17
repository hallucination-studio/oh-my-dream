//! Atomic application of the closed Workflow mutation action set.

use crate::node_capability::WorkflowNodeCapabilityRegistry;

use super::{
    WorkflowAggregate, WorkflowAggregateRestoreData, WorkflowApplyMutationCommand,
    WorkflowGraphError, WorkflowInputBinding, WorkflowInputTarget, WorkflowMutationAction,
    WorkflowNodeEntity, WorkflowNodeId, WorkflowOrderedInputItems, WorkflowUpdatedAt,
};

impl WorkflowAggregate {
    /// Applies every ordered action to one candidate and returns only a fully valid next snapshot.
    pub fn apply_mutation_command(
        &self,
        command: &WorkflowApplyMutationCommand,
        observed_at: WorkflowUpdatedAt,
        capabilities: &WorkflowNodeCapabilityRegistry,
    ) -> Result<Self, WorkflowGraphError> {
        if command.workflow_id() != self.id {
            return Err(WorkflowGraphError::ReferenceViolation);
        }
        if command.base_revision() != self.revision {
            return Err(WorkflowGraphError::RevisionConflict);
        }
        let next_revision = self.revision.checked_next()?;
        let next_updated_at = self.updated_at.next_from_observation(observed_at)?;
        let mut candidate = self.clone();
        for action in command.actions() {
            apply_action(&mut candidate, action)?;
        }
        WorkflowAggregate::try_restore(
            WorkflowAggregateRestoreData {
                schema_version: candidate.schema_version,
                id: candidate.id,
                project_id: candidate.project_id,
                revision: next_revision,
                created_at: candidate.created_at,
                updated_at: next_updated_at,
                nodes: candidate.nodes.into_values().collect(),
                input_bindings: candidate.input_bindings.into_iter().collect(),
            },
            capabilities,
        )
    }
}

fn apply_action(
    candidate: &mut WorkflowAggregate,
    action: &WorkflowMutationAction,
) -> Result<(), WorkflowGraphError> {
    match action {
        WorkflowMutationAction::AddNode(action) => add_node(candidate, action),
        WorkflowMutationAction::RemoveNode(action) => remove_node(candidate, action.node_id),
        WorkflowMutationAction::ReplaceNodeParameters(action) => {
            require_node_mut(candidate, action.node_id)?.parameter_set =
                action.parameter_set.clone();
            Ok(())
        }
        WorkflowMutationAction::SelectNodeCapability(action) => {
            let node = require_node_mut(candidate, action.node_id)?;
            node.capability_contract = action.capability_contract.clone();
            node.parameter_set = action.parameter_set.clone();
            Ok(())
        }
        WorkflowMutationAction::MoveNode(action) => {
            require_node_mut(candidate, action.node_id)?.canvas_position = action.canvas_position;
            Ok(())
        }
        WorkflowMutationAction::BindSingleInput(action) => {
            if candidate.input_bindings.contains_key(&action.target) {
                return Err(WorkflowGraphError::InputOccupied);
            }
            let binding = WorkflowInputBinding::try_single(action.new_item.clone())?;
            candidate.input_bindings.insert(action.target.clone(), binding);
            Ok(())
        }
        WorkflowMutationAction::InsertReferenceItem(action) => insert_reference_item(
            candidate,
            &action.target,
            action.new_item.clone(),
            action.insertion_index,
        ),
        WorkflowMutationAction::MoveReferenceItem(action) => move_reference_item(
            candidate,
            &action.target,
            action.input_item_id,
            action.insertion_index_after_removal,
        ),
        WorkflowMutationAction::RemoveInputItem(action) => {
            remove_input_item(candidate, &action.target, action.input_item_id)
        }
        WorkflowMutationAction::SetInputItemRole(action) => set_input_item_role(candidate, action),
    }
}

fn add_node(
    candidate: &mut WorkflowAggregate,
    action: &super::WorkflowAddNodeAction,
) -> Result<(), WorkflowGraphError> {
    let node = WorkflowNodeEntity {
        id: action.new_node_id,
        capability_contract: action.capability_contract.clone(),
        parameter_set: action.parameter_set.clone(),
        canvas_position: action.canvas_position,
    };
    if candidate.nodes.insert(node.id, node).is_some() {
        return Err(WorkflowGraphError::DuplicateNode);
    }
    Ok(())
}

fn remove_node(
    candidate: &mut WorkflowAggregate,
    node_id: WorkflowNodeId,
) -> Result<(), WorkflowGraphError> {
    if candidate.nodes.remove(&node_id).is_none() {
        return Err(WorkflowGraphError::NodeNotFound);
    }
    candidate.input_bindings.retain(|target, _| target.node_id != node_id);
    let targets = candidate.input_bindings.keys().cloned().collect::<Vec<_>>();
    for target in targets {
        remove_items_from_source(candidate, &target, node_id)?;
    }
    Ok(())
}

fn remove_items_from_source(
    candidate: &mut WorkflowAggregate,
    target: &WorkflowInputTarget,
    source_node_id: WorkflowNodeId,
) -> Result<(), WorkflowGraphError> {
    let Some(binding) = candidate.input_bindings.get(target) else {
        return Ok(());
    };
    let remaining = binding
        .items()
        .filter(|item| item.source_node_id != source_node_id)
        .cloned()
        .collect::<Vec<_>>();
    if remaining.len() == binding.items().count() {
        return Ok(());
    }
    if remaining.is_empty() {
        candidate.input_bindings.remove(target);
    } else {
        candidate.input_bindings.insert(
            target.clone(),
            WorkflowInputBinding::ordered_references(WorkflowOrderedInputItems::try_new(
                remaining,
            )?),
        );
    }
    Ok(())
}

fn insert_reference_item(
    candidate: &mut WorkflowAggregate,
    target: &WorkflowInputTarget,
    item: super::WorkflowInputItemEntity,
    insertion_index: u32,
) -> Result<(), WorkflowGraphError> {
    let mut items = match candidate.input_bindings.get(target) {
        Some(WorkflowInputBinding::OrderedReferences { items }) => items.as_slice().to_vec(),
        Some(WorkflowInputBinding::Single { .. }) => {
            return Err(WorkflowGraphError::BindingShapeMismatch);
        }
        None => Vec::new(),
    };
    let index =
        usize::try_from(insertion_index).map_err(|_| WorkflowGraphError::CardinalityViolation)?;
    if index > items.len() {
        return Err(WorkflowGraphError::CardinalityViolation);
    }
    items.insert(index, item);
    candidate.input_bindings.insert(
        target.clone(),
        WorkflowInputBinding::ordered_references(WorkflowOrderedInputItems::try_new(items)?),
    );
    Ok(())
}

fn move_reference_item(
    candidate: &mut WorkflowAggregate,
    target: &WorkflowInputTarget,
    item_id: crate::node_capability::WorkflowInputItemId,
    insertion_index: u32,
) -> Result<(), WorkflowGraphError> {
    let mut items = ordered_items(candidate, target)?.to_vec();
    let old_index = items
        .iter()
        .position(|item| item.id == item_id)
        .ok_or(WorkflowGraphError::InputItemNotFound)?;
    let item = items.remove(old_index);
    let new_index =
        usize::try_from(insertion_index).map_err(|_| WorkflowGraphError::CardinalityViolation)?;
    if new_index > items.len() {
        return Err(WorkflowGraphError::CardinalityViolation);
    }
    items.insert(new_index, item);
    candidate.input_bindings.insert(
        target.clone(),
        WorkflowInputBinding::ordered_references(WorkflowOrderedInputItems::try_new(items)?),
    );
    Ok(())
}

fn remove_input_item(
    candidate: &mut WorkflowAggregate,
    target: &WorkflowInputTarget,
    item_id: crate::node_capability::WorkflowInputItemId,
) -> Result<(), WorkflowGraphError> {
    let binding = candidate.input_bindings.get(target).ok_or(WorkflowGraphError::InputNotFound)?;
    let mut items = binding.items().cloned().collect::<Vec<_>>();
    let index = items
        .iter()
        .position(|item| item.id == item_id)
        .ok_or(WorkflowGraphError::InputItemNotFound)?;
    items.remove(index);
    if items.is_empty() {
        candidate.input_bindings.remove(target);
    } else {
        candidate.input_bindings.insert(
            target.clone(),
            WorkflowInputBinding::ordered_references(WorkflowOrderedInputItems::try_new(items)?),
        );
    }
    Ok(())
}

fn set_input_item_role(
    candidate: &mut WorkflowAggregate,
    action: &super::WorkflowSetInputItemRoleAction,
) -> Result<(), WorkflowGraphError> {
    let binding = candidate
        .input_bindings
        .get_mut(&action.target)
        .ok_or(WorkflowGraphError::InputNotFound)?;
    let WorkflowInputBinding::OrderedReferences { items } = binding else {
        return Err(WorkflowGraphError::BindingShapeMismatch);
    };
    let item = items
        .as_mut_slice()
        .iter_mut()
        .find(|item| item.id == action.input_item_id)
        .ok_or(WorkflowGraphError::InputItemNotFound)?;
    item.input_role_key = Some(action.input_role_key.clone());
    Ok(())
}

fn ordered_items<'a>(
    candidate: &'a WorkflowAggregate,
    target: &WorkflowInputTarget,
) -> Result<&'a [super::WorkflowInputItemEntity], WorkflowGraphError> {
    match candidate.input_bindings.get(target) {
        Some(WorkflowInputBinding::OrderedReferences { items }) => Ok(items.as_slice()),
        Some(WorkflowInputBinding::Single { .. }) => Err(WorkflowGraphError::BindingShapeMismatch),
        None => Err(WorkflowGraphError::InputNotFound),
    }
}

fn require_node_mut(
    candidate: &mut WorkflowAggregate,
    node_id: WorkflowNodeId,
) -> Result<&mut WorkflowNodeEntity, WorkflowGraphError> {
    candidate.nodes.get_mut(&node_id).ok_or(WorkflowGraphError::NodeNotFound)
}
