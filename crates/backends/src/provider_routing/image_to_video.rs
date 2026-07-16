use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use engine::node_capability::{
    NodeCapabilityProviderFailure, WorkflowNodeExecutionContext, WorkflowTextValue,
};
use nodes::{
    GeneratedVideoPayload, GenerationProfileRef, ImageToVideoDurationSeconds,
    ImageToVideoProviderInterface, ImageToVideoProviderRequest, NodeCapabilityReadableImageInput,
};

use super::{
    GenerationProviderRouteAvailability, GenerationProviderRouteId,
    GenerationProviderRouterConstructionError, no_configured_route_failure, validate_routes,
};

/// Profile-free semantic request delegated to one image-to-video route.
pub struct ImageToVideoProviderRouteRequest {
    context: WorkflowNodeExecutionContext,
    image: NodeCapabilityReadableImageInput,
    prompt: Option<WorkflowTextValue>,
    duration_seconds: ImageToVideoDurationSeconds,
}

impl ImageToVideoProviderRouteRequest {
    /// Returns execution identity, deadline, and cancellation unchanged.
    #[must_use]
    pub const fn context(&self) -> &WorkflowNodeExecutionContext {
        &self.context
    }
    /// Returns the exact readable source Image unchanged.
    #[must_use]
    pub const fn image(&self) -> &NodeCapabilityReadableImageInput {
        &self.image
    }
    /// Returns the optional normalized prompt unchanged.
    #[must_use]
    pub fn prompt(&self) -> Option<&WorkflowTextValue> {
        self.prompt.as_ref()
    }
    /// Returns the provider-independent duration unchanged.
    #[must_use]
    pub const fn duration_seconds(&self) -> ImageToVideoDurationSeconds {
        self.duration_seconds
    }
}

/// Private exact route boundary implemented by image-to-video adapters.
#[async_trait]
pub trait ImageToVideoProviderRouteInterface: Send + Sync {
    /// Returns the fixed identity selected before provider submission.
    fn generation_provider_route_id(&self) -> GenerationProviderRouteId;
    /// Observes current availability without changing route selection.
    async fn observe_provider_route_availability(&self) -> GenerationProviderRouteAvailability;
    /// Generates one validated Video payload through this exact route.
    async fn generate_video_from_image(
        &self,
        request: ImageToVideoProviderRouteRequest,
    ) -> Result<GeneratedVideoPayload, NodeCapabilityProviderFailure>;
}

/// Exact Generation Profile router for image-to-video execution.
pub struct ImageToVideoProviderRouterImpl {
    routes_by_profile: BTreeMap<GenerationProfileRef, Arc<dyn ImageToVideoProviderRouteInterface>>,
    no_configured_route_failure: NodeCapabilityProviderFailure,
}

impl ImageToVideoProviderRouterImpl {
    /// Validates unique, known, operation-compatible profile-to-route mappings.
    pub fn try_new(
        routes: impl IntoIterator<
            Item = (GenerationProfileRef, Arc<dyn ImageToVideoProviderRouteInterface>),
        >,
    ) -> Result<Self, GenerationProviderRouterConstructionError> {
        Ok(Self {
            routes_by_profile: validate_routes(
                routes,
                "video.generate_from_image",
                ImageToVideoProviderRouteInterface::generation_provider_route_id,
            )?,
            no_configured_route_failure: no_configured_route_failure()?,
        })
    }
}

#[async_trait]
impl ImageToVideoProviderInterface for ImageToVideoProviderRouterImpl {
    async fn generate_video_from_image(
        &self,
        request: ImageToVideoProviderRequest,
    ) -> Result<GeneratedVideoPayload, NodeCapabilityProviderFailure> {
        let (profile_ref, context, image, prompt, duration_seconds) = request.into_parts();
        let route = self
            .routes_by_profile
            .get(&profile_ref)
            .ok_or_else(|| self.no_configured_route_failure.clone())?;
        route
            .generate_video_from_image(ImageToVideoProviderRouteRequest {
                context,
                image,
                prompt,
                duration_seconds,
            })
            .await
    }
}
