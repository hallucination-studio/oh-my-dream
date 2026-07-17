//! Focused Generation Task effect-boundary values.

use crate::generation_task::domain::{
    GenerationTaskAggregate, GenerationTaskOrigin, GenerationTaskRequest,
    GenerationTaskRequestKind, GenerationTaskResult, GenerationTaskTarget, GenerationTaskTimestamp,
};
use crate::generation_task::interfaces::{
    ImageGenerationProviderResult, TextGenerationProviderResult, VideoGenerationProviderResult,
    VoiceGenerationProviderResult,
};
use engine::node_capability::{WorkflowNodeExecutionId, WorkflowRunId};

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
    workflow_run_id: WorkflowRunId,
    workflow_node_execution_id: WorkflowNodeExecutionId,
    request_kind: GenerationTaskRequestKind,
}

impl GenerationTaskAssetKey {
    /// Creates the task's only MVP media-output key.
    #[must_use]
    pub fn from_task(task: &GenerationTaskAggregate) -> Self {
        Self {
            workflow_run_id: task.origin().workflow_run_id(),
            workflow_node_execution_id: task.origin().workflow_node_execution_id(),
            request_kind: task.request().kind(),
        }
    }

    /// Returns the exact owning Workflow Run.
    #[must_use]
    pub const fn workflow_run_id(self) -> WorkflowRunId {
        self.workflow_run_id
    }

    /// Returns the exact Workflow node execution.
    #[must_use]
    pub const fn workflow_node_execution_id(self) -> WorkflowNodeExecutionId {
        self.workflow_node_execution_id
    }

    /// Returns the request kind that mechanically selects the output slot.
    #[must_use]
    pub const fn request_kind(self) -> GenerationTaskRequestKind {
        self.request_kind
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
    request: GenerationTaskRequest,
    observed_at: GenerationTaskTimestamp,
    provider_deadline_at: GenerationTaskTimestamp,
    provider_result: GenerationTaskProviderResult,
}

impl GenerationTaskStoreAssetCommand {
    /// Copies exact immutable task coordinates around one media result.
    #[must_use]
    pub fn from_task(
        task: &GenerationTaskAggregate,
        provider_result: GenerationTaskProviderResult,
        observed_at: GenerationTaskTimestamp,
    ) -> Self {
        Self {
            key: GenerationTaskAssetKey::from_task(task),
            origin: task.origin().clone(),
            target: task.target().clone(),
            request: task.request().clone(),
            observed_at,
            provider_deadline_at: task.provider_deadline_at(),
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
    /// Returns the immutable provider-neutral request.
    #[must_use]
    pub const fn request(&self) -> &GenerationTaskRequest {
        &self.request
    }
    /// Returns the wall-clock observation made before Asset storage.
    #[must_use]
    pub const fn observed_at(&self) -> GenerationTaskTimestamp {
        self.observed_at
    }
    /// Returns the immutable persisted provider deadline.
    #[must_use]
    pub const fn provider_deadline_at(&self) -> GenerationTaskTimestamp {
        self.provider_deadline_at
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
