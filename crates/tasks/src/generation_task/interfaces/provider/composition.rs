//! Provider-level composition over complete focused capabilities.

use std::collections::BTreeSet;
use std::sync::Arc;

use super::{
    GenerationProviderContract, GenerationProviderContractError, GenerationProviderDisplayName,
    GenerationProviderRouteResolutionError, ImageGenerationProviderContract,
    ImageGenerationProviderExecution, TextGenerationProviderContract,
    TextGenerationProviderExecution, VideoGenerationProviderContract,
    VideoGenerationProviderExecution, VoiceGenerationProviderContract,
    VoiceGenerationProviderExecution,
};
use crate::generation_task::domain::{GenerationProviderId, GenerationProviderRouteId};

/// Complete focused Text-generation provider capability.
pub trait TextGenerationProviderInterface: Send + Sync {
    /// Returns the immutable non-empty Text route contract.
    fn text_generation_contract(&self) -> &TextGenerationProviderContract;

    /// Resolves one exact shipped Text route without fallback.
    fn resolve_text_generation_route(
        &self,
        route_id: &GenerationProviderRouteId,
    ) -> Result<TextGenerationProviderExecution, GenerationProviderRouteResolutionError>;
}

/// Complete focused Image-generation provider capability.
pub trait ImageGenerationProviderInterface: Send + Sync {
    /// Returns the immutable non-empty Image route contract.
    fn image_generation_contract(&self) -> &ImageGenerationProviderContract;

    /// Resolves one exact shipped Image route without fallback.
    fn resolve_image_generation_route(
        &self,
        route_id: &GenerationProviderRouteId,
    ) -> Result<ImageGenerationProviderExecution, GenerationProviderRouteResolutionError>;
}

/// Complete focused Video-generation provider capability.
pub trait VideoGenerationProviderInterface: Send + Sync {
    /// Returns the immutable non-empty Video route contract.
    fn video_generation_contract(&self) -> &VideoGenerationProviderContract;

    /// Resolves one exact shipped Video route without fallback.
    fn resolve_video_generation_route(
        &self,
        route_id: &GenerationProviderRouteId,
    ) -> Result<VideoGenerationProviderExecution, GenerationProviderRouteResolutionError>;
}

/// Complete focused Voice-generation provider capability.
pub trait VoiceGenerationProviderInterface: Send + Sync {
    /// Returns the immutable non-empty Voice route contract.
    fn voice_generation_contract(&self) -> &VoiceGenerationProviderContract;

    /// Resolves one exact shipped Voice route without fallback.
    fn resolve_voice_generation_route(
        &self,
        route_id: &GenerationProviderRouteId,
    ) -> Result<VoiceGenerationProviderExecution, GenerationProviderRouteResolutionError>;
}

/// Immutable non-empty product of complete focused provider capabilities.
pub struct GenerationProviderCapabilities {
    text: Option<Arc<dyn TextGenerationProviderInterface>>,
    image: Option<Arc<dyn ImageGenerationProviderInterface>>,
    video: Option<Arc<dyn VideoGenerationProviderInterface>>,
    voice: Option<Arc<dyn VoiceGenerationProviderInterface>>,
}

impl GenerationProviderCapabilities {
    /// Validates non-empty composition, route uniqueness, and exact route resolution.
    pub fn try_new(
        text: Option<Arc<dyn TextGenerationProviderInterface>>,
        image: Option<Arc<dyn ImageGenerationProviderInterface>>,
        video: Option<Arc<dyn VideoGenerationProviderInterface>>,
        voice: Option<Arc<dyn VoiceGenerationProviderInterface>>,
    ) -> Result<Self, GenerationProviderContractError> {
        if text.is_none() && image.is_none() && video.is_none() && voice.is_none() {
            return Err(GenerationProviderContractError::EmptyCapabilities);
        }
        validate_declared_routes(&text, &image, &video, &voice)?;
        Ok(Self { text, image, video, voice })
    }

    /// Returns the complete Text capability when contributed.
    #[must_use]
    pub fn text(&self) -> Option<&Arc<dyn TextGenerationProviderInterface>> {
        self.text.as_ref()
    }

    /// Returns the complete Image capability when contributed.
    #[must_use]
    pub fn image(&self) -> Option<&Arc<dyn ImageGenerationProviderInterface>> {
        self.image.as_ref()
    }

    /// Returns the complete Video capability when contributed.
    #[must_use]
    pub fn video(&self) -> Option<&Arc<dyn VideoGenerationProviderInterface>> {
        self.video.as_ref()
    }

    /// Returns the complete Voice capability when contributed.
    #[must_use]
    pub fn voice(&self) -> Option<&Arc<dyn VoiceGenerationProviderInterface>> {
        self.voice.as_ref()
    }
}

/// Provider-level identity and immutable complete capability composition.
pub trait GenerationProviderInterface: Send + Sync {
    /// Returns the stable provider identity.
    fn generation_provider_id(&self) -> &GenerationProviderId;

    /// Returns the safe provider display name.
    fn generation_provider_display_name(&self) -> &GenerationProviderDisplayName;

    /// Returns the immutable non-empty focused capability product.
    fn generation_provider_capabilities(&self) -> &GenerationProviderCapabilities;
}

impl GenerationProviderContract {
    /// Mechanically derives the safe contract projection from one provider implementation.
    #[must_use]
    pub fn from_provider(provider: &dyn GenerationProviderInterface) -> Self {
        let capabilities = provider.generation_provider_capabilities();
        Self::new(
            provider.generation_provider_id().clone(),
            provider.generation_provider_display_name().clone(),
            capabilities.text().map(|value| value.text_generation_contract().clone()),
            capabilities.image().map(|value| value.image_generation_contract().clone()),
            capabilities.video().map(|value| value.video_generation_contract().clone()),
            capabilities.voice().map(|value| value.voice_generation_contract().clone()),
        )
    }
}

fn validate_declared_routes(
    text: &Option<Arc<dyn TextGenerationProviderInterface>>,
    image: &Option<Arc<dyn ImageGenerationProviderInterface>>,
    video: &Option<Arc<dyn VideoGenerationProviderInterface>>,
    voice: &Option<Arc<dyn VoiceGenerationProviderInterface>>,
) -> Result<(), GenerationProviderContractError> {
    let mut route_ids = BTreeSet::new();
    if let Some(capability) = text {
        validate_routes(
            capability.text_generation_contract().routes(),
            &mut route_ids,
            |route_id| capability.resolve_text_generation_route(route_id).map(|_| ()),
        )?;
    }
    if let Some(capability) = image {
        validate_routes(
            capability.image_generation_contract().routes(),
            &mut route_ids,
            |route_id| capability.resolve_image_generation_route(route_id).map(|_| ()),
        )?;
    }
    if let Some(capability) = video {
        validate_routes(
            capability.video_generation_contract().routes(),
            &mut route_ids,
            |route_id| capability.resolve_video_generation_route(route_id).map(|_| ()),
        )?;
    }
    if let Some(capability) = voice {
        validate_routes(
            capability.voice_generation_contract().routes(),
            &mut route_ids,
            |route_id| capability.resolve_voice_generation_route(route_id).map(|_| ()),
        )?;
    }
    Ok(())
}

fn validate_routes(
    routes: &[super::GenerationProviderRouteContract],
    route_ids: &mut BTreeSet<GenerationProviderRouteId>,
    mut resolve: impl FnMut(
        &GenerationProviderRouteId,
    ) -> Result<(), GenerationProviderRouteResolutionError>,
) -> Result<(), GenerationProviderContractError> {
    for route in routes {
        if !route_ids.insert(route.route_id().clone()) {
            return Err(GenerationProviderContractError::DuplicateRouteId);
        }
        resolve(route.route_id())
            .map_err(|_| GenerationProviderContractError::RouteResolutionMismatch)?;
    }
    Ok(())
}
