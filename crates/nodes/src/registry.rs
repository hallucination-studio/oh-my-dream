//! Concrete capability registrations owned by the nodes crate.

use crate::{AssetReferenceResolverInterface, SharedAssetStore};
use engine::{CapabilityRegistryError, NodeRegistry};
use std::sync::Arc;

/// Registers the versioned co-author capability set.
pub(crate) fn register_all(
    registry: &mut NodeRegistry,
    generators: crate::GenerationAdapters,
    store: SharedAssetStore,
    asset_resolver: Arc<dyn AssetReferenceResolverInterface>,
) -> Result<(), CapabilityRegistryError> {
    let crate::GenerationAdapters {
        text_to_image,
        reference_image,
        reference_video,
        image_to_video,
        text_to_audio,
    } = generators;
    for registration in crate::asset_source::registrations(Arc::clone(&asset_resolver)) {
        registry.register_selector_capability(registration)?;
    }
    registry.register_selector_capability(crate::text_prompt::registration())?;
    registry.register_selector_capability(crate::text_to_image::registration(
        text_to_image,
        Arc::clone(&store),
    ))?;
    registry.register_selector_capability(crate::reference_image_generation::registration(
        reference_image,
        Arc::clone(&store),
        Arc::clone(&asset_resolver),
    ))?;
    registry.register_selector_capability(crate::reference_video_generation::registration(
        reference_video,
        Arc::clone(&store),
        Arc::clone(&asset_resolver),
    ))?;
    registry.register_selector_capability(crate::image_to_video::registration(
        image_to_video,
        Arc::clone(&store),
        asset_resolver,
    ))?;
    registry.register_selector_capability(crate::video_concat::registration())?;
    registry
        .register_selector_capability(crate::text_to_audio::registration(text_to_audio, store))?;
    Ok(())
}
