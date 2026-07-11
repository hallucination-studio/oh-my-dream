//! oh-my-dream workflow engine.
//!
//! This layer only concerns "how the node graph is organized and executed".
//! It knows nothing about what individual nodes do, nor about UI and network.
//! Inference backends, the asset library, and the interface all live outside
//! the engine.

#![forbid(unsafe_code)]

mod cache;
pub mod error;
pub mod executor;
pub mod graph;
pub mod node;
pub mod port;
pub mod registry;
mod validation;
pub mod value;

pub use cache::ResultCache;
pub use error::{EngineError, Result};
pub use executor::{
    CancellationSignal, Executor, NodeExecutionState, NodeProgressEvent, RunOutputs,
};
pub use graph::{OutputRef, Workflow, WorkflowNode};
pub use node::{
    InputPort, Node, NodeRunContext, NodeRunError, NodeRunResult, OutputPort, cancelled_node_run,
};
pub use port::PortType;
pub use registry::{NodeFactory, NodeParams, NodeRegistry};
pub use value::{Value, ValueMap};
