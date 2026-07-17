//! Type-safe dispatch for one exact Generation Provider submit route.

use super::{GenerationProviderResolvedRoute, GenerationTaskProviderResult};
use crate::generation_task::domain::{GenerationTaskAggregate, GenerationTaskRequest};
use crate::generation_task::interfaces::{
    GenerationProviderCallContext, GenerationProviderFailure, ImageGenerationImmediateOutcome,
    ImageGenerationProviderExecution, ImageGenerationSubmitOutcome, TextGenerationImmediateOutcome,
    TextGenerationProviderExecution, TextGenerationSubmitOutcome, VideoGenerationImmediateOutcome,
    VideoGenerationProviderExecution, VideoGenerationSubmitOutcome,
    VoiceGenerationImmediateOutcome, VoiceGenerationProviderExecution,
    VoiceGenerationSubmitOutcome,
};

#[derive(Clone)]
pub(super) enum SubmitEffectOutcome {
    Accepted(crate::generation_task::domain::GenerationProviderTaskHandle),
    Completed(GenerationTaskProviderResult),
    Rejected(GenerationProviderFailure),
}

#[derive(Clone, Copy)]
pub(super) enum SubmitDispatchError {
    InvalidRoute,
    InvalidContext,
    ProviderCall,
}

pub(super) async fn execute_submit(
    route: &GenerationProviderResolvedRoute,
    task: &GenerationTaskAggregate,
) -> Result<SubmitEffectOutcome, SubmitDispatchError> {
    let context = GenerationProviderCallContext::try_new(
        task.id(),
        task.target().clone(),
        task.created_at(),
        task.provider_deadline_at(),
    )
    .map_err(|_| SubmitDispatchError::InvalidContext)?;
    match (route, task.request()) {
        (
            GenerationProviderResolvedRoute::Text { execution, .. },
            GenerationTaskRequest::Text(spec),
        ) => submit_text(execution, &context, spec).await,
        (
            GenerationProviderResolvedRoute::Image { execution, .. },
            GenerationTaskRequest::Image(spec),
        ) => submit_image(execution, &context, spec).await,
        (
            GenerationProviderResolvedRoute::Video { execution, .. },
            GenerationTaskRequest::Video(spec),
        ) => submit_video(execution, &context, spec).await,
        (
            GenerationProviderResolvedRoute::Voice { execution, .. },
            GenerationTaskRequest::Voice(spec),
        ) => submit_voice(execution, &context, spec).await,
        _ => Err(SubmitDispatchError::InvalidRoute),
    }
}

macro_rules! submit_modality {
    ($function:ident, $execution:ident, $spec:ty, $immediate_method:ident, $submit_method:ident,
     $immediate_outcome:ident, $submit_outcome:ident, $result_variant:ident) => {
        async fn $function(
            execution: &$execution,
            context: &GenerationProviderCallContext,
            spec: &$spec,
        ) -> Result<SubmitEffectOutcome, SubmitDispatchError> {
            match execution {
                $execution::Immediate(executor) => match executor
                    .$immediate_method(context, spec)
                    .await
                    .map_err(|_| SubmitDispatchError::ProviderCall)?
                {
                    $immediate_outcome::Completed(result) => Ok(SubmitEffectOutcome::Completed(
                        GenerationTaskProviderResult::$result_variant(result),
                    )),
                    $immediate_outcome::Rejected(failure) => {
                        Ok(SubmitEffectOutcome::Rejected(failure))
                    }
                },
                $execution::Remote { submitter, .. }
                | $execution::CancellableRemote { submitter, .. } => match submitter
                    .$submit_method(context, spec)
                    .await
                    .map_err(|_| SubmitDispatchError::ProviderCall)?
                {
                    $submit_outcome::Accepted(handle) => Ok(SubmitEffectOutcome::Accepted(handle)),
                    $submit_outcome::Completed(result) => Ok(SubmitEffectOutcome::Completed(
                        GenerationTaskProviderResult::$result_variant(result),
                    )),
                    $submit_outcome::Rejected(failure) => {
                        Ok(SubmitEffectOutcome::Rejected(failure))
                    }
                },
            }
        }
    };
}

submit_modality!(
    submit_text,
    TextGenerationProviderExecution,
    crate::generation_task::domain::TextGenerationSpec,
    execute_text_generation,
    submit_text_generation,
    TextGenerationImmediateOutcome,
    TextGenerationSubmitOutcome,
    Text
);
submit_modality!(
    submit_image,
    ImageGenerationProviderExecution,
    crate::generation_task::domain::ImageGenerationSpec,
    execute_image_generation,
    submit_image_generation,
    ImageGenerationImmediateOutcome,
    ImageGenerationSubmitOutcome,
    Image
);
submit_modality!(
    submit_video,
    VideoGenerationProviderExecution,
    crate::generation_task::domain::VideoGenerationSpec,
    execute_video_generation,
    submit_video_generation,
    VideoGenerationImmediateOutcome,
    VideoGenerationSubmitOutcome,
    Video
);
submit_modality!(
    submit_voice,
    VoiceGenerationProviderExecution,
    crate::generation_task::domain::VoiceGenerationSpec,
    execute_voice_generation,
    submit_voice_generation,
    VoiceGenerationImmediateOutcome,
    VoiceGenerationSubmitOutcome,
    Voice
);
