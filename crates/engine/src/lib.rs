//! oh-my-dream workflow engine.
//!
//! This layer only concerns "how the node graph is organized and executed".
//! It knows nothing about what individual nodes do, nor about UI and network.
//! Inference backends, the asset library, and the interface all live outside
//! the engine.

#![forbid(unsafe_code)]

pub mod error;
pub mod executor;
pub mod graph;
pub mod node;
pub mod port;
pub mod registry;
pub mod value;

pub use error::{EngineError, Result};
pub use executor::{Executor, ResultCache, RunOutputs};
pub use graph::{OutputRef, Workflow, WorkflowNode};
pub use node::{InputPort, Node, NodeRunError, OutputPort};
pub use port::PortType;
pub use registry::{NodeFactory, NodeParams, NodeRegistry};
pub use value::{Value, ValueMap};
