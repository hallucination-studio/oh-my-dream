//! Stateless deterministic Mock Generation Provider composite.

use std::collections::BTreeSet;
use std::sync::Arc;

use nodes::{
    GenerationProfileError, GenerationProfileId, GenerationProfileRef, GenerationProfileVersion,
};
use tasks::generation_task::*;

const COMPLETION_OFFSET_MILLISECONDS: i64 = 1_000;

/// Construction failure for the immutable Mock provider graph.
#[derive(Debug, thiserror::Error)]
pub enum MockGenerationProviderConstructionError {
    /// A frozen Task domain identity is invalid.
    #[error(transparent)]
    Domain(#[from] GenerationTaskDomainError),
    /// A frozen provider contract is inconsistent.
    #[error(transparent)]
    Contract(#[from] GenerationProviderContractError),
    /// A frozen provider boundary value is invalid.
    #[error(transparent)]
    Value(#[from] GenerationProviderValueError),
    /// A frozen Generation Profile reference is invalid.
    #[error(transparent)]
    Profile(#[from] GenerationProfileError),
}

/// The only provider composite shipped by the MVP.
pub struct MockGenerationProviderAdapterImpl {
    provider_id: GenerationProviderId,
    display_name: GenerationProviderDisplayName,
    capabilities: GenerationProviderCapabilities,
}

impl MockGenerationProviderAdapterImpl {
    /// Constructs the exact frozen Image, Video, and Voice capability product.
    pub fn try_new() -> Result<Self, MockGenerationProviderConstructionError> {
        let image = Arc::new(MockTextToImageProviderRouteImpl::try_new()?);
        let video = Arc::new(MockImageToVideoProviderRouteImpl::try_new()?);
        let voice = Arc::new(MockTextToSpeechProviderRouteImpl::try_new()?);
        let capabilities =
            GenerationProviderCapabilities::try_new(None, Some(image), Some(video), Some(voice))?;
        Ok(Self {
            provider_id: GenerationProviderId::try_new("mock")?,
            display_name: GenerationProviderDisplayName::try_new("Mock")?,
            capabilities,
        })
    }
}

impl GenerationProviderInterface for MockGenerationProviderAdapterImpl {
    fn generation_provider_id(&self) -> &GenerationProviderId {
        &self.provider_id
    }

    fn generation_provider_display_name(&self) -> &GenerationProviderDisplayName {
        &self.display_name
    }

    fn generation_provider_capabilities(&self) -> &GenerationProviderCapabilities {
        &self.capabilities
    }
}

macro_rules! mock_remote_route {
    (
        $route:ident, $kind:literal, $route_id:literal, $display:literal, $profile:literal,
        $contract:ident, $capability:ident, $contract_method:ident, $resolve_method:ident,
        $execution:ident, $spec:ty, $submitter:ident, $submit_method:ident, $submit_outcome:ident,
        $poller:ident, $poll_method:ident, $poll_outcome:ident, $result:ident, $bytes:expr
    ) => {
        #[doc = concat!("Stateless Mock implementation for `", $route_id, "`.")]
        #[derive(Clone)]
        pub struct $route {
            contract: $contract,
            invalid_call: GenerationProviderCallError,
            deadline_failure: GenerationProviderFailure,
            result: $result,
        }

        impl $route {
            fn try_new() -> Result<Self, MockGenerationProviderConstructionError> {
                Ok(Self {
                    contract: $contract::try_new(vec![route_contract(
                        $route_id, $display, $profile,
                    )?])?,
                    invalid_call: invalid_call_error()?,
                    deadline_failure: deadline_failure()?,
                    result: $result::try_new($bytes.to_vec())?,
                })
            }
        }

        impl $capability for $route {
            fn $contract_method(&self) -> &$contract {
                &self.contract
            }

            fn $resolve_method(
                &self,
                route_id: &GenerationProviderRouteId,
            ) -> Result<$execution, GenerationProviderRouteResolutionError> {
                if route_id.as_str() != $route_id {
                    return Err(GenerationProviderRouteResolutionError::RouteNotFound);
                }
                let route = Arc::new(self.clone());
                Ok($execution::Remote { submitter: route.clone(), poller: route })
            }
        }

        #[async_trait::async_trait]
        impl $submitter for $route {
            async fn $submit_method(
                &self,
                context: &GenerationProviderCallContext,
                _spec: &$spec,
            ) -> Result<$submit_outcome, GenerationProviderCallError> {
                if !valid_target(context, $route_id, $profile) {
                    return Err(self.invalid_call.clone());
                }
                let completion_at =
                    completion_at(context).ok_or_else(|| self.invalid_call.clone())?;
                let now = observe_now().map_err(|_| self.invalid_call.clone())?;
                if now >= context.provider_deadline_at()
                    || completion_at >= context.provider_deadline_at()
                {
                    return Ok($submit_outcome::Rejected(self.deadline_failure.clone()));
                }
                let handle = encode_handle($kind, context, completion_at)
                    .map_err(|_| self.invalid_call.clone())?;
                Ok($submit_outcome::Accepted(handle))
            }
        }

        #[async_trait::async_trait]
        impl $poller for $route {
            async fn $poll_method(
                &self,
                context: &GenerationProviderCallContext,
                handle: &GenerationProviderTaskHandle,
            ) -> Result<$poll_outcome, GenerationProviderCallError> {
                if !valid_target(context, $route_id, $profile) {
                    return Err(self.invalid_call.clone());
                }
                let completion_at = decode_handle($kind, context, handle)
                    .ok_or_else(|| self.invalid_call.clone())?;
                let now = observe_now().map_err(|_| self.invalid_call.clone())?;
                if now >= context.provider_deadline_at() {
                    return Ok($poll_outcome::Failed(self.deadline_failure.clone()));
                }
                if now < completion_at {
                    return Ok($poll_outcome::Pending(
                        GenerationProviderProgress::try_new(Some(50))
                            .map_err(|_| self.invalid_call.clone())?,
                    ));
                }
                Ok($poll_outcome::Completed(self.result.clone()))
            }
        }
    };
}

mock_remote_route!(
    MockTextToImageProviderRouteImpl,
    "image",
    "mock.image.high-quality-general.v1",
    "High Quality General Image",
    "image.high_quality_general",
    ImageGenerationProviderContract,
    ImageGenerationProviderInterface,
    image_generation_contract,
    resolve_image_generation_route,
    ImageGenerationProviderExecution,
    ImageGenerationSpec,
    ImageGenerationSubmitterInterface,
    submit_image_generation,
    ImageGenerationSubmitOutcome,
    ImageGenerationPollerInterface,
    poll_image_generation,
    ImageGenerationPollOutcome,
    ImageGenerationProviderResult,
    IMAGE_BYTES
);

mock_remote_route!(
    MockImageToVideoProviderRouteImpl,
    "video",
    "mock.video.cinematic-image-animation.v1",
    "Cinematic Image Animation",
    "video.cinematic_image_animation",
    VideoGenerationProviderContract,
    VideoGenerationProviderInterface,
    video_generation_contract,
    resolve_video_generation_route,
    VideoGenerationProviderExecution,
    VideoGenerationSpec,
    VideoGenerationSubmitterInterface,
    submit_video_generation,
    VideoGenerationSubmitOutcome,
    VideoGenerationPollerInterface,
    poll_video_generation,
    VideoGenerationPollOutcome,
    VideoGenerationProviderResult,
    VIDEO_BYTES
);

mock_remote_route!(
    MockTextToSpeechProviderRouteImpl,
    "voice",
    "mock.voice.multilingual-narration.v1",
    "Multilingual Narration",
    "speech.multilingual_narration",
    VoiceGenerationProviderContract,
    VoiceGenerationProviderInterface,
    voice_generation_contract,
    resolve_voice_generation_route,
    VoiceGenerationProviderExecution,
    VoiceGenerationSpec,
    VoiceGenerationSubmitterInterface,
    submit_voice_generation,
    VoiceGenerationSubmitOutcome,
    VoiceGenerationPollerInterface,
    poll_voice_generation,
    VoiceGenerationPollOutcome,
    VoiceGenerationProviderResult,
    VOICE_BYTES
);

fn route_contract(
    route_id: &str,
    display_name: &str,
    profile_id: &str,
) -> Result<GenerationProviderRouteContract, MockGenerationProviderConstructionError> {
    Ok(GenerationProviderRouteContract::try_new(
        GenerationProviderRouteId::try_new(route_id)?,
        GenerationProviderRouteDisplayName::try_new(display_name)?,
        BTreeSet::from([GenerationProfileRef::new(
            GenerationProfileId::try_new(profile_id)?,
            GenerationProfileVersion::try_new(1)?,
        )]),
    )?)
}

fn completion_at(context: &GenerationProviderCallContext) -> Option<GenerationTaskTimestamp> {
    let value = context
        .task_created_at()
        .as_utc_milliseconds()
        .checked_add(COMPLETION_OFFSET_MILLISECONDS)?;
    GenerationTaskTimestamp::from_utc_milliseconds(value).ok()
}

fn encode_handle(
    kind: &str,
    context: &GenerationProviderCallContext,
    completion_at: GenerationTaskTimestamp,
) -> Result<GenerationProviderTaskHandle, GenerationTaskDomainError> {
    GenerationProviderTaskHandle::try_new(format!(
        "mock-v1|{kind}|{}|{}|{}",
        context.task_id(),
        context.task_created_at().as_utc_milliseconds(),
        completion_at.as_utc_milliseconds()
    ))
}

fn valid_target(context: &GenerationProviderCallContext, route_id: &str, profile_id: &str) -> bool {
    let target = context.target();
    target.provider_id().as_str() == "mock"
        && target.route_id().as_str() == route_id
        && target.generation_profile_ref().id().as_str() == profile_id
        && target.generation_profile_ref().version().get() == 1
}

fn decode_handle(
    kind: &str,
    context: &GenerationProviderCallContext,
    handle: &GenerationProviderTaskHandle,
) -> Option<GenerationTaskTimestamp> {
    let mut parts = handle.as_str().split('|');
    let valid_prefix = parts.next()? == "mock-v1" && parts.next()? == kind;
    let valid_task = parts.next()? == context.task_id().to_string();
    let created_at = parts.next()?.parse::<i64>().ok()?;
    let completed_at = parts.next()?.parse::<i64>().ok()?;
    if !valid_prefix
        || !valid_task
        || parts.next().is_some()
        || created_at != context.task_created_at().as_utc_milliseconds()
        || completed_at != created_at.checked_add(COMPLETION_OFFSET_MILLISECONDS)?
    {
        return None;
    }
    GenerationTaskTimestamp::from_utc_milliseconds(completed_at).ok()
}

fn observe_now() -> Result<GenerationTaskTimestamp, GenerationTaskDomainError> {
    let duration = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| GenerationTaskDomainError::InvalidTimestamp)?;
    let milliseconds = i64::try_from(duration.as_millis())
        .map_err(|_| GenerationTaskDomainError::InvalidTimestamp)?;
    GenerationTaskTimestamp::from_utc_milliseconds(milliseconds)
}

fn invalid_call_error() -> Result<GenerationProviderCallError, GenerationProviderValueError> {
    GenerationProviderCallError::try_new(
        GenerationProviderCallErrorKind::Permanent,
        "INVALID_MOCK_CALL",
        "Mock provider call context or handle is invalid.",
        None,
        GenerationTaskTimestamp::from_utc_milliseconds(0)
            .map_err(|_| GenerationProviderValueError::InvalidCallContext)?,
    )
}

fn deadline_failure() -> Result<GenerationProviderFailure, GenerationProviderValueError> {
    GenerationProviderFailure::try_new(
        GenerationProviderFailureKind::DeadlineExceeded,
        "PROVIDER_DEADLINE_EXCEEDED",
        "Mock provider deadline was exceeded.",
    )
}

const IMAGE_BYTES: &[u8] = include_bytes!("fixtures/image.png");
const VIDEO_BYTES: &[u8] = include_bytes!("fixtures/video.mp4");
const VOICE_BYTES: &[u8] = include_bytes!("fixtures/voice.mp3");
