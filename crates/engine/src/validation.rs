//! Validation and preparation of executable workflow plans.

use crate::error::{EngineError, Result};
use crate::graph::{InputBinding, OutputRef, Workflow, WorkflowNode};
use crate::node::{InputPort, Node};
use crate::registry::{NodeParams, NodeRegistry};
use crate::value::ValueMap;
use std::collections::{BTreeMap, BTreeSet};

const SUPPORTED_WORKFLOW_VERSION: &str = "1.0";

pub(crate) struct ExecutionPlan {
    pub(crate) nodes: Vec<PlanNode>,
}

pub(crate) struct PlanNode {
    pub(crate) id: String,
    pub(crate) type_id: String,
    pub(crate) params: NodeParams,
    pub(crate) inputs: BTreeMap<String, InputBinding<OutputRef>>,
    pub(crate) node: Box<dyn Node>,
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

pub(crate) fn build_plan(registry: &NodeRegistry, workflow: &Workflow) -> Result<ExecutionPlan> {
    validate_workflow_identity(workflow)?;
    let nodes = instantiate_nodes(registry, workflow)?;
    let node_indexes = node_indexes(&nodes);
    validate_wiring(&nodes, &node_indexes)?;
    Ok(ExecutionPlan { nodes })
}

pub(crate) fn validate_node_outputs(node: &PlanNode, outputs: &ValueMap) -> Result<()> {
    for output in node.node.outputs() {
        let value = outputs.get(&output.name).ok_or_else(|| EngineError::MissingNodeOutput {
            node_id: node.id.clone(),
            output: output.name.clone(),
        })?;
        let actual_type = value.port_type();
        if actual_type != output.port_type {
            return Err(EngineError::OutputTypeMismatch {
                node_id: node.id.clone(),
                output: output.name.clone(),
                output_type: output.port_type,
                actual_type,
            });
        }
    }

    for output_name in outputs.keys() {
        if node.node.output_port(output_name).is_none() {
            return Err(EngineError::UnexpectedNodeOutput {
                node_id: node.id.clone(),
                output: output_name.clone(),
            });
        }
    }
    Ok(())
}

fn validate_workflow_identity(workflow: &Workflow) -> Result<()> {
    if workflow.version != SUPPORTED_WORKFLOW_VERSION {
        return Err(EngineError::UnsupportedWorkflowVersion { version: workflow.version.clone() });
    }

    let mut node_ids = BTreeSet::new();
    for node in &workflow.nodes {
        if !node_ids.insert(node.id.as_str()) {
            return Err(EngineError::DuplicateNodeId { node_id: node.id.clone() });
        }
    }
    Ok(())
}

fn instantiate_nodes(registry: &NodeRegistry, workflow: &Workflow) -> Result<Vec<PlanNode>> {
    workflow
        .nodes
        .iter()
        .map(|workflow_node| {
            registry
                .instantiate_workflow_node(
                    &workflow_node.id,
                    &workflow_node.type_id,
                    &workflow_node.contract_version,
                    &workflow_node.params,
                )
                .map(|node| PlanNode::from_workflow_node(workflow_node, node))
        })
        .collect()
}

fn node_indexes(nodes: &[PlanNode]) -> BTreeMap<String, usize> {
    nodes.iter().enumerate().map(|(index, node)| (node.id.clone(), index)).collect()
}

fn validate_wiring(nodes: &[PlanNode], node_indexes: &BTreeMap<String, usize>) -> Result<()> {
    for node in nodes {
        validate_input_defaults(node)?;
        for (input_name, binding) in &node.inputs {
            let input_port = node.node.input_port(input_name).ok_or_else(|| {
                EngineError::UnknownTargetInput {
                    node_id: node.id.clone(),
                    input: input_name.clone(),
                }
            })?;
            let sources = binding.sources().collect::<Vec<_>>();
            if matches!(binding, InputBinding::Single { .. })
                && !matches!(input_port.cardinality, crate::PortCardinality::One)
            {
                return Err(EngineError::InvalidWorkflow {
                    message: format!(
                        "input `{input_name}` on node `{}` requires an ordered-many binding",
                        node.id
                    ),
                });
            }
            if matches!(binding, InputBinding::OrderedMany { .. })
                && matches!(input_port.cardinality, crate::PortCardinality::One)
            {
                return Err(EngineError::InvalidWorkflow {
                    message: format!(
                        "input `{input_name}` on node `{}` accepts one source",
                        node.id
                    ),
                });
            }
            for source in sources {
                let source_index = node_indexes.get(source.node_id()).ok_or_else(|| {
                    EngineError::UnknownSourceNode {
                        node_id: node.id.clone(),
                        input: input_name.clone(),
                        source_node: source.node_id().to_owned(),
                    }
                })?;
                validate_source_output(nodes, node, input_port, source, *source_index)?;
            }
        }
    }
    Ok(())
}

fn validate_input_defaults(node: &PlanNode) -> Result<()> {
    for input in node.node.inputs() {
        if let Some(default) = &input.default
            && default.port_type() != input.port_type
        {
            return Err(EngineError::DefaultTypeMismatch {
                node_id: node.id.clone(),
                input: input.name.clone(),
                input_type: input.port_type,
                default_type: default.port_type(),
            });
        }
        if input.required && input.default.is_none() && !node.inputs.contains_key(&input.name) {
            return Err(EngineError::MissingRequiredInput {
                node_id: node.id.clone(),
                input: input.name.clone(),
            });
        }
        if let crate::PortCardinality::Many { minimum, .. } = input.cardinality {
            let count = match node.inputs.get(&input.name) {
                Some(InputBinding::OrderedMany { sources }) => sources.len(),
                Some(InputBinding::Single { .. }) => 1,
                None => 0,
            };
            if count < minimum {
                return Err(EngineError::MissingRequiredInput {
                    node_id: node.id.clone(),
                    input: input.name.clone(),
                });
            }
        }
    }
    Ok(())
}

fn validate_source_output(
    nodes: &[PlanNode],
    node: &PlanNode,
    input_port: &InputPort,
    source: &OutputRef,
    source_index: usize,
) -> Result<()> {
    let source_node = &nodes[source_index];
    let source_output = source_node.node.output_port(source.output_name()).ok_or_else(|| {
        EngineError::UnknownSourceOutput {
            node_id: node.id.clone(),
            input: input_port.name.clone(),
            source_node: source.node_id().to_owned(),
            output: source.output_name().to_owned(),
        }
    })?;

    if !source_output.port_type.is_compatible_with(input_port.port_type) {
        return Err(EngineError::TypeMismatch {
            node_id: node.id.clone(),
            input: input_port.name.clone(),
            input_type: input_port.port_type,
            source_node: source.node_id().to_owned(),
            output: source.output_name().to_owned(),
            source_type: source_output.port_type,
        });
    }
    Ok(())
}
