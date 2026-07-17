//! Focused Generation Task effect-boundary values.

use crate::generation_task::domain::{
    GenerationTaskAggregate, GenerationTaskId, GenerationTaskOrigin, GenerationTaskResult,
    GenerationTaskTarget,
};
use crate::generation_task::interfaces::{
    ImageGenerationProviderResult, TextGenerationProviderResult, VideoGenerationProviderResult,
    VoiceGenerationProviderResult,
};

/// Result observed from one type-specific provider route.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GenerationTaskProviderResult {
    /// Inline Text requires no Asset finalization.
    Text(TextGenerationProviderResult),
    /// Validated Image bytes.
    Image(ImageGenerationProviderResult),
    /// Validated Video bytes.
    Video(VideoGenerationProviderResult),
    /// Validated Voice bytes whose business result is Audio.
    Voice(VoiceGenerationProviderResult),
}

/// Deterministic single-output Asset recovery key for one Task.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct GenerationTaskAssetKey {
    task_id: GenerationTaskId,
}

impl GenerationTaskAssetKey {
    /// Creates the task's only MVP media-output key.
    #[must_use]
    pub const fn new(task_id: GenerationTaskId) -> Self {
        Self { task_id }
    }

    /// Returns the owning Task identity.
    #[must_use]
    pub const fn task_id(self) -> GenerationTaskId {
        self.task_id
    }
}

/// Available durable Asset returned by the task-owned Asset sink.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GenerationTaskAvailableAsset {
    result: GenerationTaskResult,
}

impl GenerationTaskAvailableAsset {
    /// Accepts only one durable Asset result.
    pub fn try_new(
        result: GenerationTaskResult,
    ) -> Result<Self, super::GenerationTaskApplicationError> {
        if !matches!(result, GenerationTaskResult::Asset(_)) {
            return Err(super::GenerationTaskApplicationError::InvalidArgument);
        }
        Ok(Self { result })
    }

    /// Returns the exact durable Task result.
    #[must_use]
    pub const fn result(&self) -> &GenerationTaskResult {
        &self.result
    }
}

/// Recovery state checked before any repeat provider observation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GenerationTaskAssetRecovery {
    /// Exact media Asset is already Available.
    Available(GenerationTaskAvailableAsset),
    /// Durable Asset finalization remains pending.
    Pending,
    /// No recoverable Asset/source exists; provider observation is required.
    SourceRequired,
}

/// Complete command for storing one validated provider media result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GenerationTaskStoreAssetCommand {
    key: GenerationTaskAssetKey,
    origin: GenerationTaskOrigin,
    target: GenerationTaskTarget,
    provider_result: GenerationTaskProviderResult,
}

impl GenerationTaskStoreAssetCommand {
    /// Copies exact immutable task coordinates around one media result.
    #[must_use]
    pub fn from_task(
        task: &GenerationTaskAggregate,
        provider_result: GenerationTaskProviderResult,
    ) -> Self {
        Self {
            key: GenerationTaskAssetKey::new(task.id()),
            origin: task.origin().clone(),
            target: task.target().clone(),
            provider_result,
        }
    }

    /// Returns deterministic recovery key.
    #[must_use]
    pub const fn key(&self) -> GenerationTaskAssetKey {
        self.key
    }
    /// Returns exact Workflow origin.
    #[must_use]
    pub const fn origin(&self) -> &GenerationTaskOrigin {
        &self.origin
    }
    /// Returns immutable provider target.
    #[must_use]
    pub const fn target(&self) -> &GenerationTaskTarget {
        &self.target
    }
    /// Returns validated provider result.
    #[must_use]
    pub const fn provider_result(&self) -> &GenerationTaskProviderResult {
        &self.provider_result
    }
}

/// Workflow-owned origin state observed before an external Task effect.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GenerationTaskOriginState {
    /// Workflow execution has not yet committed durable waiting handoff.
    Running,
    /// Exact node execution is waiting for this Generation Task.
    WaitingForExternalCompletion,
    /// Owning Workflow Run or node is cancelled.
    Cancelled,
    /// Owning Workflow node is otherwise terminal.
    Terminal,
}

/// Idempotent Workflow completion delivery result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GenerationTaskWorkflowCompletionOutcome {
    /// Terminal Task outcome was applied now.
    Applied,
    /// Equivalent terminal outcome was already applied.
    AlreadyApplied,
    /// Origin is terminal and must not be reopened.
    OriginTerminal,
}
