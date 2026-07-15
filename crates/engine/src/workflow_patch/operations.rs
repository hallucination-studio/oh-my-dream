//! Ordered mutation primitives for pure Workflow patch application.

use super::{
    NodeRef, WorkflowPatchError, WorkflowPatchOperation, indexed_error, instantiate,
    validation::has_cycle,
};
use crate::capability::CapabilityRef;
use crate::graph::{InputBinding, OutputRef, Workflow, WorkflowNode};
use crate::port::{PortCardinality, PortType};
use crate::registry::{NodeParams, NodeRegistry};
use std::collections::BTreeMap;

pub(super) fn apply_operation(
    registry: &NodeRegistry,
    workflow: &mut Workflow,
    aliases: &mut BTreeMap<String, String>,
    operation: &WorkflowPatchOperation,
    index: usize,
) -> Result<(), WorkflowPatchError> {
    match operation {
        WorkflowPatchOperation::AddNode { alias, capability, params, position } => add_node(
            registry,
            workflow,
            aliases,
            AddNodeSpec { alias, capability, params, position: *position },
            index,
        ),
        WorkflowPatchOperation::ReplaceParams { node, params } => {
            let node_id = resolve_node(node, workflow, aliases, index, "/node")?;
            let target = find_node_mut(workflow, &node_id, index)?;
            target.params = normalize_params(registry, target, params, index, "/params")?;
            Ok(())
        }
        WorkflowPatchOperation::SetInput { node, input, binding } => {
            set_input(registry, workflow, aliases, node, input, binding, index)
        }
        WorkflowPatchOperation::ClearInput { node, input } => {
            let node_id = resolve_node(node, workflow, aliases, index, "/node")?;
            let target = find_node_mut(workflow, &node_id, index)?;
            let instance = instantiate(registry, target, index, "/node")?;
            if instance.input_port(input).is_none() {
                return Err(indexed_error(
                    "INPUT_NOT_DECLARED",
                    "/input",
                    format!("node `{node_id}` does not declare input `{input}`"),
                    index,
                ));
            }
            target.inputs.remove(input);
            Ok(())
        }
        WorkflowPatchOperation::RemoveNode { node } => remove_node(workflow, aliases, node, index),
        WorkflowPatchOperation::SetPosition { node, position } => {
            validate_position(*position, index, "/position")?;
            let node_id = resolve_node(node, workflow, aliases, index, "/node")?;
            find_node_mut(workflow, &node_id, index)?.position = Some(*position);
            Ok(())
        }
    }
}

fn set_input(
    registry: &NodeRegistry,
    workflow: &mut Workflow,
    aliases: &BTreeMap<String, String>,
    node: &NodeRef,
    input: &str,
    binding: &InputBinding<NodeRef>,
    index: usize,
) -> Result<(), WorkflowPatchError> {
    let node_id = resolve_node(node, workflow, aliases, index, "/node")?;
    let target_index = node_index(workflow, &node_id, index)?;
    let target = workflow
        .nodes
        .get(target_index)
        .ok_or_else(|| indexed_error("NODE_NOT_FOUND", "/node", "target node is absent", index))?;
    let target_node = instantiate(registry, target, index, "/node")?;
    let port = target_node.input_port(input).ok_or_else(|| {
        indexed_error(
            "INPUT_NOT_DECLARED",
            "/input",
            format!("node `{node_id}` does not declare input `{input}`"),
            index,
        )
    })?;
    let converted = convert_binding(
        registry,
        workflow,
        aliases,
        binding,
        port.cardinality,
        port.port_type,
        index,
    )?;
    workflow.nodes[target_index].inputs.insert(input.to_owned(), converted);
    if has_cycle(workflow) {
        return Err(indexed_error(
            "WORKFLOW_CYCLE",
            "/binding",
            "input binding would create a Workflow cycle",
            index,
        ));
    }
    Ok(())
}

struct AddNodeSpec<'a> {
    alias: &'a str,
    capability: &'a CapabilityRef,
    params: &'a NodeParams,
    position: Option<[f64; 2]>,
}

fn add_node(
    registry: &NodeRegistry,
    workflow: &mut Workflow,
    aliases: &mut BTreeMap<String, String>,
    spec: AddNodeSpec<'_>,
    index: usize,
) -> Result<(), WorkflowPatchError> {
    if spec.alias.trim().is_empty() || aliases.contains_key(spec.alias) {
        return Err(indexed_error(
            "ALIAS_INVALID",
            "/alias",
            "add_node aliases must be non-empty and unique within a patch",
            index,
        ));
    }
    if let Some(position) = spec.position {
        validate_position(position, index, "/position")?;
    }
    let (type_id, normalized) =
        normalize_for_reference(registry, spec.capability, spec.params, index, "/params")?;
    let node_id = generated_node_id(workflow);
    workflow.nodes.push(WorkflowNode {
        id: node_id.clone(),
        type_id,
        contract_version: spec.capability.version.clone(),
        params: normalized,
        inputs: BTreeMap::new(),
        position: spec.position,
    });
    aliases.insert(spec.alias.to_owned(), node_id);
    Ok(())
}

fn convert_binding(
    registry: &NodeRegistry,
    workflow: &Workflow,
    aliases: &BTreeMap<String, String>,
    binding: &InputBinding<NodeRef>,
    cardinality: PortCardinality,
    target_type: PortType,
    index: usize,
) -> Result<InputBinding<OutputRef>, WorkflowPatchError> {
    let (sources, ordered) = match binding {
        InputBinding::Single { source } => (vec![source], false),
        InputBinding::OrderedMany { sources } => (sources.iter().collect(), true),
    };
    validate_binding_cardinality(ordered, cardinality, sources.len(), index)?;
    let mut converted = Vec::with_capacity(sources.len());
    for source in sources {
        let source_id = resolve_node(source, workflow, aliases, index, "/binding/source")?;
        let source_node =
            workflow.nodes.iter().find(|node| node.id == source_id).ok_or_else(|| {
                indexed_error("NODE_NOT_FOUND", "/binding/source", "source node is absent", index)
            })?;
        let instance = instantiate(registry, source_node, index, "/binding/source")?;
        let output = instance.outputs().first().ok_or_else(|| {
            indexed_error(
                "OUTPUT_NOT_DECLARED",
                "/binding/source",
                "source node has no outputs",
                index,
            )
        })?;
        if !output.port_type.is_compatible_with(target_type) {
            return Err(indexed_error(
                "INPUT_OUTPUT_TYPE_MISMATCH",
                "/binding/source",
                "source output type is incompatible with the target input",
                index,
            ));
        }
        converted.push(OutputRef(source_id, output.name.clone()));
    }
    if ordered {
        Ok(InputBinding::ordered_many(converted))
    } else {
        let source = converted.into_iter().next().ok_or_else(|| {
            indexed_error("INPUT_BINDING_EMPTY", "/binding", "binding must contain a source", index)
        })?;
        Ok(InputBinding::single(source))
    }
}

fn validate_binding_cardinality(
    ordered: bool,
    cardinality: PortCardinality,
    source_count: usize,
    index: usize,
) -> Result<(), WorkflowPatchError> {
    match (ordered, cardinality) {
        (false, PortCardinality::Many { .. }) => Err(indexed_error(
            "INPUT_BINDING_CARDINALITY",
            "/binding",
            "many inputs require an ordered_many binding",
            index,
        )),
        (true, PortCardinality::One) => Err(indexed_error(
            "INPUT_BINDING_CARDINALITY",
            "/binding",
            "single inputs require a single binding",
            index,
        )),
        (true, PortCardinality::Many { maximum: Some(maximum), .. }) if source_count > maximum => {
            Err(indexed_error(
                "INPUT_BINDING_CARDINALITY",
                "/binding/sources",
                format!("ordered_many accepts at most {maximum} sources"),
                index,
            ))
        }
        _ => Ok(()),
    }
}

pub(super) fn normalize_workflow(
    registry: &NodeRegistry,
    mut workflow: Workflow,
) -> Result<Workflow, WorkflowPatchError> {
    for (index, node) in workflow.nodes.iter_mut().enumerate() {
        let pointer = format!("/nodes/{index}/params");
        *node = registry.normalize_workflow_node(node).map_err(|error| {
            let diagnostic = super::engine_diagnostic(error, &pointer);
            WorkflowPatchError::new(
                diagnostic.code,
                diagnostic.pointer,
                diagnostic.constraint,
                None,
            )
        })?;
    }
    Ok(workflow)
}

fn normalize_params(
    registry: &NodeRegistry,
    node: &WorkflowNode,
    params: &NodeParams,
    index: usize,
    pointer: &str,
) -> Result<NodeParams, WorkflowPatchError> {
    let registration = registry
        .workflow_capability(&node.id, &node.type_id, &node.contract_version, params)
        .map_err(|error| {
            let diagnostic = super::engine_diagnostic(error, pointer);
            indexed_error(diagnostic.code, pointer, diagnostic.constraint, index)
        })?;
    registration.normalize_params(params).map_err(|error| {
        indexed_error("CAPABILITY_PARAMS_INVALID", pointer, error.to_string(), index)
    })
}

fn normalize_for_reference(
    registry: &NodeRegistry,
    reference: &CapabilityRef,
    params: &NodeParams,
    index: usize,
    pointer: &str,
) -> Result<(String, NodeParams), WorkflowPatchError> {
    let registration = registry.capability(reference).map_err(|error| {
        indexed_error("CAPABILITY_VERSION_UNAVAILABLE", pointer, error.to_string(), index)
    })?;
    let type_id =
        registration.selector().map(|selector| selector.type_id.clone()).ok_or_else(|| {
            indexed_error(
                "CAPABILITY_SELECTOR_UNAVAILABLE",
                pointer,
                "exact capability does not declare a Workflow selector",
                index,
            )
        })?;
    let params = registration.normalize_params(params).map_err(|error| {
        indexed_error("CAPABILITY_PARAMS_INVALID", pointer, error.to_string(), index)
    })?;
    Ok((type_id, params))
}

fn resolve_node(
    reference: &NodeRef,
    workflow: &Workflow,
    aliases: &BTreeMap<String, String>,
    index: usize,
    pointer: &str,
) -> Result<String, WorkflowPatchError> {
    let id = match reference {
        NodeRef::Id { id } => id.clone(),
        NodeRef::Alias { alias } => aliases.get(alias).cloned().ok_or_else(|| {
            indexed_error(
                "ALIAS_NOT_AVAILABLE",
                pointer,
                "alias was not introduced earlier in the patch",
                index,
            )
        })?,
    };
    if workflow.nodes.iter().any(|node| node.id == id) {
        Ok(id)
    } else {
        Err(indexed_error("NODE_NOT_FOUND", pointer, "referenced node does not exist", index))
    }
}

fn node_index(
    workflow: &Workflow,
    node_id: &str,
    index: usize,
) -> Result<usize, WorkflowPatchError> {
    workflow.nodes.iter().position(|node| node.id == node_id).ok_or_else(|| {
        indexed_error("NODE_NOT_FOUND", "/node", "referenced node does not exist", index)
    })
}

fn find_node_mut<'a>(
    workflow: &'a mut Workflow,
    node_id: &str,
    index: usize,
) -> Result<&'a mut WorkflowNode, WorkflowPatchError> {
    workflow.nodes.iter_mut().find(|node| node.id == node_id).ok_or_else(|| {
        indexed_error("NODE_NOT_FOUND", "/node", "referenced node does not exist", index)
    })
}

fn remove_node(
    workflow: &mut Workflow,
    aliases: &BTreeMap<String, String>,
    node: &NodeRef,
    index: usize,
) -> Result<(), WorkflowPatchError> {
    let node_id = resolve_node(node, workflow, aliases, index, "/node")?;
    workflow.nodes.retain(|candidate| candidate.id != node_id);
    for candidate in &mut workflow.nodes {
        candidate.inputs.retain(|_, binding| {
            remove_source(binding, &node_id);
            binding.sources().next().is_some()
        });
    }
    Ok(())
}

fn remove_source(binding: &mut InputBinding<OutputRef>, removed_node_id: &str) {
    match binding {
        InputBinding::Single { source } => {
            if source.node_id() == removed_node_id {
                *binding = InputBinding::ordered_many(Vec::new());
            }
        }
        InputBinding::OrderedMany { sources } => {
            sources.retain(|source| source.node_id() != removed_node_id);
        }
    }
}

fn generated_node_id(workflow: &Workflow) -> String {
    let mut counter = 1_u64;
    loop {
        let candidate = format!("n{counter}");
        if !workflow.nodes.iter().any(|node| node.id == candidate) {
            return candidate;
        }
        counter = counter.saturating_add(1);
    }
}

fn validate_position(
    position: [f64; 2],
    index: usize,
    pointer: &str,
) -> Result<(), WorkflowPatchError> {
    if position.iter().all(|value| value.is_finite()) {
        return Ok(());
    }
    Err(indexed_error("POSITION_INVALID", pointer, "position values must be finite", index))
}
