use std::sync::Arc;

use async_trait::async_trait;
use backends::provider_routing::{
    GenerationProviderRouteAvailability, GenerationProviderRouteId,
    GenerationProviderRouterConstructionError, ImageToVideoProviderRouteInterface,
    ImageToVideoProviderRouteRequest, ImageToVideoProviderRouterImpl,
    TextToImageProviderRouteInterface, TextToImageProviderRouteRequest,
    TextToImageProviderRouterImpl, TextToSpeechProviderRouteInterface,
    TextToSpeechProviderRouteRequest, TextToSpeechProviderRouterImpl,
};
use engine::node_capability::NodeCapabilityProviderFailure;
use nodes::{
    GeneratedImagePayload, GeneratedVideoPayload, GenerationProfileId, GenerationProfileRef,
    GenerationProfileVersion, SynthesizedSpeechPayload,
};

#[test]
fn exact_operation_routers_accept_their_one_compatible_profile() {
    TextToImageProviderRouterImpl::try_new([(
        profile("image.high_quality_general"),
        Arc::new(TextToImageRoute::new("test.image.primary"))
            as Arc<dyn TextToImageProviderRouteInterface>,
    )])
    .unwrap();
    ImageToVideoProviderRouterImpl::try_new([(
        profile("video.cinematic_image_animation"),
        Arc::new(ImageToVideoRoute::new("test.video.primary"))
            as Arc<dyn ImageToVideoProviderRouteInterface>,
    )])
    .unwrap();
    TextToSpeechProviderRouterImpl::try_new([(
        profile("speech.multilingual_narration"),
        Arc::new(TextToSpeechRoute::new("test.speech.primary"))
            as Arc<dyn TextToSpeechProviderRouteInterface>,
    )])
    .unwrap();
}

#[test]
fn router_rejects_duplicate_profile_mappings_before_building_its_map() {
    let profile_ref = profile("image.high_quality_general");
    let result = TextToImageProviderRouterImpl::try_new([
        (
            profile_ref.clone(),
            Arc::new(TextToImageRoute::new("test.image.first"))
                as Arc<dyn TextToImageProviderRouteInterface>,
        ),
        (
            profile_ref,
            Arc::new(TextToImageRoute::new("test.image.second"))
                as Arc<dyn TextToImageProviderRouteInterface>,
        ),
    ]);

    let Err(error) = result else { panic!("duplicate profile mapping was accepted") };
    assert_eq!(error, GenerationProviderRouterConstructionError::DuplicateProfileMapping);
}

#[test]
fn router_rejects_duplicate_route_ids_even_across_invalid_profile_entries() {
    let result = TextToImageProviderRouterImpl::try_new([
        (
            profile("image.high_quality_general"),
            Arc::new(TextToImageRoute::new("test.shared"))
                as Arc<dyn TextToImageProviderRouteInterface>,
        ),
        (
            profile("video.cinematic_image_animation"),
            Arc::new(TextToImageRoute::new("test.shared"))
                as Arc<dyn TextToImageProviderRouteInterface>,
        ),
    ]);

    let Err(error) = result else { panic!("duplicate route identity was accepted") };
    assert_eq!(error, GenerationProviderRouterConstructionError::DuplicateRouteId);
}

#[test]
fn router_rejects_unknown_and_operation_incompatible_profiles() {
    let unknown = TextToImageProviderRouterImpl::try_new([(
        profile("image.unknown"),
        Arc::new(TextToImageRoute::new("test.image.unknown"))
            as Arc<dyn TextToImageProviderRouteInterface>,
    )]);
    let Err(error) = unknown else { panic!("unknown profile was accepted") };
    assert_eq!(error, GenerationProviderRouterConstructionError::UnknownProfile);

    let incompatible = TextToImageProviderRouterImpl::try_new([(
        profile("video.cinematic_image_animation"),
        Arc::new(TextToImageRoute::new("test.image.incompatible"))
            as Arc<dyn TextToImageProviderRouteInterface>,
    )]);
    let Err(error) = incompatible else { panic!("incompatible profile was accepted") };
    assert_eq!(error, GenerationProviderRouterConstructionError::IncompatibleProfile);
}

fn profile(id: &str) -> GenerationProfileRef {
    GenerationProfileRef::new(
        GenerationProfileId::try_new(id).unwrap(),
        GenerationProfileVersion::try_new(1).unwrap(),
    )
}

macro_rules! route_fake {
    ($name:ident, $interface:ident, $request:ident, $payload:ident, $method:ident) => {
        struct $name(GenerationProviderRouteId);

        impl $name {
            fn new(id: &str) -> Self {
                Self(GenerationProviderRouteId::new(id))
            }
        }

        #[async_trait]
        impl $interface for $name {
            fn generation_provider_route_id(&self) -> GenerationProviderRouteId {
                self.0.clone()
            }

            async fn observe_provider_route_availability(
                &self,
            ) -> GenerationProviderRouteAvailability {
                GenerationProviderRouteAvailability::Available
            }

            async fn $method(
                &self,
                _request: $request,
            ) -> Result<$payload, NodeCapabilityProviderFailure> {
                unimplemented!("route execution belongs to later provider leaves")
            }
        }
    };
}

route_fake!(
    TextToImageRoute,
    TextToImageProviderRouteInterface,
    TextToImageProviderRouteRequest,
    GeneratedImagePayload,
    generate_image_from_text
);
route_fake!(
    ImageToVideoRoute,
    ImageToVideoProviderRouteInterface,
    ImageToVideoProviderRouteRequest,
    GeneratedVideoPayload,
    generate_video_from_image
);
route_fake!(
    TextToSpeechRoute,
    TextToSpeechProviderRouteInterface,
    TextToSpeechProviderRouteRequest,
    SynthesizedSpeechPayload,
    synthesize_speech_from_text
);
