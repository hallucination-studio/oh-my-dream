// Original integration modules intentionally own isolated copies of shared test fixtures.
#![allow(clippy::duplicate_mod)]

#[path = "active_node_capability_behavior.rs"]
mod active_node_capability_behavior;
#[path = "active_node_capability_contracts.rs"]
mod active_node_capability_contracts;
#[path = "c3_capability_contracts.rs"]
mod c3_capability_contracts;
#[path = "generation_capabilities.rs"]
mod generation_capabilities;
#[path = "generation_capability_faults.rs"]
mod generation_capability_faults;
#[path = "generation_capability_interruptions.rs"]
mod generation_capability_interruptions;
#[path = "generation_capability_readiness.rs"]
mod generation_capability_readiness;
#[path = "generation_profile_availability.rs"]
mod generation_profile_availability;
#[path = "generation_profile_catalog.rs"]
mod generation_profile_catalog;
#[path = "generation_task_start_contract.rs"]
mod generation_task_start_contract;
#[path = "image_to_video_capability_faults.rs"]
mod image_to_video_capability_faults;
#[path = "image_to_video_capability_mapping.rs"]
mod image_to_video_capability_mapping;
#[path = "literal_and_asset_read_capabilities.rs"]
mod literal_and_asset_read_capabilities;
#[path = "node_capability_architecture.rs"]
mod node_capability_architecture;
#[path = "node_capability_external_interface_contracts.rs"]
mod node_capability_external_interface_contracts;
#[path = "node_capability_provider_interface_contracts.rs"]
mod node_capability_provider_interface_contracts;
#[path = "text_generation_capability_mapping.rs"]
mod text_generation_capability_mapping;
