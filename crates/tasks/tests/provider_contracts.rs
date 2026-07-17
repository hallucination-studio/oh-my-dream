use std::collections::BTreeSet;
use std::sync::Arc;

use async_trait::async_trait;
use nodes::{GenerationProfileId, GenerationProfileRef, GenerationProfileVersion};
use tasks::generation_task::domain::*;
use tasks::generation_task::interfaces::*;

use crate::support::time;

#[test]
fn rejects_empty_and_duplicate_route_contracts() {
    assert_eq!(
        GenerationProviderRouteContract::try_new(
            route_id("mock.text.v1"),
            route_name("Text"),
            BTreeSet::new(),
        ),
        Err(GenerationProviderContractError::EmptyCompatibleProfiles)
    );
    assert_eq!(
        TextGenerationProviderContract::try_new(Vec::new()),
        Err(GenerationProviderContractError::EmptyRoutes)
    );
    assert_eq!(
        TextGenerationProviderContract::try_new(vec![
            route("mock.text.v1", "Text A", "text.general"),
            route("mock.text.v1", "Text B", "text.alternate"),
        ]),
        Err(GenerationProviderContractError::DuplicateRouteId)
    );
}

#[test]
fn rejects_empty_duplicate_and_unresolvable_capability_compositions() {
    assert!(matches!(
        GenerationProviderCapabilities::try_new(None, None, None, None),
        Err(GenerationProviderContractError::EmptyCapabilities)
    ));

    let duplicate = Arc::new(TestCapabilityImpl::new("shared.route.v1", "shared.route.v1"));
    assert!(matches!(
        GenerationProviderCapabilities::try_new(
            Some(duplicate.clone()),
            Some(duplicate),
            None,
            None,
        ),
        Err(GenerationProviderContractError::DuplicateRouteId)
    ));

    let broken: Arc<dyn TextGenerationProviderInterface> = Arc::new(BrokenTextCapabilityImpl {
        contract: TextGenerationProviderContract::try_new(vec![route(
            "broken.text.v1",
            "Broken Text",
            "text.general",
        )])
        .unwrap(),
    });
    assert!(matches!(
        GenerationProviderCapabilities::try_new(Some(broken), None, None, None),
        Err(GenerationProviderContractError::RouteResolutionMismatch)
    ));
}

#[test]
fn composes_all_four_complete_capabilities_and_derives_safe_contract() {
    let capability = Arc::new(TestCapabilityImpl::new("mock.text.v1", "mock.image.v1"));
    let capabilities = GenerationProviderCapabilities::try_new(
        Some(capability.clone()),
        Some(capability.clone()),
        Some(capability.clone()),
        Some(capability),
    )
    .unwrap();
    let provider = TestProviderImpl {
        id: GenerationProviderId::try_new("mock").unwrap(),
        display_name: GenerationProviderDisplayName::try_new("Mock Provider").unwrap(),
        capabilities,
    };

    let contract = GenerationProviderContract::from_provider(&provider);

    assert_eq!(contract.provider_id(), provider.generation_provider_id());
    assert_eq!(contract.display_name(), provider.generation_provider_display_name());
    assert_eq!(contract.text().unwrap().routes().len(), 1);
    assert_eq!(contract.image().unwrap().routes().len(), 1);
    assert_eq!(contract.video().unwrap().routes().len(), 1);
    assert_eq!(contract.voice().unwrap().routes().len(), 1);
}

#[test]
fn provider_boundary_values_enforce_bounds_without_deep_clone() {
    assert_eq!(
        GenerationProviderDisplayName::try_new(" Mock "),
        Err(GenerationProviderContractError::InvalidDisplayName)
    );
    assert_eq!(
        ImageGenerationProviderResult::try_new(Vec::new()),
        Err(GenerationProviderValueError::InvalidResult)
    );
    assert_eq!(
        GenerationProviderProgress::try_new(Some(101)),
        Err(GenerationProviderValueError::InvalidProgress)
    );
    assert_eq!(
        GenerationProviderCallError::try_new(
            GenerationProviderCallErrorKind::Permanent,
            "CALL_FAILED",
            "The call failed.",
            Some(time(200)),
            time(100),
        ),
        Err(GenerationProviderValueError::InvalidFailure)
    );
    assert_eq!(
        GenerationProviderCallError::try_new(
            GenerationProviderCallErrorKind::Transient,
            "CALL_RETRY",
            "The call may be retried.",
            Some(time(100)),
            time(100),
        ),
        Err(GenerationProviderValueError::InvalidFailure)
    );

    let result = VideoGenerationProviderResult::try_new(vec![1, 2, 3]).unwrap();
    let cloned_bytes = result.clone().into_bytes();
    let original_bytes = result.into_bytes();
    assert!(Arc::ptr_eq(&cloned_bytes, &original_bytes));
}

struct TestProviderImpl {
    id: GenerationProviderId,
    display_name: GenerationProviderDisplayName,
    capabilities: GenerationProviderCapabilities,
}

impl GenerationProviderInterface for TestProviderImpl {
    fn generation_provider_id(&self) -> &GenerationProviderId {
        &self.id
    }

    fn generation_provider_display_name(&self) -> &GenerationProviderDisplayName {
        &self.display_name
    }

    fn generation_provider_capabilities(&self) -> &GenerationProviderCapabilities {
        &self.capabilities
    }
}

#[derive(Clone)]
struct TestCapabilityImpl {
    text: TextGenerationProviderContract,
    image: ImageGenerationProviderContract,
    video: VideoGenerationProviderContract,
    voice: VoiceGenerationProviderContract,
}

impl TestCapabilityImpl {
    fn new(text_route: &str, image_route: &str) -> Self {
        Self {
            text: TextGenerationProviderContract::try_new(vec![route(
                text_route,
                "Text",
                "text.general",
            )])
            .unwrap(),
            image: ImageGenerationProviderContract::try_new(vec![route(
                image_route,
                "Image",
                "image.general",
            )])
            .unwrap(),
            video: VideoGenerationProviderContract::try_new(vec![route(
                "mock.video.v1",
                "Video",
                "video.general",
            )])
            .unwrap(),
            voice: VoiceGenerationProviderContract::try_new(vec![route(
                "mock.voice.v1",
                "Voice",
                "speech.general",
            )])
            .unwrap(),
        }
    }
}

macro_rules! focused_capability_impl {
    ($interface:ident, $contract_method:ident, $resolve_method:ident, $field:ident, $contract:ident, $execution:ident) => {
        impl $interface for TestCapabilityImpl {
            fn $contract_method(&self) -> &$contract {
                &self.$field
            }

            fn $resolve_method(
                &self,
                route_id: &GenerationProviderRouteId,
            ) -> Result<$execution, GenerationProviderRouteResolutionError> {
                self.$field
                    .find_route(route_id)
                    .ok_or(GenerationProviderRouteResolutionError::RouteNotFound)?;
                Ok($execution::Immediate(Arc::new(self.clone())))
            }
        }
    };
}

focused_capability_impl!(
    TextGenerationProviderInterface,
    text_generation_contract,
    resolve_text_generation_route,
    text,
    TextGenerationProviderContract,
    TextGenerationProviderExecution
);
focused_capability_impl!(
    ImageGenerationProviderInterface,
    image_generation_contract,
    resolve_image_generation_route,
    image,
    ImageGenerationProviderContract,
    ImageGenerationProviderExecution
);
focused_capability_impl!(
    VideoGenerationProviderInterface,
    video_generation_contract,
    resolve_video_generation_route,
    video,
    VideoGenerationProviderContract,
    VideoGenerationProviderExecution
);
focused_capability_impl!(
    VoiceGenerationProviderInterface,
    voice_generation_contract,
    resolve_voice_generation_route,
    voice,
    VoiceGenerationProviderContract,
    VoiceGenerationProviderExecution
);

#[async_trait]
impl TextGenerationImmediateExecutorInterface for TestCapabilityImpl {
    async fn execute_text_generation(
        &self,
        _context: &GenerationProviderCallContext,
        _spec: &TextGenerationSpec,
    ) -> Result<TextGenerationImmediateOutcome, GenerationProviderCallError> {
        Ok(TextGenerationImmediateOutcome::Completed(TextGenerationProviderResult::new(
            GenerationTaskText::try_new("text result").unwrap(),
        )))
    }
}

macro_rules! media_executor_impl {
    ($interface:ident, $method:ident, $spec:ident, $outcome:ident, $result:ident) => {
        #[async_trait]
        impl $interface for TestCapabilityImpl {
            async fn $method(
                &self,
                _context: &GenerationProviderCallContext,
                _spec: &$spec,
            ) -> Result<$outcome, GenerationProviderCallError> {
                Ok($outcome::Completed($result::try_new(vec![1]).unwrap()))
            }
        }
    };
}

media_executor_impl!(
    ImageGenerationImmediateExecutorInterface,
    execute_image_generation,
    ImageGenerationSpec,
    ImageGenerationImmediateOutcome,
    ImageGenerationProviderResult
);
media_executor_impl!(
    VideoGenerationImmediateExecutorInterface,
    execute_video_generation,
    VideoGenerationSpec,
    VideoGenerationImmediateOutcome,
    VideoGenerationProviderResult
);
media_executor_impl!(
    VoiceGenerationImmediateExecutorInterface,
    execute_voice_generation,
    VoiceGenerationSpec,
    VoiceGenerationImmediateOutcome,
    VoiceGenerationProviderResult
);

struct BrokenTextCapabilityImpl {
    contract: TextGenerationProviderContract,
}

impl TextGenerationProviderInterface for BrokenTextCapabilityImpl {
    fn text_generation_contract(&self) -> &TextGenerationProviderContract {
        &self.contract
    }

    fn resolve_text_generation_route(
        &self,
        _route_id: &GenerationProviderRouteId,
    ) -> Result<TextGenerationProviderExecution, GenerationProviderRouteResolutionError> {
        Err(GenerationProviderRouteResolutionError::RouteNotFound)
    }
}

fn route(id: &str, name: &str, profile: &str) -> GenerationProviderRouteContract {
    GenerationProviderRouteContract::try_new(
        route_id(id),
        route_name(name),
        BTreeSet::from([GenerationProfileRef::new(
            GenerationProfileId::try_new(profile).unwrap(),
            GenerationProfileVersion::try_new(1).unwrap(),
        )]),
    )
    .unwrap()
}

fn route_id(value: &str) -> GenerationProviderRouteId {
    GenerationProviderRouteId::try_new(value).unwrap()
}

fn route_name(value: &str) -> GenerationProviderRouteDisplayName {
    GenerationProviderRouteDisplayName::try_new(value).unwrap()
}
