//! Pure Workflow aggregate reconstruction and graph validation.

use std::collections::{BTreeMap, BTreeSet};

use projects::project::domain::ProjectId;

use crate::node_capability::{
    NodeCapabilityContract, NodeCapabilityInputBindingContract, WorkflowNodeCapabilityRegistry,
};

use super::{
    WorkflowCreatedAt, WorkflowGraphConstructionError, WorkflowId, WorkflowInputBinding,
    WorkflowInputItemEntity, WorkflowInputTarget, WorkflowNodeEntity, WorkflowNodeId,
    WorkflowRevision, WorkflowSchemaVersion, WorkflowUpdatedAt,
};

/// Authoritative editable Workflow graph aggregate.
#[derive(Clone, Debug, PartialEq)]
pub struct WorkflowAggregate {
    /// Persisted hard-cut schema.
    pub schema_version: WorkflowSchemaVersion,
    /// Aggregate identity.
    pub id: WorkflowId,
    /// Owning Project identity.
    pub project_id: ProjectId,
    /// Optimistic-concurrency revision.
    pub revision: WorkflowRevision,
    /// Immutable creation time.
    pub created_at: WorkflowCreatedAt,
    /// Latest successful mutation time.
    pub updated_at: WorkflowUpdatedAt,
    nodes: BTreeMap<WorkflowNodeId, WorkflowNodeEntity>,
    input_bindings: BTreeMap<WorkflowInputTarget, WorkflowInputBinding>,
}

/// Pure persisted shape used to reconstruct one Workflow aggregate.
pub struct WorkflowAggregateRestoreData {
    /// Persisted hard-cut schema.
    pub schema_version: WorkflowSchemaVersion,
    /// Aggregate identity.
    pub id: WorkflowId,
    /// Owning Project identity.
    pub project_id: ProjectId,
    /// Persisted revision.
    pub revision: WorkflowRevision,
    /// Immutable creation time.
    pub created_at: WorkflowCreatedAt,
    /// Latest successful mutation time.
    pub updated_at: WorkflowUpdatedAt,
    /// Node records whose duplicate identities must remain observable.
    pub nodes: Vec<WorkflowNodeEntity>,
    /// Target/binding records whose duplicate targets must remain observable.
    pub input_bindings: Vec<(WorkflowInputTarget, WorkflowInputBinding)>,
}

impl WorkflowAggregate {
    /// Reconstructs only a completely draft-valid graph without performing I/O.
    pub fn try_restore(
        data: WorkflowAggregateRestoreData,
        capabilities: &WorkflowNodeCapabilityRegistry,
    ) -> Result<Self, WorkflowGraphConstructionError> {
        if data.updated_at.as_utc_milliseconds() < data.created_at.as_utc_milliseconds() {
            return Err(WorkflowGraphConstructionError::TimestampOutOfRange);
        }
        let nodes = collect_unique_nodes(data.nodes)?;
        let input_bindings = collect_unique_bindings(data.input_bindings)?;
        let contracts = resolve_contracts(&nodes, capabilities)?;
        validate_bindings(&nodes, &input_bindings, &contracts)?;
        reject_cycles(&nodes, &input_bindings)?;
        Ok(Self {
            schema_version: data.schema_version,
            id: data.id,
            project_id: data.project_id,
            revision: data.revision,
            created_at: data.created_at,
            updated_at: data.updated_at,
            nodes,
            input_bindings,
        })
    }

    /// Returns nodes in stable identity order.
    #[must_use]
    pub const fn nodes(&self) -> &BTreeMap<WorkflowNodeId, WorkflowNodeEntity> {
        &self.nodes
    }

    /// Returns bindings in stable target order while preserving item vector order.
    #[must_use]
    pub const fn input_bindings(&self) -> &BTreeMap<WorkflowInputTarget, WorkflowInputBinding> {
        &self.input_bindings
    }

    /// Derives incoming stable item IDs by target node without storing an index.
    #[must_use]
    pub fn derive_incoming_input_item_ids(
        &self,
    ) -> BTreeMap<WorkflowNodeId, Vec<crate::node_capability::WorkflowInputItemId>> {
        let mut index = BTreeMap::<_, Vec<_>>::new();
        for (target, binding) in &self.input_bindings {
            index.entry(target.node_id).or_default().extend(binding.items().map(|item| item.id));
        }
        index
    }

    /// Derives outgoing stable item IDs by source node without storing an index.
    #[must_use]
    pub fn derive_outgoing_input_item_ids(
        &self,
    ) -> BTreeMap<WorkflowNodeId, Vec<crate::node_capability::WorkflowInputItemId>> {
        let mut index = BTreeMap::<_, Vec<_>>::new();
        for binding in self.input_bindings.values() {
            for item in binding.items() {
                index.entry(item.source_node_id).or_default().push(item.id);
            }
        }
        index
    }
}

fn collect_unique_nodes(
    nodes: Vec<WorkflowNodeEntity>,
) -> Result<BTreeMap<WorkflowNodeId, WorkflowNodeEntity>, WorkflowGraphConstructionError> {
    let mut result = BTreeMap::new();
    for node in nodes {
        if result.insert(node.id, node).is_some() {
            return Err(WorkflowGraphConstructionError::DuplicateNode);
        }
    }
    Ok(result)
}

fn collect_unique_bindings(
    bindings: Vec<(WorkflowInputTarget, WorkflowInputBinding)>,
) -> Result<BTreeMap<WorkflowInputTarget, WorkflowInputBinding>, WorkflowGraphConstructionError> {
    let mut result = BTreeMap::new();
    for (target, binding) in bindings {
        if result.insert(target, binding).is_some() {
            return Err(WorkflowGraphConstructionError::InputOccupied);
        }
    }
    Ok(result)
}

fn resolve_contracts(
    nodes: &BTreeMap<WorkflowNodeId, WorkflowNodeEntity>,
    capabilities: &WorkflowNodeCapabilityRegistry,
) -> Result<BTreeMap<WorkflowNodeId, NodeCapabilityContract>, WorkflowGraphConstructionError> {
    let mut contracts = BTreeMap::new();
    for node in nodes.values() {
        let implementation = capabilities
            .resolve_node_capability(&node.capability_contract)
            .map_err(|_| WorkflowGraphConstructionError::ReferenceViolation)?;
        implementation
            .normalize_node_parameters(&node.parameter_set)
            .map_err(|_| WorkflowGraphConstructionError::ReferenceViolation)?;
        contracts.insert(node.id, implementation.node_capability_contract().clone());
    }
    Ok(contracts)
}

fn validate_bindings(
    nodes: &BTreeMap<WorkflowNodeId, WorkflowNodeEntity>,
    bindings: &BTreeMap<WorkflowInputTarget, WorkflowInputBinding>,
    contracts: &BTreeMap<WorkflowNodeId, NodeCapabilityContract>,
) -> Result<(), WorkflowGraphConstructionError> {
    let mut item_ids = BTreeSet::new();
    for (target, binding) in bindings {
        let target_contract =
            contracts.get(&target.node_id).ok_or(WorkflowGraphConstructionError::NodeNotFound)?;
        let input = target_contract
            .inputs()
            .iter()
            .find(|input| input.key() == &target.input_key)
            .ok_or(WorkflowGraphConstructionError::InputNotFound)?;
        validate_binding_shape(binding, input.binding())?;
        let mut endpoints = BTreeSet::new();
        for item in binding.items() {
            if !item_ids.insert(item.id) {
                return Err(WorkflowGraphConstructionError::DuplicateInputItem);
            }
            if !endpoints.insert((item.source_node_id, item.source_output_key.clone())) {
                return Err(WorkflowGraphConstructionError::ReferenceViolation);
            }
            validate_item(nodes, contracts, target, item, input.binding())?;
        }
    }
    Ok(())
}

fn validate_binding_shape(
    binding: &WorkflowInputBinding,
    contract: &NodeCapabilityInputBindingContract,
) -> Result<(), WorkflowGraphConstructionError> {
    match (binding, contract) {
        (
            WorkflowInputBinding::Single { .. },
            NodeCapabilityInputBindingContract::OptionalSingleValue { .. }
            | NodeCapabilityInputBindingContract::RequiredSingleValue { .. },
        ) => Ok(()),
        (
            WorkflowInputBinding::OrderedReferences { items },
            NodeCapabilityInputBindingContract::OrderedReferences {
                minimum_items,
                maximum_items,
                ..
            },
        ) => {
            let count = u32::try_from(items.as_slice().len())
                .map_err(|_| WorkflowGraphConstructionError::CardinalityViolation)?;
            if count < *minimum_items || maximum_items.is_some_and(|maximum| count > maximum) {
                Err(WorkflowGraphConstructionError::CardinalityViolation)
            } else {
                Ok(())
            }
        }
        _ => Err(WorkflowGraphConstructionError::BindingShapeMismatch),
    }
}

fn validate_item(
    nodes: &BTreeMap<WorkflowNodeId, WorkflowNodeEntity>,
    contracts: &BTreeMap<WorkflowNodeId, NodeCapabilityContract>,
    target: &WorkflowInputTarget,
    item: &WorkflowInputItemEntity,
    input: &NodeCapabilityInputBindingContract,
) -> Result<(), WorkflowGraphConstructionError> {
    if item.source_node_id == target.node_id {
        return Err(WorkflowGraphConstructionError::SelfEdge);
    }
    if !nodes.contains_key(&item.source_node_id) {
        return Err(WorkflowGraphConstructionError::NodeNotFound);
    }
    let source_contract =
        contracts.get(&item.source_node_id).ok_or(WorkflowGraphConstructionError::NodeNotFound)?;
    let output = source_contract
        .outputs()
        .iter()
        .find(|output| output.key() == &item.source_output_key)
        .ok_or(WorkflowGraphConstructionError::OutputNotFound)?;
    match input {
        NodeCapabilityInputBindingContract::OptionalSingleValue { data_type }
        | NodeCapabilityInputBindingContract::RequiredSingleValue { data_type } => {
            if output.data_type() == *data_type {
                Ok(())
            } else {
                Err(WorkflowGraphConstructionError::DataTypeMismatch)
            }
        }
        NodeCapabilityInputBindingContract::OrderedReferences {
            accepted_data_types_by_role,
            ..
        } => {
            let role = item
                .input_role_key
                .as_ref()
                .ok_or(WorkflowGraphConstructionError::RoleViolation)?;
            let accepted = accepted_data_types_by_role
                .get(role)
                .ok_or(WorkflowGraphConstructionError::RoleViolation)?;
            if accepted.contains(output.data_type()) {
                Ok(())
            } else {
                Err(WorkflowGraphConstructionError::DataTypeMismatch)
            }
        }
    }
}

fn reject_cycles(
    nodes: &BTreeMap<WorkflowNodeId, WorkflowNodeEntity>,
    bindings: &BTreeMap<WorkflowInputTarget, WorkflowInputBinding>,
) -> Result<(), WorkflowGraphConstructionError> {
    let mut indegree = nodes.keys().map(|id| (*id, 0_usize)).collect::<BTreeMap<_, _>>();
    let mut outgoing = BTreeMap::<WorkflowNodeId, BTreeSet<WorkflowNodeId>>::new();
    for (target, binding) in bindings {
        for item in binding.items() {
            if outgoing.entry(item.source_node_id).or_default().insert(target.node_id) {
                *indegree
                    .get_mut(&target.node_id)
                    .ok_or(WorkflowGraphConstructionError::NodeNotFound)? += 1;
            }
        }
    }
    let mut ready = indegree
        .iter()
        .filter_map(|(id, degree)| (*degree == 0).then_some(*id))
        .collect::<Vec<_>>();
    let mut visited = 0_usize;
    while let Some(source) = ready.pop() {
        visited += 1;
        if let Some(targets) = outgoing.get(&source) {
            for target in targets {
                let degree =
                    indegree.get_mut(target).ok_or(WorkflowGraphConstructionError::NodeNotFound)?;
                *degree -= 1;
                if *degree == 0 {
                    ready.push(*target);
                }
            }
        }
    }
    if visited == nodes.len() { Ok(()) } else { Err(WorkflowGraphConstructionError::Cycle) }
}
