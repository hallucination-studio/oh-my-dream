//! Consumer-owned Generation Task repository and external-boundary interfaces.

use async_trait::async_trait;
use projects::project::domain::ProjectId;

use super::provider::GenerationProviderRouteResolutionError;
use crate::generation_task::application::{
    GenerationProviderRegistryError, GenerationProviderResolvedRoute, GenerationTaskAssetKey,
    GenerationTaskAssetRecovery, GenerationTaskAvailableAsset, GenerationTaskBoundaryError,
    GenerationTaskClaimedEffect, GenerationTaskCreateResult, GenerationTaskCursorPage,
    GenerationTaskListQuery, GenerationTaskOriginState, GenerationTaskOutboxChanges,
    GenerationTaskRepositoryError, GenerationTaskStoreAssetCommand, GenerationTaskSummaryView,
    GenerationTaskWorkflowCompletionOutcome,
};
use crate::generation_task::domain::{
    GenerationTaskAggregate, GenerationTaskId, GenerationTaskRequestKind, GenerationTaskTarget,
    GenerationTaskTimestamp,
};

/// Claim/reset boundary consumed only by the Generation Task worker and startup recovery.
#[async_trait]
pub trait GenerationTaskOutboxReaderInterface: Send + Sync {
    /// Atomically claims at most one due effect, skipping tasks already in flight.
    async fn claim_next_generation_task_effect(
        &self,
        now: GenerationTaskTimestamp,
    ) -> Result<Option<GenerationTaskClaimedEffect>, GenerationTaskRepositoryError>;

    /// Resets every prior-process claim after the exclusive process lock is acquired.
    async fn reset_claimed_generation_task_effects(
        &self,
    ) -> Result<u64, GenerationTaskRepositoryError>;
}

/// Atomic aggregate and outbox persistence boundary.
#[async_trait]
pub trait GenerationTaskRepositoryInterface: Send + Sync {
    /// Creates one task and initial effect or returns the matching existing task.
    async fn create_generation_task(
        &self,
        task: &GenerationTaskAggregate,
        message: crate::generation_task::application::GenerationTaskEffect,
    ) -> Result<GenerationTaskCreateResult, GenerationTaskRepositoryError>;

    /// Loads one trusted effect-owned Task identity.
    async fn load_generation_task(
        &self,
        id: GenerationTaskId,
    ) -> Result<Option<GenerationTaskAggregate>, GenerationTaskRepositoryError>;

    /// Loads one Task only inside its required public Project scope.
    async fn load_generation_task_for_project(
        &self,
        project_id: ProjectId,
        id: GenerationTaskId,
    ) -> Result<Option<GenerationTaskAggregate>, GenerationTaskRepositoryError>;

    /// Atomically saves one validated transition and its outbox changes.
    async fn save_generation_task(
        &self,
        task: &GenerationTaskAggregate,
        expected_revision: u64,
        outbox: GenerationTaskOutboxChanges,
    ) -> Result<(), GenerationTaskRepositoryError>;

    /// Lists one stable bounded Project-scoped page.
    async fn list_generation_tasks(
        &self,
        query: GenerationTaskListQuery,
    ) -> Result<GenerationTaskCursorPage<GenerationTaskSummaryView>, GenerationTaskRepositoryError>;
}

/// Dynamic immutable provider registry used for exact task routing and recovery.
pub trait GenerationProviderRegistryInterface: Send + Sync {
    /// Resolves one exact provider/kind/route target without Settings fallback.
    fn resolve_generation_provider_route(
        &self,
        target: &GenerationTaskTarget,
        request_kind: GenerationTaskRequestKind,
    ) -> Result<&GenerationProviderResolvedRoute, GenerationProviderRegistryError>;
}

/// Task-owned wall-clock boundary.
pub trait GenerationTaskClockInterface: Send + Sync {
    /// Observes one non-decreasing UTC-millisecond task time.
    fn observe_generation_task_time(
        &self,
    ) -> Result<GenerationTaskTimestamp, GenerationTaskBoundaryError>;
}

/// Exact Workflow-origin state reader used before every provider or Asset effect.
#[async_trait]
pub trait GenerationTaskOriginStateReaderInterface: Send + Sync {
    /// Reads the authoritative state of one exact Task origin.
    async fn read_generation_task_origin_state(
        &self,
        task: &GenerationTaskAggregate,
    ) -> Result<GenerationTaskOriginState, GenerationTaskBoundaryError>;
}

/// Asset recovery and storage boundary consumed by Task finalization.
#[async_trait]
pub trait GenerationTaskAssetSinkInterface: Send + Sync {
    /// Recovers one deterministic output before any provider re-observation.
    async fn recover_generation_task_asset(
        &self,
        key: GenerationTaskAssetKey,
    ) -> Result<GenerationTaskAssetRecovery, GenerationTaskBoundaryError>;

    /// Stores one validated media result and returns only an Available Asset.
    async fn store_generation_task_asset(
        &self,
        command: GenerationTaskStoreAssetCommand,
    ) -> Result<GenerationTaskAvailableAsset, GenerationTaskBoundaryError>;
}

/// Terminal Task-to-Workflow completion boundary.
#[async_trait]
pub trait GenerationTaskWorkflowCompletionInterface: Send + Sync {
    /// Applies or idempotently observes one terminal Task outcome.
    async fn complete_generation_task_workflow_origin(
        &self,
        task: &GenerationTaskAggregate,
    ) -> Result<GenerationTaskWorkflowCompletionOutcome, GenerationTaskBoundaryError>;
}

impl From<GenerationProviderRouteResolutionError> for GenerationProviderRegistryError {
    fn from(_: GenerationProviderRouteResolutionError) -> Self {
        Self::RouteNotFound
    }
}
