use std::collections::{BTreeMap, BTreeSet};

use crate::node_capability::{
    NodeCapabilityContractRef, NodeCapabilityInputKey, NodeCapabilityNormalizedParameters,
    WorkflowNodeExecutionId,
};
use crate::workflow_graph::{WorkflowId, WorkflowInputBinding, WorkflowNodeId, WorkflowRevision};

use super::{WorkflowDomainError, WorkflowRunScope};

/// One named frozen binding, including stable items and exact producer coordinates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowPlannedInputBinding {
    /// Target capability input.
    pub input_key: NodeCapabilityInputKey,
    /// Frozen single or ordered graph binding.
    pub binding: WorkflowInputBinding,
}

/// Everything required to execute one node after admission.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowPlannedNode {
    /// Source Workflow node identity.
    pub node_id: WorkflowNodeId,
    /// Run-local provider-idempotency identity.
    pub node_execution_id: WorkflowNodeExecutionId,
    /// Exact admitted capability contract.
    pub capability_contract: NodeCapabilityContractRef,
    /// Complete validated and defaulted parameters.
    pub normalized_parameters: NodeCapabilityNormalizedParameters,
    /// Named bindings in ascending input-key order.
    pub input_bindings: Vec<WorkflowPlannedInputBinding>,
}

/// Immutable provider-independent plan frozen at Run admission.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowExecutionPlan {
    workflow_id: WorkflowId,
    workflow_revision: WorkflowRevision,
    scope: WorkflowRunScope,
    nodes: Vec<WorkflowPlannedNode>,
}

impl WorkflowExecutionPlan {
    /// Builds a plan whose node order is already the deterministic topological order.
    pub fn try_new(
        workflow_id: WorkflowId,
        workflow_revision: WorkflowRevision,
        scope: WorkflowRunScope,
        nodes: Vec<WorkflowPlannedNode>,
    ) -> Result<Self, WorkflowDomainError> {
        if nodes.is_empty() {
            return Err(WorkflowDomainError::InvalidWorkflowExecutionPlan);
        }
        let positions = nodes
            .iter()
            .enumerate()
            .map(|(index, node)| (node.node_id, index))
            .collect::<BTreeMap<_, _>>();
        let execution_ids =
            nodes.iter().map(|node| node.node_execution_id).collect::<BTreeSet<_>>();
        if positions.len() != nodes.len() || execution_ids.len() != nodes.len() {
            return Err(WorkflowDomainError::InvalidWorkflowExecutionPlan);
        }
        if let WorkflowRunScope::ThroughNode(selected) = scope
            && !positions.contains_key(&selected)
        {
            return Err(WorkflowDomainError::InvalidWorkflowExecutionPlan);
        }
        for (target_index, node) in nodes.iter().enumerate() {
            let unique_inputs = node
                .input_bindings
                .iter()
                .map(|binding| &binding.input_key)
                .collect::<BTreeSet<_>>();
            if unique_inputs.len() != node.input_bindings.len() {
                return Err(WorkflowDomainError::InvalidWorkflowExecutionPlan);
            }
            for item in node.input_bindings.iter().flat_map(|binding| binding.binding.items()) {
                let Some(source_index) = positions.get(&item.source_node_id) else {
                    return Err(WorkflowDomainError::InvalidWorkflowExecutionPlan);
                };
                if *source_index >= target_index {
                    return Err(WorkflowDomainError::InvalidWorkflowExecutionPlan);
                }
            }
        }
        validate_deterministic_order(&nodes, &positions)?;
        if let WorkflowRunScope::ThroughNode(selected) = scope {
            validate_ancestor_only_scope(selected, &nodes)?;
        }
        Ok(Self { workflow_id, workflow_revision, scope, nodes })
    }

    /// Returns the source Workflow identity.
    #[must_use]
    pub const fn workflow_id(&self) -> WorkflowId {
        self.workflow_id
    }
    /// Returns the exact source revision.
    #[must_use]
    pub const fn workflow_revision(&self) -> WorkflowRevision {
        self.workflow_revision
    }
    /// Returns the admitted scope.
    #[must_use]
    pub const fn scope(&self) -> WorkflowRunScope {
        self.scope
    }
    /// Returns planned nodes in deterministic topological order.
    #[must_use]
    pub fn nodes(&self) -> &[WorkflowPlannedNode] {
        &self.nodes
    }
}

fn validate_deterministic_order(
    nodes: &[WorkflowPlannedNode],
    positions: &BTreeMap<WorkflowNodeId, usize>,
) -> Result<(), WorkflowDomainError> {
    let mut remaining = nodes.iter().map(|node| node.node_id).collect::<BTreeSet<_>>();
    let dependencies = nodes
        .iter()
        .map(|node| {
            let sources = node
                .input_bindings
                .iter()
                .flat_map(|binding| binding.binding.items())
                .map(|item| item.source_node_id)
                .collect::<BTreeSet<_>>();
            (node.node_id, sources)
        })
        .collect::<BTreeMap<_, _>>();
    for expected in nodes {
        let next = remaining.iter().copied().find(|candidate| {
            dependencies[candidate].iter().all(|source| !remaining.contains(source))
        });
        if next != Some(expected.node_id) {
            return Err(WorkflowDomainError::InvalidWorkflowExecutionPlan);
        }
        remaining.remove(&expected.node_id);
    }
    debug_assert_eq!(positions.len(), dependencies.len());
    Ok(())
}

fn validate_ancestor_only_scope(
    selected: WorkflowNodeId,
    nodes: &[WorkflowPlannedNode],
) -> Result<(), WorkflowDomainError> {
    let dependencies = nodes
        .iter()
        .map(|node| {
            let sources = node
                .input_bindings
                .iter()
                .flat_map(|binding| binding.binding.items())
                .map(|item| item.source_node_id)
                .collect::<BTreeSet<_>>();
            (node.node_id, sources)
        })
        .collect::<BTreeMap<_, _>>();
    let mut included = BTreeSet::from([selected]);
    let mut frontier = vec![selected];
    while let Some(node_id) = frontier.pop() {
        for source in &dependencies[&node_id] {
            if included.insert(*source) {
                frontier.push(*source);
            }
        }
    }
    if included.len() == nodes.len() {
        Ok(())
    } else {
        Err(WorkflowDomainError::InvalidWorkflowExecutionPlan)
    }
}
