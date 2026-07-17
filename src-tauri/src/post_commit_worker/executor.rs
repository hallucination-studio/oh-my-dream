use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use assets::asset::application::{
    AssetApplicationError, AssetFinalizeContentCommand, AssetFinalizeContentEffect,
    AssetFinalizeContentUseCase,
};
use assistant::{
    application::{AssistantApplyWorkflowChangeEffect, AssistantApplyWorkflowChangeEffectUseCase},
    interfaces::{
        AssistantApplicationError, AssistantModelContinuationStoreInterface,
        AssistantModelRunnerInterface, AssistantWorkflowChangeRepositoryInterface,
        AssistantWorkflowMutationApplierInterface, AssistantWorkflowRunStarterInterface,
    },
};
use async_trait::async_trait;
use engine::{
    node_capability::WorkflowRunId,
    workflow::{
        WorkflowApplicationError, WorkflowClockInterface, WorkflowExecuteRunUseCase,
        WorkflowRunEventPublisherInterface, WorkflowRunRepositoryInterface,
    },
};

use crate::post_commit_effect::DesktopPostCommitEffect;

/// Closed result understood by the post-commit worker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DesktopPostCommitEffectExecutionOutcome {
    /// The business effect reached its durable success outcome.
    Completed,
    /// A technical failure permits exact idempotent retry.
    TransientFailure,
    /// The owning business state proves no further execution is valid.
    OwningStateAlreadyTerminal,
}

/// Executes only the three closed Desktop effect variants.
#[async_trait]
pub trait DesktopPostCommitEffectExecutorInterface: Send + Sync {
    /// Executes one already-committed closed effect.
    async fn execute_desktop_post_commit_effect(
        &self,
        effect: DesktopPostCommitEffect,
    ) -> DesktopPostCommitEffectExecutionOutcome;
}

#[async_trait]
/// Executes the Workflow member of the closed Desktop effect union.
pub trait DesktopWorkflowEffectExecutorInterface: Send + Sync {
    /// Executes one already-admitted Workflow Run.
    async fn execute_workflow_effect(
        &self,
        run_id: WorkflowRunId,
    ) -> DesktopPostCommitEffectExecutionOutcome;
}

#[async_trait]
/// Executes the Asset member of the closed Desktop effect union.
pub trait DesktopAssetEffectExecutorInterface: Send + Sync {
    /// Finalizes one already-committed Asset content object.
    async fn execute_asset_effect(
        &self,
        effect: AssetFinalizeContentEffect,
    ) -> DesktopPostCommitEffectExecutionOutcome;
}

#[async_trait]
/// Executes the Assistant member of the closed Desktop effect union.
pub trait DesktopAssistantEffectExecutorInterface: Send + Sync {
    /// Applies one already-approved Assistant Workflow change.
    async fn execute_assistant_effect(
        &self,
        effect: AssistantApplyWorkflowChangeEffect,
    ) -> DesktopPostCommitEffectExecutionOutcome;
}

/// Routes the closed union to exactly one matching business use case.
pub struct DesktopPostCommitEffectExecutorAdapterImpl {
    workflow: Arc<dyn DesktopWorkflowEffectExecutorInterface>,
    asset: Arc<dyn DesktopAssetEffectExecutorInterface>,
    assistant: Arc<dyn DesktopAssistantEffectExecutorInterface>,
}

impl DesktopPostCommitEffectExecutorAdapterImpl {
    /// Wires the exact three focused business effect executors.
    #[must_use]
    pub fn new(
        workflow: Arc<dyn DesktopWorkflowEffectExecutorInterface>,
        asset: Arc<dyn DesktopAssetEffectExecutorInterface>,
        assistant: Arc<dyn DesktopAssistantEffectExecutorInterface>,
    ) -> Self {
        Self { workflow, asset, assistant }
    }
}

#[async_trait]
impl DesktopPostCommitEffectExecutorInterface for DesktopPostCommitEffectExecutorAdapterImpl {
    async fn execute_desktop_post_commit_effect(
        &self,
        effect: DesktopPostCommitEffect,
    ) -> DesktopPostCommitEffectExecutionOutcome {
        match effect {
            DesktopPostCommitEffect::Workflow(effect) => {
                self.workflow.execute_workflow_effect(effect.workflow_run_id).await
            }
            DesktopPostCommitEffect::Asset(effect) => self.asset.execute_asset_effect(effect).await,
            DesktopPostCommitEffect::Assistant(effect) => {
                self.assistant.execute_assistant_effect(effect).await
            }
        }
    }
}

#[async_trait]
impl<R, C, P> DesktopWorkflowEffectExecutorInterface for WorkflowExecuteRunUseCase<R, C, P>
where
    R: WorkflowRunRepositoryInterface + 'static,
    C: WorkflowClockInterface + 'static,
    P: WorkflowRunEventPublisherInterface + 'static,
{
    async fn execute_workflow_effect(
        &self,
        run_id: WorkflowRunId,
    ) -> DesktopPostCommitEffectExecutionOutcome {
        match self.execute_workflow_run(run_id).await {
            Ok(_) => DesktopPostCommitEffectExecutionOutcome::Completed,
            Err(WorkflowApplicationError::WorkflowRunNotFound) => {
                DesktopPostCommitEffectExecutionOutcome::OwningStateAlreadyTerminal
            }
            Err(_) => DesktopPostCommitEffectExecutionOutcome::TransientFailure,
        }
    }
}

#[async_trait]
impl DesktopAssetEffectExecutorInterface for AssetFinalizeContentUseCase {
    async fn execute_asset_effect(
        &self,
        effect: AssetFinalizeContentEffect,
    ) -> DesktopPostCommitEffectExecutionOutcome {
        let deadline = Instant::now() + Duration::from_secs(30);
        match self.finalize_asset_content(AssetFinalizeContentCommand::new(effect, deadline)).await
        {
            Ok(_) => DesktopPostCommitEffectExecutionOutcome::Completed,
            Err(
                AssetApplicationError::ManagedStorageFailed
                | AssetApplicationError::Cancelled
                | AssetApplicationError::DeadlineExceeded,
            ) => DesktopPostCommitEffectExecutionOutcome::TransientFailure,
            Err(_) => DesktopPostCommitEffectExecutionOutcome::OwningStateAlreadyTerminal,
        }
    }
}

#[async_trait]
impl<R, A, S, M, W> DesktopAssistantEffectExecutorInterface
    for AssistantApplyWorkflowChangeEffectUseCase<R, A, S, M, W>
where
    R: AssistantWorkflowChangeRepositoryInterface + 'static,
    A: AssistantWorkflowMutationApplierInterface + 'static,
    S: AssistantModelContinuationStoreInterface + 'static,
    M: AssistantModelRunnerInterface + 'static,
    W: AssistantWorkflowRunStarterInterface + 'static,
{
    async fn execute_assistant_effect(
        &self,
        effect: AssistantApplyWorkflowChangeEffect,
    ) -> DesktopPostCommitEffectExecutionOutcome {
        match self.apply_workflow_change_effect(effect.workflow_change_id()).await {
            Ok(_) => DesktopPostCommitEffectExecutionOutcome::Completed,
            Err(
                AssistantApplicationError::NotFound
                | AssistantApplicationError::NotVisible
                | AssistantApplicationError::InvalidTransition
                | AssistantApplicationError::ApprovalExpired
                | AssistantApplicationError::ApprovalMismatch,
            ) => DesktopPostCommitEffectExecutionOutcome::OwningStateAlreadyTerminal,
            Err(_) => DesktopPostCommitEffectExecutionOutcome::TransientFailure,
        }
    }
}
