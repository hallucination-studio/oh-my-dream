use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use engine::node_capability::{
    NodeCapabilityProviderFailure, WorkflowNodeExecutionContext, WorkflowTextValue,
};
use nodes::{
    GeneratedImagePayload, GenerationProfileRef, ImageAspectRatio, TextToImageProviderInterface,
    TextToImageProviderRequest,
};

use super::{
    GenerationProviderRouteAvailability, GenerationProviderRouteId,
    GenerationProviderRouterConstructionError, no_configured_route_availability,
    no_configured_route_failure, validate_routes,
};

/// Profile-free semantic request delegated to one text-to-image route.
pub struct TextToImageProviderRouteRequest {
    context: WorkflowNodeExecutionContext,
    prompt: WorkflowTextValue,
    aspect_ratio: ImageAspectRatio,
}

impl TextToImageProviderRouteRequest {
    /// Returns execution identity, deadline, and cancellation unchanged.
    #[must_use]
    pub const fn context(&self) -> &WorkflowNodeExecutionContext {
        &self.context
    }
    /// Returns the normalized semantic prompt unchanged.
    #[must_use]
    pub const fn prompt(&self) -> &WorkflowTextValue {
        &self.prompt
    }
    /// Returns the provider-independent aspect ratio unchanged.
    #[must_use]
    pub const fn aspect_ratio(&self) -> ImageAspectRatio {
        self.aspect_ratio
    }
}

/// Private exact route boundary implemented by text-to-image adapters.
#[async_trait]
pub trait TextToImageProviderRouteInterface: Send + Sync {
    /// Returns the fixed identity selected before provider submission.
    fn generation_provider_route_id(&self) -> GenerationProviderRouteId;
    /// Observes current availability without changing route selection.
    async fn observe_provider_route_availability(&self) -> GenerationProviderRouteAvailability;
    /// Generates one validated Image payload through this exact route.
    async fn generate_image_from_text(
        &self,
        request: TextToImageProviderRouteRequest,
    ) -> Result<GeneratedImagePayload, NodeCapabilityProviderFailure>;
}

/// Exact Generation Profile router for text-to-image execution.
pub struct TextToImageProviderRouterImpl {
    routes_by_profile: BTreeMap<GenerationProfileRef, Arc<dyn TextToImageProviderRouteInterface>>,
    no_configured_route_failure: NodeCapabilityProviderFailure,
}

impl TextToImageProviderRouterImpl {
    /// Validates unique, known, operation-compatible profile-to-route mappings.
    pub fn try_new(
        routes: impl IntoIterator<
            Item = (GenerationProfileRef, Arc<dyn TextToImageProviderRouteInterface>),
        >,
    ) -> Result<Self, GenerationProviderRouterConstructionError> {
        Ok(Self {
            routes_by_profile: validate_routes(
                routes,
                "image.generate_from_text",
                TextToImageProviderRouteInterface::generation_provider_route_id,
            )?,
            no_configured_route_failure: no_configured_route_failure()?,
        })
    }

    /// Reads current state from this router's exact profile-to-route map.
    pub async fn observe_generation_profile_availability(
        &self,
        profile_ref: &GenerationProfileRef,
    ) -> GenerationProviderRouteAvailability {
        match self.routes_by_profile.get(profile_ref) {
            Some(route) => route.observe_provider_route_availability().await,
            None => no_configured_route_availability(),
        }
    }
}

#[async_trait]
impl TextToImageProviderInterface for TextToImageProviderRouterImpl {
    async fn generate_image_from_text(
        &self,
        request: TextToImageProviderRequest,
    ) -> Result<GeneratedImagePayload, NodeCapabilityProviderFailure> {
        let (profile_ref, context, prompt, aspect_ratio) = request.into_parts();
        let route = self
            .routes_by_profile
            .get(&profile_ref)
            .ok_or_else(|| self.no_configured_route_failure.clone())?;
        route
            .generate_image_from_text(TextToImageProviderRouteRequest {
                context,
                prompt,
                aspect_ratio,
            })
            .await
    }
}
