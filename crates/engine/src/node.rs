//! The [`Node`] trait and port declarations.
//!
//! A node mirrors ComfyUI's INPUT_TYPES / RETURN_TYPES / FUNCTION idea in
//! idiomatic Rust: it declares typed input and output ports and exposes a
//! synchronous execution method. Execution here is deliberately synchronous —
//! the engine is pure logic, and concrete node implementations consume
//! synchronous capability contracts whose adapters own any async cloud work.

use crate::error::EngineError;
use crate::executor::{CancellationSignalInterface, NodeExecutionState, NodeProgressEvent};
use crate::port::{PortCardinality, PortType};
use crate::value::{NodeInputs, ValueMap, WorkflowNodeValue};

/// Declaration of a single input port on a node.
#[derive(Debug, Clone)]
pub struct InputPort {
    /// Port name, unique within the node's inputs.
    pub name: String,
    /// The data type this port accepts.
    pub port_type: PortType,
    /// Number of values the input accepts.
    pub cardinality: PortCardinality,
    /// Whether the port must be satisfied (by a wire or a default) to run.
    pub required: bool,
    /// Optional default value used when the port is left unconnected.
    pub default: Option<WorkflowNodeValue>,
}

/// Declaration of a single output port on a node.
#[derive(Debug, Clone)]
pub struct OutputPort {
    /// Port name, unique within the node's outputs.
    pub name: String,
    /// The data type this port produces.
    pub port_type: PortType,
}

/// A unit of work in a workflow graph.
///
/// Implementations are constructed from their serialized `params` by a factory
/// registered in the [`crate::registry::NodeRegistry`].
pub trait NodeInterface: Send + Sync {
    /// Stable identifier of this node's type (matches the workflow `type`).
    fn type_id(&self) -> &str;

    /// The input ports this node declares.
    fn inputs(&self) -> &[InputPort];

    /// The output ports this node declares.
    fn outputs(&self) -> &[OutputPort];

    /// Executes the node with fully resolved `inputs`, returning its outputs.
    ///
    /// The executor guarantees that every required input is present and
    /// type-checked before calling this. Implementations return an error
    /// (boxed) rather than panicking; the executor wraps it with node context.
    /// A successful return is the node commit point: implementations must
    /// observe cancellation before starting irreversible side effects.
    fn run(
        &self,
        inputs: &NodeInputs,
        context: &mut NodeRunContextImpl<'_>,
    ) -> std::result::Result<NodeRunResult, NodeRunError>;

    /// Looks up an output port declaration by name.
    fn output_port(&self, name: &str) -> Option<&OutputPort> {
        self.outputs().iter().find(|port| port.name == name)
    }

    /// Looks up an input port declaration by name.
    fn input_port(&self, name: &str) -> Option<&InputPort> {
        self.inputs().iter().find(|port| port.name == name)
    }
}

/// Error returned by a node's own [`Node::run`] implementation.
///
/// The executor converts this into [`EngineError::NodeExecution`], attaching
/// the node id and type id so the failure is actionable higher up.
pub type NodeRunError = Box<dyn std::error::Error + Send + Sync>;

#[derive(Debug, thiserror::Error)]
#[error("node execution was cancelled")]
struct NodeRunCancelled;

/// Creates the structured node outcome used when cancellation was observed.
#[must_use]
pub fn cancelled_node_run() -> NodeRunError {
    Box::new(NodeRunCancelled)
}

pub(crate) fn is_cancelled_node_run(error: &NodeRunError) -> bool {
    error.downcast_ref::<NodeRunCancelled>().is_some()
}

/// Result returned by a node run.
#[derive(Debug, Clone, PartialEq)]
pub struct NodeRunResult {
    /// Named output values produced by the node.
    pub outputs: ValueMap,
    /// Estimated cost in micro-USD.
    pub cost: Option<i64>,
}

impl NodeRunResult {
    /// Creates a zero-cost result from output values.
    #[must_use]
    pub fn new(outputs: ValueMap) -> Self {
        Self { outputs, cost: None }
    }
}

/// Synchronous context passed into a running node.
pub struct NodeRunContextImpl<'a> {
    node_id: &'a str,
    project_id: &'a str,
    workflow_snapshot: &'a serde_json::Value,
    cancellation: &'a dyn CancellationSignalInterface,
    observer: &'a mut dyn FnMut(&NodeProgressEvent),
}

impl<'a> NodeRunContextImpl<'a> {
    /// Creates a context for `node_id`.
    pub(crate) fn new(
        node_id: &'a str,
        project_id: &'a str,
        workflow_snapshot: &'a serde_json::Value,
        cancellation: &'a dyn CancellationSignalInterface,
        observer: &'a mut dyn FnMut(&NodeProgressEvent),
    ) -> Self {
        Self { node_id, project_id, workflow_snapshot, cancellation, observer }
    }

    /// Current workflow node id.
    #[must_use]
    pub fn node_id(&self) -> &str {
        self.node_id
    }

    /// Project id on the current workflow.
    #[must_use]
    pub fn project_id(&self) -> &str {
        self.project_id
    }

    /// Serialized snapshot of the workflow currently being executed.
    #[must_use]
    pub fn workflow_snapshot(&self) -> &serde_json::Value {
        self.workflow_snapshot
    }

    /// Returns whether the caller has cancelled the current workflow run.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancellation.is_cancelled()
    }

    /// Emits best-effort progress for the current node.
    pub fn progress(&mut self, progress: f32) {
        if self.is_cancelled() {
            return;
        }
        (self.observer)(&NodeProgressEvent {
            node_id: self.node_id.to_owned(),
            state: NodeExecutionState::Running,
            progress: Some(progress),
            cost: None,
        });
    }
}

impl From<(&str, &str, NodeRunError)> for EngineError {
    fn from((node_id, type_id, source): (&str, &str, NodeRunError)) -> Self {
        EngineError::NodeExecution {
            node_id: node_id.to_owned(),
            type_id: type_id.to_owned(),
            source,
        }
    }
}
