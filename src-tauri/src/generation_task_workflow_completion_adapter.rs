//! Desktop bridge from terminal Generation Tasks to canonical Workflow completion.

use std::sync::Arc;

use assets::asset::{
    application::{AssetApplicationError, AssetGetQuery, AssetGetUseCase},
    domain::{AssetManagedContentState, AssetMediaKind},
};
use async_trait::async_trait;
use engine::{
    node_capability::{
        WorkflowManagedAssetIdBoundaryValue, WorkflowManagedAudioRef,
        WorkflowManagedContentFingerprint, WorkflowManagedImageRef, WorkflowManagedVideoRef,
        WorkflowTextPart, WorkflowTextValue,
    },
    workflow::{
        WorkflowApplicationError, WorkflowClockInterface, WorkflowCompleteGenerationTaskCommand,
        WorkflowCompleteGenerationTaskOutcome, WorkflowCompleteGenerationTaskUseCase,
        WorkflowGenerationTaskCompletionId, WorkflowGenerationTaskCompletionOutcome,
        WorkflowGenerationTaskCompletionValue, WorkflowGenerationTaskFailure,
        WorkflowGenerationTaskOrigin, WorkflowRunRepositoryInterface,
    },
};
use tasks::generation_task::{
    GenerationTaskAggregate, GenerationTaskBoundaryError, GenerationTaskFailureKind,
    GenerationTaskResult, GenerationTaskState, GenerationTaskWorkflowCompletionInterface,
    GenerationTaskWorkflowCompletionOutcome,
};

/// Applies terminal Generation Task outcomes through the canonical Workflow use case.
pub struct DesktopGenerationTaskWorkflowCompletionAdapterImpl<R, C> {
    workflow_completion: Arc<WorkflowCompleteGenerationTaskUseCase<R, C>>,
    asset_get: Arc<AssetGetUseCase>,
}

impl<R, C> DesktopGenerationTaskWorkflowCompletionAdapterImpl<R, C>
where
    R: WorkflowRunRepositoryInterface,
    C: WorkflowClockInterface,
{
    /// Wires canonical Workflow completion and Project-scoped Asset reads.
    #[must_use]
    pub const fn new(
        workflow_completion: Arc<WorkflowCompleteGenerationTaskUseCase<R, C>>,
        asset_get: Arc<AssetGetUseCase>,
    ) -> Self {
        Self { workflow_completion, asset_get }
    }

    async fn command(
        &self,
        task: &GenerationTaskAggregate,
    ) -> Result<WorkflowCompleteGenerationTaskCommand, GenerationTaskBoundaryError> {
        let origin = task.origin();
        let workflow_origin = WorkflowGenerationTaskOrigin {
            project_id: origin.project_id(),
            workflow_id: origin.workflow_id(),
            workflow_revision: origin.workflow_revision(),
            workflow_run_id: origin.workflow_run_id(),
            workflow_node_id: origin.workflow_node_id(),
            node_execution_id: origin.workflow_node_execution_id(),
            capability_contract_ref: origin.capability_contract_ref().clone(),
        };
        let outcome = match task.state() {
            GenerationTaskState::Succeeded { .. } => {
                let result = task.result().ok_or_else(permanent)?;
                WorkflowGenerationTaskCompletionOutcome::Succeeded(
                    self.completion_value(origin.project_id(), result).await?,
                )
            }
            GenerationTaskState::Failed { failure, .. } => {
                WorkflowGenerationTaskCompletionOutcome::Failed(map_failure(failure.kind()))
            }
            GenerationTaskState::Cancelled { .. } => {
                WorkflowGenerationTaskCompletionOutcome::Failed(
                    WorkflowGenerationTaskFailure::GenerationTaskCancelled,
                )
            }
            _ => return Err(permanent()),
        };
        Ok(WorkflowCompleteGenerationTaskCommand {
            completion_id: WorkflowGenerationTaskCompletionId::from_uuid(task.id().as_uuid())
                .map_err(|_| permanent())?,
            origin: workflow_origin,
            outcome,
        })
    }

    async fn completion_value(
        &self,
        project_id: projects::project::domain::ProjectId,
        result: &GenerationTaskResult,
    ) -> Result<WorkflowGenerationTaskCompletionValue, GenerationTaskBoundaryError> {
        match result {
            GenerationTaskResult::Text { content } => {
                let text = WorkflowTextValue::try_new([WorkflowTextPart::Literal(
                    content.as_str().to_owned(),
                )])
                .map_err(|_| permanent())?;
                Ok(WorkflowGenerationTaskCompletionValue::Text(text))
            }
            GenerationTaskResult::Asset(result) => {
                let asset = self
                    .asset_get
                    .get_asset(AssetGetQuery::new(project_id, result.asset_id()))
                    .await
                    .map_err(map_asset_error)?;
                if asset.media_kind() != result.media_kind() {
                    return Err(permanent());
                }
                let AssetManagedContentState::Available { descriptor } = asset.content_state()
                else {
                    return Err(transient());
                };
                let asset_id = WorkflowManagedAssetIdBoundaryValue::from_bytes(
                    asset.id().as_uuid().into_bytes(),
                )
                .map_err(|_| permanent())?;
                let fingerprint =
                    WorkflowManagedContentFingerprint::from_bytes(descriptor.digest().as_bytes());
                Ok(match result.media_kind() {
                    AssetMediaKind::Image => WorkflowGenerationTaskCompletionValue::Image(
                        WorkflowManagedImageRef::new(asset_id, fingerprint),
                    ),
                    AssetMediaKind::Video => WorkflowGenerationTaskCompletionValue::Video(
                        WorkflowManagedVideoRef::new(asset_id, fingerprint),
                    ),
                    AssetMediaKind::Audio => WorkflowGenerationTaskCompletionValue::Audio(
                        WorkflowManagedAudioRef::new(asset_id, fingerprint),
                    ),
                })
            }
        }
    }
}

#[async_trait]
impl<R, C> GenerationTaskWorkflowCompletionInterface
    for DesktopGenerationTaskWorkflowCompletionAdapterImpl<R, C>
where
    R: WorkflowRunRepositoryInterface,
    C: WorkflowClockInterface,
{
    async fn complete_generation_task_workflow_origin(
        &self,
        task: &GenerationTaskAggregate,
    ) -> Result<GenerationTaskWorkflowCompletionOutcome, GenerationTaskBoundaryError> {
        let command = self.command(task).await?;
        match self.workflow_completion.complete_generation_task(command).await {
            Ok(WorkflowCompleteGenerationTaskOutcome::Applied) => {
                Ok(GenerationTaskWorkflowCompletionOutcome::Applied)
            }
            Ok(WorkflowCompleteGenerationTaskOutcome::AlreadyApplied) => {
                Ok(GenerationTaskWorkflowCompletionOutcome::AlreadyApplied)
            }
            Ok(WorkflowCompleteGenerationTaskOutcome::OriginTerminal) => {
                Ok(GenerationTaskWorkflowCompletionOutcome::OriginTerminal)
            }
            Err(WorkflowApplicationError::WorkflowPersistenceFailure)
            | Err(WorkflowApplicationError::WorkflowRevisionConflict) => Err(transient()),
            Err(_) => Err(permanent()),
        }
    }
}

const fn map_failure(kind: GenerationTaskFailureKind) -> WorkflowGenerationTaskFailure {
    match kind {
        GenerationTaskFailureKind::InvalidRequest => WorkflowGenerationTaskFailure::InvalidRequest,
        GenerationTaskFailureKind::Authentication => WorkflowGenerationTaskFailure::Authentication,
        GenerationTaskFailureKind::PermissionDenied => {
            WorkflowGenerationTaskFailure::PermissionDenied
        }
        GenerationTaskFailureKind::ContentPolicy => WorkflowGenerationTaskFailure::ContentPolicy,
        GenerationTaskFailureKind::RateLimited => WorkflowGenerationTaskFailure::RateLimited,
        GenerationTaskFailureKind::ProviderUnavailable => {
            WorkflowGenerationTaskFailure::ProviderUnavailable
        }
        GenerationTaskFailureKind::Timeout => WorkflowGenerationTaskFailure::Timeout,
        GenerationTaskFailureKind::ProviderRejected => {
            WorkflowGenerationTaskFailure::ProviderRejected
        }
        GenerationTaskFailureKind::InvalidProviderResponse => {
            WorkflowGenerationTaskFailure::InvalidProviderResponse
        }
        GenerationTaskFailureKind::AmbiguousSubmission => {
            WorkflowGenerationTaskFailure::AmbiguousSubmission
        }
        GenerationTaskFailureKind::InputAssetUnavailable => {
            WorkflowGenerationTaskFailure::InputAssetUnavailable
        }
        GenerationTaskFailureKind::OutputAssetImport => {
            WorkflowGenerationTaskFailure::OutputAssetImport
        }
        GenerationTaskFailureKind::Internal => WorkflowGenerationTaskFailure::Internal,
    }
}

const fn map_asset_error(error: AssetApplicationError) -> GenerationTaskBoundaryError {
    match error {
        AssetApplicationError::ContentPending | AssetApplicationError::ManagedStorageFailed => {
            GenerationTaskBoundaryError::Transient
        }
        _ => GenerationTaskBoundaryError::Permanent,
    }
}

const fn transient() -> GenerationTaskBoundaryError {
    GenerationTaskBoundaryError::Transient
}

const fn permanent() -> GenerationTaskBoundaryError {
    GenerationTaskBoundaryError::Permanent
}

#[cfg(test)]
mod tests;
