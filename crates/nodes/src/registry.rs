//! Concrete capability registrations owned by the nodes crate.

use crate::{SharedAssetStore, TextToAudioGenerator};
use engine::{CapabilityRegistryError, NodeRegistry};
use std::sync::Arc;

/// Registers the versioned co-author capability set and the legacy audio node.
pub(crate) fn register_all(
    registry: &mut NodeRegistry,
    text_to_image_generator: Arc<dyn crate::TextToImageGenerator>,
    image_to_video_generator: Arc<dyn crate::ImageToVideoGenerator>,
    text_to_audio_generator: Arc<dyn TextToAudioGenerator>,
    store: SharedAssetStore,
) -> Result<(), CapabilityRegistryError> {
    registry.register_capability(crate::text_prompt::registration())?;
    registry.register_capability(crate::text_to_image::registration(
        text_to_image_generator,
        Arc::clone(&store),
    ))?;
    registry.register_capability(crate::image_to_video::registration(
        image_to_video_generator,
        Arc::clone(&store),
    ))?;
    registry.register_capability(crate::video_concat::registration())?;
    crate::text_to_audio::register(registry, text_to_audio_generator, store);
    Ok(())
}
