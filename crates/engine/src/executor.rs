//! Workflow executor and deterministic result cache.

use crate::error::{EngineError, Result};
use crate::graph::{OutputRef, Workflow, WorkflowNode};
use crate::node::Node;
use crate::registry::NodeRegistry;
use crate::value::{Value, ValueMap};
use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};
use tracing::{debug, info};

/// Outputs produced by a single run, keyed by node id.
pub type RunOutputs = BTreeMap<String, ValueMap>;

#[derive(Default)]
pub struct ResultCache {
    entries: BTreeMap<String, CacheEntry>,
}

impl ResultCache {
    /// Creates an empty cache.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

/// Builds an execution plan from a [`Workflow`], validating wiring and ordering
/// nodes topologically, then runs nodes in order consulting the [`ResultCache`].
pub struct Executor<'r> {
    registry: &'r NodeRegistry,
}

impl<'r> Executor<'r> {
    /// Creates an executor bound to a node registry.
    #[must_use]
    pub fn new(registry: &'r NodeRegistry) -> Self {
        Self { registry }
    }

    /// Validates and executes `workflow`, returning each node's outputs.
    pub fn execute(&self, workflow: &Workflow, cache: &mut ResultCache) -> Result<RunOutputs> {
        let plan = self.build_plan(workflow)?;
        let order = topological_order(&plan)?;
        let mut outputs = RunOutputs::new();

        for index in order {
            let node = &plan.nodes[index];
            let inputs = resolve_inputs(node, &outputs)?;
            let fingerprint = cache_fingerprint(&node.type_id, &node.params, &inputs);

            if let Some(cached) = cache.get(&node.id, fingerprint) {
                info!(
                    node_id = %node.id,
                    type_id = %node.type_id,
                    "reusing cached node outputs"
                );
                outputs.insert(node.id.clone(), cached);
                continue;
            }

            info!(node_id = %node.id, type_id = %node.type_id, "executing node");
            let result = node.node.run(&inputs).map_err(|source| {
                EngineError::from((node.id.as_str(), node.type_id.as_str(), source))
            })?;
            info!(node_id = %node.id, type_id = %node.type_id, "node execution completed");
            cache.insert(node.id.clone(), fingerprint, result.clone());
            outputs.insert(node.id.clone(), result);
        }

        Ok(outputs)
    }

    fn build_plan(&self, workflow: &Workflow) -> Result<ExecutionPlan> {
        let mut nodes = Vec::with_capacity(workflow.nodes.len());
        for workflow_node in &workflow.nodes {
            nodes.push(PlanNode::from_workflow_node(
                workflow_node,
                self.registry.instantiate(
                    &workflow_node.id,
                    &workflow_node.type_id,
                    &workflow_node.params,
                )?,
            ));
        }

        let node_indexes = node_indexes(&nodes);
        validate_wiring(&nodes, &node_indexes)?;
        Ok(ExecutionPlan { nodes })
    }
}

#[derive(Clone)]
struct CacheEntry {
    fingerprint: u64,
    outputs: ValueMap,
}

impl ResultCache {
    fn get(&self, node_id: &str, fingerprint: u64) -> Option<ValueMap> {
        self.entries
            .get(node_id)
            .filter(|entry| entry.fingerprint == fingerprint)
            .map(|entry| entry.outputs.clone())
    }

    fn insert(&mut self, node_id: String, fingerprint: u64, outputs: ValueMap) {
        self.entries.insert(node_id, CacheEntry { fingerprint, outputs });
    }
}

struct ExecutionPlan {
    nodes: Vec<PlanNode>,
}

struct PlanNode {
    id: String,
    type_id: String,
    params: crate::registry::NodeParams,
    inputs: BTreeMap<String, OutputRef>,
    node: Box<dyn Node>,
}

impl PlanNode {
    fn from_workflow_node(workflow_node: &WorkflowNode, node: Box<dyn Node>) -> Self {
        Self {
            id: workflow_node.id.clone(),
            type_id: workflow_node.type_id.clone(),
            params: workflow_node.params.clone(),
            inputs: workflow_node.inputs.clone(),
            node,
        }
    }
}

fn node_indexes(nodes: &[PlanNode]) -> BTreeMap<String, usize> {
    nodes.iter().enumerate().map(|(index, node)| (node.id.clone(), index)).collect()
}

fn validate_wiring(nodes: &[PlanNode], node_indexes: &BTreeMap<String, usize>) -> Result<()> {
    for node in nodes {
        for input in node.node.inputs() {
            if input.required && input.default.is_none() && !node.inputs.contains_key(&input.name) {
                return Err(EngineError::MissingRequiredInput {
                    node_id: node.id.clone(),
                    input: input.name.clone(),
                });
            }
        }

        for (input_name, source) in &node.inputs {
            let source_index = node_indexes.get(source.node_id()).ok_or_else(|| {
                EngineError::UnknownSourceNode {
                    node_id: node.id.clone(),
                    input: input_name.clone(),
                    source_node: source.node_id().to_owned(),
                }
            })?;
            validate_source_output(nodes, node, input_name, source, *source_index)?;
        }
    }
    Ok(())
}

fn validate_source_output(
    nodes: &[PlanNode],
    node: &PlanNode,
    input_name: &str,
    source: &OutputRef,
    source_index: usize,
) -> Result<()> {
    let source_node = &nodes[source_index];
    let source_output = source_node.node.output_port(source.output_name()).ok_or_else(|| {
        EngineError::UnknownSourceOutput {
            node_id: node.id.clone(),
            input: input_name.to_owned(),
            source_node: source.node_id().to_owned(),
            output: source.output_name().to_owned(),
        }
    })?;

    if let Some(input_port) = node.node.input_port(input_name)
        && !source_output.port_type.is_compatible_with(input_port.port_type)
    {
        return Err(EngineError::TypeMismatch {
            node_id: node.id.clone(),
            input: input_name.to_owned(),
            input_type: input_port.port_type,
            source_node: source.node_id().to_owned(),
            output: source.output_name().to_owned(),
            source_type: source_output.port_type,
        });
    }

    Ok(())
}

fn topological_order(plan: &ExecutionPlan) -> Result<Vec<usize>> {
    let mut emitted = BTreeSet::new();
    let mut order = Vec::with_capacity(plan.nodes.len());

    while order.len() < plan.nodes.len() {
        let mut progressed = false;
        for (index, node) in plan.nodes.iter().enumerate() {
            if emitted.contains(&index) || !dependencies_emitted(node, plan, &emitted) {
                continue;
            }
            emitted.insert(index);
            order.push(index);
            progressed = true;
        }

        if !progressed {
            let node_id = first_unemitted_node_id(plan, &emitted);
            return Err(EngineError::Cycle { node_id });
        }
    }

    debug!(node_count = order.len(), "workflow graph ordered topologically");
    Ok(order)
}

fn dependencies_emitted(node: &PlanNode, plan: &ExecutionPlan, emitted: &BTreeSet<usize>) -> bool {
    node.inputs.values().all(|source| {
        plan.nodes
            .iter()
            .position(|candidate| candidate.id == source.0)
            .is_some_and(|index| emitted.contains(&index))
    })
}

fn first_unemitted_node_id(plan: &ExecutionPlan, emitted: &BTreeSet<usize>) -> String {
    plan.nodes
        .iter()
        .enumerate()
        .find(|(index, _)| !emitted.contains(index))
        .map(|(_, node)| node.id.clone())
        .unwrap_or_default()
}

fn resolve_inputs(node: &PlanNode, outputs: &RunOutputs) -> Result<ValueMap> {
    let mut inputs = ValueMap::new();
    for port in node.node.inputs() {
        if let Some(default) = &port.default {
            inputs.insert(port.name.clone(), default.clone());
        }
    }

    for (input_name, source) in &node.inputs {
        let source_outputs =
            outputs.get(source.node_id()).ok_or_else(|| EngineError::UnknownSourceNode {
                node_id: node.id.clone(),
                input: input_name.clone(),
                source_node: source.node_id().to_owned(),
            })?;
        let value = source_outputs.get(source.output_name()).ok_or_else(|| {
            EngineError::UnknownSourceOutput {
                node_id: node.id.clone(),
                input: input_name.clone(),
                source_node: source.node_id().to_owned(),
                output: source.output_name().to_owned(),
            }
        })?;
        inputs.insert(input_name.clone(), value.clone());
    }

    Ok(inputs)
}

fn cache_fingerprint(
    type_id: &str,
    params: &crate::registry::NodeParams,
    inputs: &ValueMap,
) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    type_id.hash(&mut hasher);
    hash_params(params, &mut hasher);
    hash_value_map(inputs, &mut hasher);
    hasher.finish()
}

fn hash_params(params: &crate::registry::NodeParams, state: &mut impl Hasher) {
    for (key, value) in params {
        key.hash(state);
        hash_json_value(value, state);
    }
}

fn hash_json_value(value: &serde_json::Value, state: &mut impl Hasher) {
    match value {
        serde_json::Value::Null => 0_u8.hash(state),
        serde_json::Value::Bool(value) => {
            1_u8.hash(state);
            value.hash(state);
        }
        serde_json::Value::Number(value) => hash_json_number(value, state),
        serde_json::Value::String(value) => {
            3_u8.hash(state);
            value.hash(state);
        }
        serde_json::Value::Array(values) => {
            4_u8.hash(state);
            for value in values {
                hash_json_value(value, state);
            }
        }
        serde_json::Value::Object(values) => {
            5_u8.hash(state);
            hash_params(values, state);
        }
    }
}

fn hash_json_number(value: &serde_json::Number, state: &mut impl Hasher) {
    2_u8.hash(state);
    if let Some(value) = value.as_i64() {
        0_u8.hash(state);
        value.hash(state);
    } else if let Some(value) = value.as_u64() {
        1_u8.hash(state);
        value.hash(state);
    } else if let Some(value) = value.as_f64() {
        2_u8.hash(state);
        value.to_bits().hash(state);
    }
}

fn hash_value_map(values: &ValueMap, state: &mut impl Hasher) {
    for (key, value) in values {
        key.hash(state);
        hash_value(value, state);
    }
}

fn hash_value(value: &Value, state: &mut impl Hasher) {
    match value {
        Value::String(value) => hash_tagged_string(0, value, state),
        Value::Image(value) => hash_tagged_string(1, value, state),
        Value::Video(value) => hash_tagged_string(2, value, state),
        Value::Model(value) => hash_tagged_string(3, value, state),
        Value::Int(value) => {
            4_u8.hash(state);
            value.hash(state);
        }
        Value::Float(value) => {
            5_u8.hash(state);
            value.to_bits().hash(state);
        }
    }
}

fn hash_tagged_string(tag: u8, value: &str, state: &mut impl Hasher) {
    tag.hash(state);
    value.hash(state);
}
