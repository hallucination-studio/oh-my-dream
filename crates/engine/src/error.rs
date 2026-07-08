//! Error types for the engine layer.
//!
//! Errors carry enough context (node id, port name) to be actionable at the
//! layer that finally handles them. The engine propagates; it never swallows.

use thiserror::Error;

/// Errors raised while building or executing a workflow graph.
#[derive(Debug, Error)]
pub enum EngineError {
    /// A node referenced a type id that is not present in the registry.
    #[error("unknown node type `{type_id}` for node `{node_id}`")]
    UnknownNodeType { node_id: String, type_id: String },

    /// A wire referenced a source node that does not exist in the graph.
    #[error("node `{node_id}` input `{input}` references unknown source node `{source_node}`")]
    UnknownSourceNode { node_id: String, input: String, source_node: String },

    /// A wire referenced an output name the source node does not declare.
    #[error(
        "node `{node_id}` input `{input}` references unknown output `{output}` on node `{source_node}`"
    )]
    UnknownSourceOutput { node_id: String, input: String, source_node: String, output: String },

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
pub type Result<T> = std::result::Result<T, EngineError>;
