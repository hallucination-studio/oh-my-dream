//! oh-my-dream inference backends.
//!
//! Concrete provider composites and route adapters selected by the Desktop composition root.

#![forbid(unsafe_code)]

pub mod deterministic_provider;
pub mod generation_provider_settings;
pub mod mock_generation_provider;
pub mod provider_routing;
