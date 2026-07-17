use std::sync::Arc;
use std::time::{Duration, Instant};

use backends::deterministic_provider::DeterministicTextToImageProviderRouteImpl;
use backends::provider_routing::{
    ImageToVideoProviderRouterImpl, ProviderRouterGenerationProfileAvailabilityReaderAdapterImpl,
    TextToImageProviderRouteInterface, TextToImageProviderRouterImpl,
    TextToSpeechProviderRouterImpl,
};
use engine::node_capability::{
    NodeCapabilityContractId, NodeCapabilityContractRef, NodeCapabilityContractVersion,
};
use nodes::{
    GenerationProfileAvailabilityReaderInterface, GenerationProfileAvailabilityRequest,
    GenerationProfileAvailabilityState, GenerationProfileId, GenerationProfileRef,
    GenerationProfileUnavailableReason, GenerationProfileVersion,
};

#[tokio::test]
async fn adapter_reads_the_exact_router_map_and_preserves_request_order() {
    let image_profile = profile("image.high_quality_general");
    let adapter = ProviderRouterGenerationProfileAvailabilityReaderAdapterImpl::new(
        Arc::new(
            TextToImageProviderRouterImpl::try_new([(
                image_profile.clone(),
                Arc::new(DeterministicTextToImageProviderRouteImpl::try_new().unwrap())
                    as Arc<dyn TextToImageProviderRouteInterface>,
            )])
            .unwrap(),
        ),
        Arc::new(ImageToVideoProviderRouterImpl::try_new([]).unwrap()),
        Arc::new(TextToSpeechProviderRouterImpl::try_new([]).unwrap()),
    );

    let observations = adapter
        .read_generation_profile_availability(
            GenerationProfileAvailabilityRequest::try_new(
                capability("image.generate_from_text"),
                vec![image_profile.clone()],
                Instant::now() + Duration::from_secs(4),
            )
            .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(observations.len(), 1);
    assert_eq!(observations[0].profile_ref(), &image_profile);
    assert!(matches!(observations[0].state(), GenerationProfileAvailabilityState::Available));
}

#[tokio::test]
async fn absent_router_entry_is_reported_as_no_configured_route() {
    let adapter = ProviderRouterGenerationProfileAvailabilityReaderAdapterImpl::new(
        Arc::new(TextToImageProviderRouterImpl::try_new([]).unwrap()),
        Arc::new(ImageToVideoProviderRouterImpl::try_new([]).unwrap()),
        Arc::new(TextToSpeechProviderRouterImpl::try_new([]).unwrap()),
    );
    let observations = adapter
        .read_generation_profile_availability(
            GenerationProfileAvailabilityRequest::try_new(
                capability("audio.synthesize_speech_from_text"),
                vec![profile("speech.multilingual_narration")],
                Instant::now() + Duration::from_secs(4),
            )
            .unwrap(),
        )
        .await
        .unwrap();

    assert!(matches!(
        observations[0].state(),
        GenerationProfileAvailabilityState::Unavailable {
            reason: GenerationProfileUnavailableReason::NoConfiguredRoute,
            retry_after: None,
        }
    ));
}

fn profile(id: &str) -> GenerationProfileRef {
    GenerationProfileRef::new(
        GenerationProfileId::try_new(id).unwrap(),
        GenerationProfileVersion::try_new(1).unwrap(),
    )
}

fn capability(id: &str) -> NodeCapabilityContractRef {
    NodeCapabilityContractRef::new(
        NodeCapabilityContractId::new(id).unwrap(),
        NodeCapabilityContractVersion::new(1, 0).unwrap(),
    )
}
