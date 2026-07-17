//! Type-specific provider execution compositions and normalized outcomes.

use std::sync::Arc;

use async_trait::async_trait;

use super::{
    GenerationProviderCallError, GenerationProviderFailure, GenerationProviderProgress,
    GenerationProviderValueError, ImageGenerationProviderResult, TextGenerationProviderResult,
    VideoGenerationProviderResult, VoiceGenerationProviderResult,
};
use crate::generation_task::domain::{
    GenerationProviderTaskHandle, GenerationTaskId, GenerationTaskTarget, GenerationTaskTimestamp,
    ImageGenerationSpec, TextGenerationSpec, VideoGenerationSpec, VoiceGenerationSpec,
};

/// Immutable task-owned context supplied to every provider call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GenerationProviderCallContext {
    task_id: GenerationTaskId,
    target: GenerationTaskTarget,
    task_created_at: GenerationTaskTimestamp,
    provider_deadline_at: GenerationTaskTimestamp,
}

impl GenerationProviderCallContext {
    /// Creates call context only when the provider deadline follows creation.
    pub fn try_new(
        task_id: GenerationTaskId,
        target: GenerationTaskTarget,
        task_created_at: GenerationTaskTimestamp,
        provider_deadline_at: GenerationTaskTimestamp,
    ) -> Result<Self, GenerationProviderValueError> {
        if provider_deadline_at <= task_created_at {
            return Err(GenerationProviderValueError::InvalidCallContext);
        }
        Ok(Self { task_id, target, task_created_at, provider_deadline_at })
    }

    /// Returns the stable local task identity.
    #[must_use]
    pub const fn task_id(&self) -> GenerationTaskId {
        self.task_id
    }

    /// Returns the immutable admitted provider target.
    #[must_use]
    pub const fn target(&self) -> &GenerationTaskTarget {
        &self.target
    }

    /// Returns the persisted task creation time.
    #[must_use]
    pub const fn task_created_at(&self) -> GenerationTaskTimestamp {
        self.task_created_at
    }

    /// Returns the immutable provider deadline.
    #[must_use]
    pub const fn provider_deadline_at(&self) -> GenerationTaskTimestamp {
        self.provider_deadline_at
    }
}

/// Closed remote cancellation observation.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum GenerationCancellationOutcome {
    /// Provider accepted the cancellation request.
    Accepted,
    /// Provider reports that work is already cancelled.
    AlreadyCancelled,
    /// Provider reports no corresponding remote work.
    RemoteAbsent,
}

/// Complete remote-cancellation capability shared by cancellable route compositions.
#[async_trait]
pub trait GenerationCancellerInterface: Send + Sync {
    /// Requests cancellation for one exact accepted handle.
    async fn cancel_generation(
        &self,
        context: &GenerationProviderCallContext,
        handle: &GenerationProviderTaskHandle,
    ) -> Result<GenerationCancellationOutcome, GenerationProviderCallError>;
}

macro_rules! provider_execution_contract {
    (
        $execution:ident,
        $immediate_trait:ident,
        $immediate_method:ident,
        $submitter_trait:ident,
        $submit_method:ident,
        $poller_trait:ident,
        $poll_method:ident,
        $immediate_outcome:ident,
        $submit_outcome:ident,
        $poll_outcome:ident,
        $spec:ty,
        $result:ty,
        $label:literal
    ) => {
        #[doc = concat!("Complete ", $label, " provider route execution composition.")]
        pub enum $execution {
            /// One call returns a terminal result without a durable remote handle.
            Immediate(Arc<dyn $immediate_trait>),
            /// Submission returns a durable handle observed through a complete poller.
            Remote {
                /// Complete submit capability.
                submitter: Arc<dyn $submitter_trait>,
                /// Complete poll capability.
                poller: Arc<dyn $poller_trait>,
            },
            /// Remote execution additionally provides complete cancellation.
            CancellableRemote {
                /// Complete submit capability.
                submitter: Arc<dyn $submitter_trait>,
                /// Complete poll capability.
                poller: Arc<dyn $poller_trait>,
                /// Complete remote-cancellation capability.
                canceller: Arc<dyn GenerationCancellerInterface>,
            },
        }

        #[doc = concat!("Terminal outcome from one Immediate ", $label, " call.")]
        #[derive(Clone, Debug, PartialEq, Eq)]
        pub enum $immediate_outcome {
            /// Provider completed with one type-specific result.
            Completed($result),
            /// Provider declared a structured terminal rejection.
            Rejected(GenerationProviderFailure),
        }

        #[doc = concat!("Outcome from one ", $label, " submission call.")]
        #[derive(Clone, Debug, PartialEq, Eq)]
        pub enum $submit_outcome {
            /// Provider accepted work under one durable opaque handle.
            Accepted(GenerationProviderTaskHandle),
            /// Provider completed during submission.
            Completed($result),
            /// Provider declared a structured terminal rejection.
            Rejected(GenerationProviderFailure),
        }

        #[doc = concat!("Observation from one accepted ", $label, " handle.")]
        #[derive(Clone, Debug, PartialEq, Eq)]
        pub enum $poll_outcome {
            /// Remote work remains non-terminal with normalized progress.
            Pending(GenerationProviderProgress),
            /// Provider completed with one type-specific result.
            Completed($result),
            /// Provider declared a structured terminal failure.
            Failed(GenerationProviderFailure),
            /// Provider reports that remote work was cancelled.
            Cancelled,
        }

        #[doc = concat!("Complete Immediate executor for ", $label, " generation.")]
        #[async_trait]
        pub trait $immediate_trait: Send + Sync {
            #[doc = concat!("Executes one Immediate ", $label, " generation call.")]
            async fn $immediate_method(
                &self,
                context: &GenerationProviderCallContext,
                spec: &$spec,
            ) -> Result<$immediate_outcome, GenerationProviderCallError>;
        }

        #[doc = concat!("Complete remote submitter for ", $label, " generation.")]
        #[async_trait]
        pub trait $submitter_trait: Send + Sync {
            #[doc = concat!("Submits one remote ", $label, " generation call.")]
            async fn $submit_method(
                &self,
                context: &GenerationProviderCallContext,
                spec: &$spec,
            ) -> Result<$submit_outcome, GenerationProviderCallError>;
        }

        #[doc = concat!("Complete accepted-handle poller for ", $label, " generation.")]
        #[async_trait]
        pub trait $poller_trait: Send + Sync {
            #[doc = concat!("Polls one accepted ", $label, " generation handle.")]
            async fn $poll_method(
                &self,
                context: &GenerationProviderCallContext,
                handle: &GenerationProviderTaskHandle,
            ) -> Result<$poll_outcome, GenerationProviderCallError>;
        }
    };
}

provider_execution_contract!(
    TextGenerationProviderExecution,
    TextGenerationImmediateExecutorInterface,
    execute_text_generation,
    TextGenerationSubmitterInterface,
    submit_text_generation,
    TextGenerationPollerInterface,
    poll_text_generation,
    TextGenerationImmediateOutcome,
    TextGenerationSubmitOutcome,
    TextGenerationPollOutcome,
    TextGenerationSpec,
    TextGenerationProviderResult,
    "Text"
);
provider_execution_contract!(
    ImageGenerationProviderExecution,
    ImageGenerationImmediateExecutorInterface,
    execute_image_generation,
    ImageGenerationSubmitterInterface,
    submit_image_generation,
    ImageGenerationPollerInterface,
    poll_image_generation,
    ImageGenerationImmediateOutcome,
    ImageGenerationSubmitOutcome,
    ImageGenerationPollOutcome,
    ImageGenerationSpec,
    ImageGenerationProviderResult,
    "Image"
);
provider_execution_contract!(
    VideoGenerationProviderExecution,
    VideoGenerationImmediateExecutorInterface,
    execute_video_generation,
    VideoGenerationSubmitterInterface,
    submit_video_generation,
    VideoGenerationPollerInterface,
    poll_video_generation,
    VideoGenerationImmediateOutcome,
    VideoGenerationSubmitOutcome,
    VideoGenerationPollOutcome,
    VideoGenerationSpec,
    VideoGenerationProviderResult,
    "Video"
);
provider_execution_contract!(
    VoiceGenerationProviderExecution,
    VoiceGenerationImmediateExecutorInterface,
    execute_voice_generation,
    VoiceGenerationSubmitterInterface,
    submit_voice_generation,
    VoiceGenerationPollerInterface,
    poll_voice_generation,
    VoiceGenerationImmediateOutcome,
    VoiceGenerationSubmitOutcome,
    VoiceGenerationPollOutcome,
    VoiceGenerationSpec,
    VoiceGenerationProviderResult,
    "Voice"
);
