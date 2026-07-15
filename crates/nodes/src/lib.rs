//! oh-my-dream concrete nodes.
//!
//! These are the real workflow nodes for the first milestone. They implement
//! the `engine::Node` trait and consume modality-scoped generation contracts
//! owned by this crate.
//!
//! Wave 2 (Track E) implements the node bodies and a `register_all` that
//! populates an `engine::NodeRegistry`. This crate depends on `engine`,
//! `assets`, which are built first.

#![forbid(unsafe_code)]

mod asset_reference;
mod asset_source;
mod contracts;
mod error;
mod generation;
mod generation_profile;
mod image_to_video;
mod media;
mod migrations;
mod node_capability_media;
mod params;
mod ports;
mod reference_image_generation;
mod reference_video_generation;
mod registry;
mod text_prompt;
mod text_to_audio;
mod text_to_image;
mod video_concat;

use assets::AssetStore;
use engine::NodeRegistry;
use std::sync::{Arc, Mutex};

pub use asset_reference::{
    AssetMediaKind, AssetReferenceError, AssetReferenceRequest, AssetReferenceResolver,
    ResolvedAssetReference,
};
pub use contracts::{
    CapabilityProjection, CapabilityProjectionError, project_capabilities, project_capability,
};
pub use generation::{
    GeneratedArtifact, GeneratedOutput, GenerationContext, GenerationError, ImageToVideoGenerator,
    ImageToVideoRequest, InlineMedia, MediaFormat, MediaKind, ReferenceImageGenerationRequest,
    ReferenceImageGenerator, ReferenceVideoGenerationRequest, ReferenceVideoGenerator,
    TextToAudioGenerator, TextToAudioRequest, TextToImageGenerator, TextToImageRequest,
};
pub use generation_profile::*;
pub use migrations::{
    CapabilityMigrationError, CapabilityNodeResolution, CapabilityNodeStatus,
    DegradedCapabilityReason, frozen_legacy_examples, migrate_legacy_node, resolve_workflow_node,
};
pub use node_capability_media::*;

/// Shared asset store used by node instances.
///
/// `AssetStore` owns a SQLite connection, which must not be shared directly
/// across concurrent node instances. The mutex serializes store access while
/// still allowing factories and nodes to hold cheap `Arc` clones.
pub type SharedAssetStore = Arc<Mutex<AssetStore>>;

/// Generation capability implementations selected by the composition root.
pub struct GenerationAdapters {
    text_to_image: Arc<dyn TextToImageGenerator>,
    reference_image: Arc<dyn ReferenceImageGenerator>,
    reference_video: Arc<dyn ReferenceVideoGenerator>,
    image_to_video: Arc<dyn ImageToVideoGenerator>,
    text_to_audio: Arc<dyn TextToAudioGenerator>,
}

impl GenerationAdapters {
    /// Groups concrete generation adapters for registry construction.
    #[must_use]
    pub fn new(
        text_to_image: Arc<dyn TextToImageGenerator>,
        reference_image: Arc<dyn ReferenceImageGenerator>,
        reference_video: Arc<dyn ReferenceVideoGenerator>,
        image_to_video: Arc<dyn ImageToVideoGenerator>,
        text_to_audio: Arc<dyn TextToAudioGenerator>,
    ) -> Self {
        Self { text_to_image, reference_image, reference_video, image_to_video, text_to_audio }
    }
}

/// Registers all first-milestone node factories into `registry`.
pub fn register_all(
    registry: &mut NodeRegistry,
    generators: GenerationAdapters,
    store: SharedAssetStore,
    asset_resolver: Arc<dyn AssetReferenceResolver>,
) -> Result<(), engine::CapabilityRegistryError> {
    registry::register_all(registry, generators, store, asset_resolver)
}
