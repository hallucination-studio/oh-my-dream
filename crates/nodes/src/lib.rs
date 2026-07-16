//! oh-my-dream concrete nodes.
//!
//! These are the real workflow nodes for the first milestone. They implement
//! the `engine::NodeInterface` trait and consume modality-scoped generation contracts
//! owned by this crate.
//!
//! Wave 2 (Track E) implements the node bodies and a `register_all` that
//! populates an `engine::NodeRegistry`. This crate depends on `engine`,
//! `assets`, which are built first.

#![forbid(unsafe_code)]

mod asset_read_capability;
mod asset_reference;
mod asset_source;
mod contracts;
mod error;
mod generation;
mod generation_capability_execution;
mod generation_profile;
mod image_to_video;
mod image_to_video_capability;
mod literal_text_capability;
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
mod text_to_image_capability;
mod text_to_speech_capability;
mod video_concat;

use assets::AssetStore;
use engine::NodeRegistry;
use std::sync::{Arc, Mutex};

pub use asset_read_capability::*;
pub use asset_reference::{
    AssetMediaKind, AssetReferenceError, AssetReferenceRequest, AssetReferenceResolverInterface,
    ResolvedAssetReference,
};
pub use contracts::{
    CapabilityProjection, CapabilityProjectionError, project_capabilities, project_capability,
};
pub use generation::{
    GeneratedArtifact, GeneratedOutput, GenerationContextInterface, GenerationError,
    ImageToVideoGeneratorInterface, ImageToVideoRequest, InlineMedia, MediaFormat, MediaKind,
    ReferenceImageGenerationRequest, ReferenceImageGeneratorInterface,
    ReferenceVideoGenerationRequest, ReferenceVideoGeneratorInterface,
    TextToAudioGeneratorInterface, TextToAudioRequest, TextToImageGeneratorInterface,
    TextToImageRequest,
};
pub use generation_profile::*;
pub use image_to_video_capability::*;
pub use literal_text_capability::*;
pub use migrations::{
    CapabilityMigrationError, CapabilityNodeResolution, CapabilityNodeStatus,
    DegradedCapabilityReason, frozen_legacy_examples, migrate_legacy_node, resolve_workflow_node,
};
pub use node_capability_media::*;
pub use text_to_image_capability::*;
pub use text_to_speech_capability::*;

/// Shared asset store used by node instances.
///
/// `AssetStore` owns a SQLite connection, which must not be shared directly
/// across concurrent node instances. The mutex serializes store access while
/// still allowing factories and nodes to hold cheap `Arc` clones.
pub type SharedAssetStore = Arc<Mutex<AssetStore>>;

/// Generation capability implementations selected by the composition root.
pub struct GenerationAdapters {
    text_to_image: Arc<dyn TextToImageGeneratorInterface>,
    reference_image: Arc<dyn ReferenceImageGeneratorInterface>,
    reference_video: Arc<dyn ReferenceVideoGeneratorInterface>,
    image_to_video: Arc<dyn ImageToVideoGeneratorInterface>,
    text_to_audio: Arc<dyn TextToAudioGeneratorInterface>,
}

impl GenerationAdapters {
    /// Groups concrete generation adapters for registry construction.
    #[must_use]
    pub fn new(
        text_to_image: Arc<dyn TextToImageGeneratorInterface>,
        reference_image: Arc<dyn ReferenceImageGeneratorInterface>,
        reference_video: Arc<dyn ReferenceVideoGeneratorInterface>,
        image_to_video: Arc<dyn ImageToVideoGeneratorInterface>,
        text_to_audio: Arc<dyn TextToAudioGeneratorInterface>,
    ) -> Self {
        Self { text_to_image, reference_image, reference_video, image_to_video, text_to_audio }
    }
}

/// Registers all first-milestone node factories into `registry`.
pub fn register_all(
    registry: &mut NodeRegistry,
    generators: GenerationAdapters,
    store: SharedAssetStore,
    asset_resolver: Arc<dyn AssetReferenceResolverInterface>,
) -> Result<(), engine::CapabilityRegistryError> {
    registry::register_all(registry, generators, store, asset_resolver)
}
