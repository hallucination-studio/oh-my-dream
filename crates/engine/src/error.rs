//! Error types for the engine layer.
//!
//! Errors carry enough context (node id, port name) to be actionable at the
//! layer that finally handles them. The engine propagates; it never swallows.

use thiserror::Error;

/// Errors raised while building or executing a workflow graph.
#[derive(Debug, Error)]
pub enum EngineError {
    /// The caller cancelled the workflow before execution completed.
    #[error("workflow execution was cancelled")]
    Cancelled,

    /// The workflow uses a format version this engine does not understand.
    #[error("unsupported workflow version `{version}`; expected `1.0`")]
    UnsupportedWorkflowVersion { version: String },

    /// Two workflow entries use the same node id.
    #[error("duplicate node id `{node_id}`")]
    DuplicateNodeId { node_id: String },

    /// A node referenced a type id that is not present in the registry.
    #[error("unknown node type `{type_id}` for node `{node_id}`")]
    UnknownNodeType { node_id: String, type_id: String },

    /// A versioned capability id was present but that exact contract was not registered.
    #[error("unknown capability `{type_id}` version `{contract_version}` for node `{node_id}`")]
    UnknownCapabilityVersion { node_id: String, type_id: String, contract_version: String },

    /// A modality node omitted an applicable selector mode or named an unknown one.
    #[error("invalid capability selector for node `{node_id}` type `{type_id}`: {reason}")]
    InvalidCapabilitySelector { node_id: String, type_id: String, reason: String },

    /// A capability's normalized params could not be decoded or validated.
    #[error(
        "invalid params for capability `{type_id}` version `{contract_version}` on node `{node_id}`"
    )]
    InvalidCapabilityParams {
        node_id: String,
        type_id: String,
        contract_version: String,
        #[source]
        source: crate::node::NodeRunError,
    },

    /// The executable node did not match its registered immutable contract.
    #[error("capability `{type_id}` does not match its registered contract: {message}")]
    CapabilityContractMismatch { type_id: String, message: String },

    /// A wire referenced a source node that does not exist in the graph.
    #[error("node `{node_id}` input `{input}` references unknown source node `{source_node}`")]
    UnknownSourceNode { node_id: String, input: String, source_node: String },

    /// A wire referenced an output name the source node does not declare.
    #[error(
        "node `{node_id}` input `{input}` references unknown output `{output}` on node `{source_node}`"
    )]
    UnknownSourceOutput { node_id: String, input: String, source_node: String, output: String },

    /// A wire targets an input the destination node does not declare.
    #[error("node `{node_id}` wiring targets undeclared input `{input}`")]
    UnknownTargetInput { node_id: String, input: String },

    /// A wire connected two ports whose types do not match.
    #[error(
        "type mismatch wiring node `{source_node}` output `{output}` ({source_type:?}) \
         into node `{node_id}` input `{input}` ({input_type:?})"
    )]
    TypeMismatch {
        node_id: String,
        input: String,
        input_type: crate::port::PortType,
        source_node: String,
        output: String,
        source_type: crate::port::PortType,
    },

    /// A required input on a node was left unconnected and has no default.
    #[error("node `{node_id}` is missing required input `{input}`")]
    MissingRequiredInput { node_id: String, input: String },

    /// A declared input default has a different runtime type from its port.
    #[error(
        "node `{node_id}` default for input `{input}` expected {input_type:?} but was {default_type:?}"
    )]
    DefaultTypeMismatch {
        node_id: String,
        input: String,
        input_type: crate::port::PortType,
        default_type: crate::port::PortType,
    },

    /// A node did not return one of its declared outputs.
    #[error("node `{node_id}` missing declared output `{output}`")]
    MissingNodeOutput { node_id: String, output: String },

    /// A node returned an output it did not declare.
    #[error("node `{node_id}` produced undeclared output `{output}`")]
    UnexpectedNodeOutput { node_id: String, output: String },

    /// A node returned a value whose runtime type differs from its output port.
    #[error(
        "node `{node_id}` output `{output}` expected {output_type:?} but produced {actual_type:?}"
    )]
    OutputTypeMismatch {
        node_id: String,
        output: String,
        output_type: crate::port::PortType,
        actual_type: crate::port::PortType,
    },

    /// The graph contains a cycle and cannot be ordered for execution.
    #[error("workflow graph contains a cycle involving node `{node_id}`")]
    Cycle { node_id: String },

    /// The workflow shape could not be prepared for execution.
    #[error("invalid workflow: {message}")]
    InvalidWorkflow { message: String },

    /// A node's own execution failed. Wraps the underlying cause with context.
    #[error("execution of node `{node_id}` (`{type_id}`) failed: {source}")]
    NodeExecution {
        node_id: String,
        type_id: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}

/// Convenient result alias for engine operations.
pub type EngineResult<T> = std::result::Result<T, EngineError>;
