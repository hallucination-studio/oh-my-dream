//! oh-my-dream workflow engine.
//!
//! This layer only concerns "how the node graph is organized and executed".
//! It knows nothing about what individual nodes do, nor about UI and network.
//! Inference backends, the asset library, and the interface all live outside
//! the engine.

#![forbid(unsafe_code)]

mod cache;
pub mod capability;
pub mod error;
pub mod executor;
pub mod graph;
pub mod node;
pub mod node_capability;
pub mod port;
pub mod registry;
mod validation;
pub mod value;
pub mod workflow;
pub mod workflow_graph;
pub mod workflow_patch;

pub use cache::ResultCache;
pub use capability::{
    CapabilityContract, CapabilityEffect, CapabilityPort, CapabilityPresentation, CapabilityRef,
    CapabilityRegistration, CapabilityRegistry, CapabilityRegistryError, CapabilitySelector,
    ContextualCreation, DEFAULT_CAPABILITY_VERSION,
};
pub use error::{EngineError, Result};
pub use executor::{
    CancellationSignalInterface, Executor, NodeExecutionState, NodeProgressEvent, RunOutputs,
};
pub use graph::{InputBinding, OutputRef, Workflow, WorkflowNode};
pub use node::{
    InputPort, NodeInterface, NodeRunContextImpl, NodeRunError, NodeRunResult, OutputPort,
    cancelled_node_run,
};
pub use port::{PortCardinality, PortType};
pub use registry::{NodeFactory, NodeParams, NodeRegistry};
pub use value::{InputValue, NodeInputs, Value, ValueMap};
pub use workflow_patch::{
    MAX_WORKFLOW_PATCH_BYTES, MAX_WORKFLOW_PATCH_OPERATIONS, NodeRef, PatchOutputRef,
    WorkflowDiagnostic, WorkflowPatch, WorkflowPatchError, WorkflowPatchOperation,
    WorkflowPatchResult, WorkflowReadinessBlocker, WorkflowValidationReport, apply_workflow_patch,
    validate_workflow,
};
