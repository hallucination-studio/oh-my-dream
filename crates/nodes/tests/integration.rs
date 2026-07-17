// Original integration modules intentionally own isolated copies of shared test fixtures.
#![allow(clippy::duplicate_mod)]

#[path = "active_node_capability_behavior.rs"]
mod active_node_capability_behavior;
#[path = "active_node_capability_contracts.rs"]
mod active_node_capability_contracts;
#[path = "architecture.rs"]
mod architecture;
#[path = "c3_capability_contracts.rs"]
mod c3_capability_contracts;
#[path = "capability_contracts.rs"]
mod capability_contracts;
#[path = "capability_projection.rs"]
mod capability_projection;
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
#[path = "image_to_video_capability_faults.rs"]
mod image_to_video_capability_faults;
#[path = "image_to_video_capability_mapping.rs"]
mod image_to_video_capability_mapping;
#[path = "literal_and_asset_read_capabilities.rs"]
mod literal_and_asset_read_capabilities;
#[path = "media_boundary.rs"]
mod media_boundary;
#[path = "node_capability_architecture.rs"]
mod node_capability_architecture;
#[path = "node_capability_external_interface_contracts.rs"]
mod node_capability_external_interface_contracts;
#[path = "node_capability_provider_interface_contracts.rs"]
mod node_capability_provider_interface_contracts;
#[path = "reference_image_generation.rs"]
mod reference_image_generation;
#[path = "reference_video_generation.rs"]
mod reference_video_generation;
#[path = "text_generation_capability_mapping.rs"]
mod text_generation_capability_mapping;
#[path = "workflow.rs"]
mod workflow;
#[path = "workflow_selector_patch.rs"]
mod workflow_selector_patch;
