use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use engine::node_capability::{
    NodeCapabilityProviderFailure, WorkflowNodeExecutionContext, WorkflowTextValue,
};
use nodes::{
    GenerationProfileRef, SynthesizedSpeechPayload, TextToSpeechProviderInterface,
    TextToSpeechProviderRequest,
};

use super::{
    GenerationProviderRouteAvailability, GenerationProviderRouteId,
    GenerationProviderRouterConstructionError, no_configured_route_availability,
    no_configured_route_failure, validate_routes,
};

/// Profile-free semantic request delegated to one text-to-speech route.
pub struct TextToSpeechProviderRouteRequest {
    context: WorkflowNodeExecutionContext,
    text: WorkflowTextValue,
}

impl TextToSpeechProviderRouteRequest {
    /// Returns execution identity, deadline, and cancellation unchanged.
    #[must_use]
    pub const fn context(&self) -> &WorkflowNodeExecutionContext {
        &self.context
    }
    /// Returns the normalized speech text unchanged.
    #[must_use]
    pub const fn text(&self) -> &WorkflowTextValue {
        &self.text
    }

    /// Consumes the routed request into every provider-independent field.
    #[must_use]
    pub fn into_parts(self) -> (WorkflowNodeExecutionContext, WorkflowTextValue) {
        (self.context, self.text)
    }
}

/// Private exact route boundary implemented by text-to-speech adapters.
#[async_trait]
pub trait TextToSpeechProviderRouteInterface: Send + Sync {
    /// Returns the fixed identity selected before provider submission.
    fn generation_provider_route_id(&self) -> GenerationProviderRouteId;
    /// Observes current availability without changing route selection.
    async fn observe_provider_route_availability(&self) -> GenerationProviderRouteAvailability;
    /// Synthesizes one validated Audio payload through this exact route.
    async fn synthesize_speech_from_text(
        &self,
        request: TextToSpeechProviderRouteRequest,
    ) -> Result<SynthesizedSpeechPayload, NodeCapabilityProviderFailure>;
}

/// Exact Generation Profile router for text-to-speech execution.
pub struct TextToSpeechProviderRouterImpl {
    routes_by_profile: BTreeMap<GenerationProfileRef, Arc<dyn TextToSpeechProviderRouteInterface>>,
    no_configured_route_failure: NodeCapabilityProviderFailure,
}

impl TextToSpeechProviderRouterImpl {
    /// Validates unique, known, operation-compatible profile-to-route mappings.
    pub fn try_new(
        routes: impl IntoIterator<
            Item = (GenerationProfileRef, Arc<dyn TextToSpeechProviderRouteInterface>),
        >,
    ) -> Result<Self, GenerationProviderRouterConstructionError> {
        Ok(Self {
            routes_by_profile: validate_routes(
                routes,
                "audio.synthesize_speech_from_text",
                TextToSpeechProviderRouteInterface::generation_provider_route_id,
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
impl TextToSpeechProviderInterface for TextToSpeechProviderRouterImpl {
    async fn synthesize_speech_from_text(
        &self,
        request: TextToSpeechProviderRequest,
    ) -> Result<SynthesizedSpeechPayload, NodeCapabilityProviderFailure> {
        let (profile_ref, context, text) = request.into_parts();
        let route = self
            .routes_by_profile
            .get(&profile_ref)
            .ok_or_else(|| self.no_configured_route_failure.clone())?;
        route.synthesize_speech_from_text(TextToSpeechProviderRouteRequest { context, text }).await
    }
}
