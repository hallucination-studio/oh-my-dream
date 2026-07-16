// Original integration modules intentionally own isolated copies of shared test fixtures.
#![allow(clippy::duplicate_mod)]

#[path = "capability_registry.rs"]
mod capability_registry;
#[path = "executor.rs"]
mod executor;
#[path = "node_capability_architecture.rs"]
mod node_capability_architecture;
#[path = "node_capability_contract.rs"]
mod node_capability_contract;
#[path = "node_capability_execution.rs"]
mod node_capability_execution;
#[path = "node_capability_parameter_values.rs"]
mod node_capability_parameter_values;
#[path = "node_capability_registry.rs"]
mod node_capability_registry;
#[path = "node_capability_runtime_values.rs"]
mod node_capability_runtime_values;
#[path = "validation.rs"]
mod validation;
#[path = "workflow_graph_architecture.rs"]
mod workflow_graph_architecture;
#[path = "workflow_graph_checkpoint.rs"]
mod workflow_graph_checkpoint;
#[path = "workflow_graph_reconstruction.rs"]
mod workflow_graph_reconstruction;
#[path = "workflow_graph_values.rs"]
mod workflow_graph_values;
#[path = "workflow_mutation_apply.rs"]
mod workflow_mutation_apply;
#[path = "workflow_mutation_command.rs"]
mod workflow_mutation_command;
#[path = "workflow_mutation_receipt.rs"]
mod workflow_mutation_receipt;
#[path = "workflow_patch.rs"]
mod workflow_patch;
#[path = "workflow_run_domain.rs"]
mod workflow_run_domain;
