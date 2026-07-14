//! Persistability and execution-readiness validation for Workflow documents.

use super::{diagnostic, instantiate};
use crate::graph::{InputBinding, Workflow, WorkflowNode};
use crate::port::PortCardinality;
use crate::registry::NodeRegistry;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};

/// A stable machine-readable Workflow validation failure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowDiagnostic {
    /// Stable diagnostic code.
    pub code: String,
    /// JSON Pointer into the canonical Workflow document.
    pub pointer: String,
    /// Human-readable constraint that was violated.
    pub constraint: String,
}

/// A condition that permits persistence but prevents execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowReadinessBlocker {
    /// Stable blocker code.
    pub code: String,
    /// JSON Pointer into the canonical Workflow document.
    pub pointer: String,
    /// Human-readable requirement for becoming executable.
    pub constraint: String,
}

/// The authoritative validation projection for a Workflow document.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowValidationReport {
    /// Errors that make the Workflow unsafe to persist as canonical state.
    pub persistence_errors: Vec<WorkflowDiagnostic>,
    /// Missing execution requirements that remain editable in persisted state.
    pub readiness_blockers: Vec<WorkflowReadinessBlocker>,
}

/// Validates canonical Workflow structure separately from execution readiness.
#[must_use]
pub fn validate_workflow(registry: &NodeRegistry, workflow: &Workflow) -> WorkflowValidationReport {
    let mut report = WorkflowValidationReport::default();
    let mut node_ids = BTreeSet::new();
    let node_map =
        workflow.nodes.iter().map(|node| (node.id.as_str(), node)).collect::<HashMap<_, _>>();

    for (index, node) in workflow.nodes.iter().enumerate() {
        if node.id.trim().is_empty() || !node_ids.insert(node.id.as_str()) {
            report.persistence_errors.push(diagnostic(
                "NODE_ID_INVALID",
                format!("/nodes/{index}/id"),
                "node ids must be non-empty and unique",
            ));
        }
        if node
            .position
            .is_some_and(|position| position.iter().any(|coordinate| !coordinate.is_finite()))
        {
            report.persistence_errors.push(diagnostic(
                "POSITION_INVALID",
                format!("/nodes/{index}/position"),
                "position values must be finite",
            ));
        }
        validate_node(registry, node, index, &node_map, &mut report);
    }
    detect_cycles(workflow, &mut report);
    report
}

fn validate_node(
    registry: &NodeRegistry,
    node: &WorkflowNode,
    index: usize,
    node_map: &HashMap<&str, &WorkflowNode>,
    report: &mut WorkflowValidationReport,
) {
    let pointer = format!("/nodes/{index}");
    let instance = match instantiate(registry, node, index, &format!("{pointer}/type")) {
        Ok(instance) => instance,
        Err(error) => {
            report.persistence_errors.push(error.diagnostic());
            return;
        }
    };
    for input_name in node.inputs.keys() {
        if instance.input_port(input_name).is_none() {
            report.persistence_errors.push(diagnostic(
                "INPUT_NOT_DECLARED",
                format!("{pointer}/inputs/{}", escape_pointer(input_name)),
                "input is not declared by the exact capability contract",
            ));
        }
    }
    for input in instance.inputs() {
        validate_input(node, input, index, node_map, registry, report);
    }
}

fn validate_input(
    node: &WorkflowNode,
    input: &crate::InputPort,
    index: usize,
    node_map: &HashMap<&str, &WorkflowNode>,
    registry: &NodeRegistry,
    report: &mut WorkflowValidationReport,
) {
    let pointer = format!("/nodes/{index}/inputs/{}", escape_pointer(&input.name));
    let Some(binding) = node.inputs.get(&input.name) else {
        add_missing_blockers(input, &pointer, report);
        return;
    };
    let sources = binding.sources().collect::<Vec<_>>();
    validate_cardinality(binding, input.cardinality, &pointer, sources.len(), report);
    for (source_index, source) in sources.iter().enumerate() {
        let source_pointer = source_pointer(binding, &pointer, source_index);
        validate_source(source, input, index, &source_pointer, node_map, registry, report);
    }
}

fn add_missing_blockers(
    input: &crate::InputPort,
    pointer: &str,
    report: &mut WorkflowValidationReport,
) {
    if input.required && input.default.is_none() {
        report.readiness_blockers.push(WorkflowReadinessBlocker {
            code: "REQUIRED_INPUT_MISSING".to_owned(),
            pointer: pointer.to_owned(),
            constraint: "required input must be connected before execution".to_owned(),
        });
    }
    if let PortCardinality::Many { minimum, .. } = input.cardinality
        && minimum > 0
    {
        report.readiness_blockers.push(WorkflowReadinessBlocker {
            code: "INPUT_CARDINALITY_UNSATISFIED".to_owned(),
            pointer: pointer.to_owned(),
            constraint: format!("at least {minimum} sources are required"),
        });
    }
}

fn validate_cardinality<T>(
    binding: &InputBinding<T>,
    cardinality: PortCardinality,
    pointer: &str,
    source_count: usize,
    report: &mut WorkflowValidationReport,
) {
    match (binding, cardinality) {
        (InputBinding::Single { .. }, PortCardinality::Many { .. })
        | (InputBinding::OrderedMany { .. }, PortCardinality::One) => {
            report.persistence_errors.push(diagnostic(
                "INPUT_BINDING_CARDINALITY",
                pointer,
                "binding kind does not match input cardinality",
            ));
        }
        (InputBinding::OrderedMany { .. }, PortCardinality::Many { minimum, maximum }) => {
            if source_count < minimum {
                report.readiness_blockers.push(WorkflowReadinessBlocker {
                    code: "INPUT_CARDINALITY_UNSATISFIED".to_owned(),
                    pointer: pointer.to_owned(),
                    constraint: format!("at least {minimum} sources are required"),
                });
            }
            if maximum.is_some_and(|maximum| source_count > maximum) {
                report.persistence_errors.push(diagnostic(
                    "INPUT_CARDINALITY_EXCEEDED",
                    pointer,
                    "ordered binding exceeds the input maximum",
                ));
            }
        }
        _ => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_source(
    source: &crate::OutputRef,
    input: &crate::InputPort,
    node_index: usize,
    pointer: &str,
    node_map: &HashMap<&str, &WorkflowNode>,
    registry: &NodeRegistry,
    report: &mut WorkflowValidationReport,
) {
    let Some(source_node) = node_map.get(source.node_id()) else {
        report.persistence_errors.push(diagnostic(
            "SOURCE_NODE_NOT_FOUND",
            pointer,
            "binding source node does not exist",
        ));
        return;
    };
    let source_instance = match instantiate(registry, source_node, node_index, pointer) {
        Ok(instance) => instance,
        Err(error) => {
            report.persistence_errors.push(error.diagnostic());
            return;
        }
    };
    let Some(output) = source_instance.output_port(source.output_name()) else {
        report.persistence_errors.push(diagnostic(
            "SOURCE_OUTPUT_NOT_FOUND",
            pointer,
            "binding source output is not declared by the source capability",
        ));
        return;
    };
    if !output.port_type.is_compatible_with(input.port_type) {
        report.persistence_errors.push(diagnostic(
            "INPUT_OUTPUT_TYPE_MISMATCH",
            pointer,
            "source output type is incompatible with the target input",
        ));
    }
}

fn source_pointer<T>(binding: &InputBinding<T>, pointer: &str, index: usize) -> String {
    match binding {
        InputBinding::Single { .. } => format!("{pointer}/source"),
        InputBinding::OrderedMany { .. } => format!("{pointer}/sources/{index}"),
    }
}

fn detect_cycles(workflow: &Workflow, report: &mut WorkflowValidationReport) {
    let Some(node_id) = cycle_node(workflow) else {
        return;
    };
    report.persistence_errors.push(diagnostic(
        "WORKFLOW_CYCLE",
        "/nodes",
        format!("workflow graph contains a cycle involving `{node_id}`"),
    ));
}

pub(super) fn has_cycle(workflow: &Workflow) -> bool {
    cycle_node(workflow).is_some()
}

fn cycle_node(workflow: &Workflow) -> Option<&str> {
    let mut state = HashMap::<String, u8>::new();
    let nodes =
        workflow.nodes.iter().map(|node| (node.id.as_str(), node)).collect::<HashMap<_, _>>();
    for node in &workflow.nodes {
        if state.get(node.id.as_str()).copied().unwrap_or(0) == 0
            && visit_cycle(node.id.as_str(), &nodes, &mut state)
        {
            return Some(node.id.as_str());
        }
    }
    None
}

fn visit_cycle(
    node_id: &str,
    nodes: &HashMap<&str, &WorkflowNode>,
    state: &mut HashMap<String, u8>,
) -> bool {
    state.insert(node_id.to_owned(), 1);
    let Some(node) = nodes.get(node_id) else {
        state.insert(node_id.to_owned(), 2);
        return false;
    };
    for source in node.inputs.values().flat_map(InputBinding::sources) {
        if state.get(source.node_id()).copied().unwrap_or(0) == 1 {
            return true;
        }
        if state.get(source.node_id()).copied().unwrap_or(0) == 0
            && visit_cycle(source.node_id(), nodes, state)
        {
            return true;
        }
    }
    state.insert(node_id.to_owned(), 2);
    false
}

fn escape_pointer(value: &str) -> String {
    value.replace('~', "~0").replace('/', "~1")
}
