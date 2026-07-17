//! Type-safe dispatch for accepted-handle polling.

use super::{GenerationProviderResolvedRoute, GenerationTaskProviderResult};
use crate::generation_task::domain::{GenerationProviderTaskHandle, GenerationTaskAggregate};
use crate::generation_task::interfaces::{
    GenerationProviderCallContext, GenerationProviderCallError, GenerationProviderFailure,
    GenerationProviderProgress, ImageGenerationPollOutcome, ImageGenerationProviderExecution,
    TextGenerationPollOutcome, TextGenerationProviderExecution, VideoGenerationPollOutcome,
    VideoGenerationProviderExecution, VoiceGenerationPollOutcome, VoiceGenerationProviderExecution,
};

pub(super) enum PollEffectOutcome {
    Pending(GenerationProviderProgress),
    Completed(GenerationTaskProviderResult),
    Failed(GenerationProviderFailure),
    Cancelled,
}

pub(super) enum PollDispatchError {
    InvalidRoute,
    ProviderCall(GenerationProviderCallError),
}

pub(super) async fn execute_poll(
    route: &GenerationProviderResolvedRoute,
    task: &GenerationTaskAggregate,
    handle: &GenerationProviderTaskHandle,
) -> Result<PollEffectOutcome, PollDispatchError> {
    let context = GenerationProviderCallContext::try_new(
        task.id(),
        task.target().clone(),
        task.created_at(),
        task.provider_deadline_at(),
    )
    .map_err(|_| PollDispatchError::InvalidRoute)?;
    match route {
        GenerationProviderResolvedRoute::Text { execution, .. } => {
            let poller = text_poller(execution)?;
            map_text(
                poller
                    .poll_text_generation(&context, handle)
                    .await
                    .map_err(PollDispatchError::ProviderCall)?,
            )
        }
        GenerationProviderResolvedRoute::Image { execution, .. } => {
            let poller = image_poller(execution)?;
            map_image(
                poller
                    .poll_image_generation(&context, handle)
                    .await
                    .map_err(PollDispatchError::ProviderCall)?,
            )
        }
        GenerationProviderResolvedRoute::Video { execution, .. } => {
            let poller = video_poller(execution)?;
            map_video(
                poller
                    .poll_video_generation(&context, handle)
                    .await
                    .map_err(PollDispatchError::ProviderCall)?,
            )
        }
        GenerationProviderResolvedRoute::Voice { execution, .. } => {
            let poller = voice_poller(execution)?;
            map_voice(
                poller
                    .poll_voice_generation(&context, handle)
                    .await
                    .map_err(PollDispatchError::ProviderCall)?,
            )
        }
    }
}

macro_rules! poller {
    ($name:ident, $execution:ident, $trait:ty) => {
        fn $name(execution: &$execution) -> Result<&$trait, PollDispatchError> {
            match execution {
                $execution::Remote { poller, .. }
                | $execution::CancellableRemote { poller, .. } => Ok(poller.as_ref()),
                $execution::Immediate(_) => Err(PollDispatchError::InvalidRoute),
            }
        }
    };
}
poller!(
    text_poller,
    TextGenerationProviderExecution,
    dyn crate::generation_task::interfaces::TextGenerationPollerInterface
);
poller!(
    image_poller,
    ImageGenerationProviderExecution,
    dyn crate::generation_task::interfaces::ImageGenerationPollerInterface
);
poller!(
    video_poller,
    VideoGenerationProviderExecution,
    dyn crate::generation_task::interfaces::VideoGenerationPollerInterface
);
poller!(
    voice_poller,
    VoiceGenerationProviderExecution,
    dyn crate::generation_task::interfaces::VoiceGenerationPollerInterface
);

macro_rules! map_poll {
    ($name:ident, $outcome:ident, $variant:ident) => {
        fn $name(outcome: $outcome) -> Result<PollEffectOutcome, PollDispatchError> {
            Ok(match outcome {
                $outcome::Pending(progress) => PollEffectOutcome::Pending(progress),
                $outcome::Completed(result) => {
                    PollEffectOutcome::Completed(GenerationTaskProviderResult::$variant(result))
                }
                $outcome::Failed(failure) => PollEffectOutcome::Failed(failure),
                $outcome::Cancelled => PollEffectOutcome::Cancelled,
            })
        }
    };
}
map_poll!(map_text, TextGenerationPollOutcome, Text);
map_poll!(map_image, ImageGenerationPollOutcome, Image);
map_poll!(map_video, VideoGenerationPollOutcome, Video);
map_poll!(map_voice, VoiceGenerationPollOutcome, Voice);
