use std::sync::Arc;

use async_trait::async_trait;
use engine::{
    node_capability::WorkflowRunId,
    workflow::{
        WorkflowApplicationError, WorkflowClockInterface, WorkflowExecuteRunUseCase,
        WorkflowRunEventPublisherInterface, WorkflowRunFailure, WorkflowRunLoadKey,
        WorkflowRunRepositoryInterface, WorkflowRunState,
    },
};

use crate::post_commit_effect::{
    DesktopApplicationInstanceId, DesktopPostCommitEffect, DesktopPostCommitEffectAbandonReason,
    DesktopPostCommitEffectOutboxInterface, DesktopPostCommitEffectState,
    DesktopPostCommitRecoveryLimit, DesktopPostCommitTimestamp,
};
use crate::workflow_storage_adapters::SqliteWorkflowRunRepositoryAdapterImpl;

use super::{DesktopPostCommitWorkerClockError, DesktopPostCommitWorkerClockInterface};

/// Workflow restart enumeration, transition, or observation failed.
#[derive(Clone, Copy, Debug, thiserror::Error, PartialEq, Eq)]
#[error("Desktop Workflow restart recovery failed")]
pub struct DesktopWorkflowRestartRecoveryError;

/// Workflow-owned facts required by Desktop startup recovery.
#[async_trait]
pub trait DesktopWorkflowRestartRecoveryInterface: Send + Sync {
    /// Idempotently marks every non-terminal Run interrupted before effect recovery begins.
    async fn interrupt_all_non_terminal_workflow_runs(
        &self,
    ) -> Result<(), DesktopWorkflowRestartRecoveryError>;

    /// Confirms the Run is terminal and selects its closed effect-abandon reason.
    async fn workflow_effect_abandon_reason(
        &self,
        run_id: WorkflowRunId,
    ) -> Result<DesktopPostCommitEffectAbandonReason, DesktopWorkflowRestartRecoveryError>;
}

/// One-Run interruption boundary consumed by the Desktop recovery adapter.
#[async_trait]
pub trait DesktopWorkflowRunRestartInterrupterInterface: Send + Sync {
    /// Idempotently interrupts one non-terminal Run after process restart.
    async fn interrupt_workflow_run_after_restart(
        &self,
        run_id: WorkflowRunId,
    ) -> Result<(), WorkflowApplicationError>;
}

/// SQLite-backed enumeration plus Workflow-owned interruption and terminal observation.
pub struct DesktopWorkflowRestartRecoveryAdapterImpl {
    repository: Arc<SqliteWorkflowRunRepositoryAdapterImpl>,
    interrupter: Arc<dyn DesktopWorkflowRunRestartInterrupterInterface>,
}

impl DesktopWorkflowRestartRecoveryAdapterImpl {
    /// Wires the Desktop-only global Run enumeration to the Workflow transition owner.
    #[must_use]
    pub fn new(
        repository: Arc<SqliteWorkflowRunRepositoryAdapterImpl>,
        interrupter: Arc<dyn DesktopWorkflowRunRestartInterrupterInterface>,
    ) -> Self {
        Self { repository, interrupter }
    }
}

#[async_trait]
impl DesktopWorkflowRestartRecoveryInterface for DesktopWorkflowRestartRecoveryAdapterImpl {
    async fn interrupt_all_non_terminal_workflow_runs(
        &self,
    ) -> Result<(), DesktopWorkflowRestartRecoveryError> {
        let mut after = None;
        loop {
            let run_ids = self
                .repository
                .list_active_workflow_run_ids_after(after, 100)
                .await
                .map_err(|_| DesktopWorkflowRestartRecoveryError)?;
            for run_id in &run_ids {
                self.interrupter
                    .interrupt_workflow_run_after_restart(*run_id)
                    .await
                    .map_err(|_| DesktopWorkflowRestartRecoveryError)?;
            }
            let Some(last) = run_ids.last().copied() else {
                return Ok(());
            };
            after = Some(last);
        }
    }

    async fn workflow_effect_abandon_reason(
        &self,
        run_id: WorkflowRunId,
    ) -> Result<DesktopPostCommitEffectAbandonReason, DesktopWorkflowRestartRecoveryError> {
        let run = self
            .repository
            .load_workflow_run(WorkflowRunLoadKey::Run(run_id))
            .await
            .map_err(|_| DesktopWorkflowRestartRecoveryError)?
            .ok_or(DesktopWorkflowRestartRecoveryError)?;
        if matches!(run.state(), WorkflowRunState::Queued | WorkflowRunState::Running) {
            return Err(DesktopWorkflowRestartRecoveryError);
        }
        Ok(if run.failure() == Some(&WorkflowRunFailure::InterruptedByRestart) {
            DesktopPostCommitEffectAbandonReason::WorkflowInterruptedByRestart
        } else {
            DesktopPostCommitEffectAbandonReason::OwningStateAlreadyTerminal
        })
    }
}

#[async_trait]
impl<R, C, P> DesktopWorkflowRunRestartInterrupterInterface for WorkflowExecuteRunUseCase<R, C, P>
where
    R: WorkflowRunRepositoryInterface + 'static,
    C: WorkflowClockInterface + 'static,
    P: WorkflowRunEventPublisherInterface + 'static,
{
    async fn interrupt_workflow_run_after_restart(
        &self,
        run_id: WorkflowRunId,
    ) -> Result<(), WorkflowApplicationError> {
        WorkflowExecuteRunUseCase::interrupt_workflow_run_after_restart(self, run_id)
            .await
            .map(|_| ())
    }
}

/// Startup recovery failed before the application could accept commands.
#[derive(Clone, Copy, Debug, thiserror::Error, PartialEq, Eq)]
pub enum DesktopStartupRecoveryError {
    /// Workflow interruption or terminal-cause observation failed.
    #[error("Desktop Workflow restart recovery failed")]
    Workflow,
    /// Durable effect recovery failed.
    #[error("Desktop post-commit effect recovery failed")]
    Outbox,
    /// A valid durable recovery timestamp could not be obtained.
    #[error("Desktop startup recovery clock failed")]
    Clock,
    /// A recovery row violated the closed startup contract.
    #[error("Desktop post-commit recovery row was invalid")]
    InvalidRecord,
}

/// Ordered, idempotent startup recovery for the closed Desktop effect outbox.
pub struct DesktopStartupRecovery {
    instance_id: DesktopApplicationInstanceId,
    outbox: Arc<dyn DesktopPostCommitEffectOutboxInterface>,
    workflow: Arc<dyn DesktopWorkflowRestartRecoveryInterface>,
    clock: Arc<dyn DesktopPostCommitWorkerClockInterface>,
}

impl DesktopStartupRecovery {
    /// Wires the exact three startup recovery boundaries.
    #[must_use]
    pub fn new(
        instance_id: DesktopApplicationInstanceId,
        outbox: Arc<dyn DesktopPostCommitEffectOutboxInterface>,
        workflow: Arc<dyn DesktopWorkflowRestartRecoveryInterface>,
        clock: Arc<dyn DesktopPostCommitWorkerClockInterface>,
    ) -> Self {
        Self { instance_id, outbox, workflow, clock }
    }

    /// Interrupts Runs first, then independently repairs every recoverable effect page.
    pub async fn recover_before_accepting_commands(
        &self,
    ) -> Result<(), DesktopStartupRecoveryError> {
        self.workflow
            .interrupt_all_non_terminal_workflow_runs()
            .await
            .map_err(|_| DesktopStartupRecoveryError::Workflow)?;
        let limit = DesktopPostCommitRecoveryLimit::from_u8(100)
            .ok_or(DesktopStartupRecoveryError::InvalidRecord)?;
        let mut cursor = None;
        loop {
            let page = self
                .outbox
                .list_recoverable_post_commit_effects(self.instance_id, cursor, limit)
                .await
                .map_err(|_| DesktopStartupRecoveryError::Outbox)?;
            for record in page.records() {
                self.recover_record(*record).await?;
            }
            cursor = page.next_cursor();
            if cursor.is_none() {
                return Ok(());
            }
        }
    }

    async fn recover_record(
        &self,
        record: crate::post_commit_effect::DesktopPostCommitEffectRecord,
    ) -> Result<(), DesktopStartupRecoveryError> {
        match record.effect() {
            DesktopPostCommitEffect::Workflow(effect) => {
                let reason = self
                    .workflow
                    .workflow_effect_abandon_reason(effect.workflow_run_id)
                    .await
                    .map_err(|_| DesktopStartupRecoveryError::Workflow)?;
                self.outbox
                    .recover_abandoned_post_commit_effect(
                        record.effect_id(),
                        record.state(),
                        self.timestamp()?,
                        reason,
                    )
                    .await
                    .map_err(|_| DesktopStartupRecoveryError::Outbox)
            }
            DesktopPostCommitEffect::Asset(_) | DesktopPostCommitEffect::Assistant(_) => {
                let DesktopPostCommitEffectState::Claimed { instance_id, .. } = record.state()
                else {
                    return Err(DesktopStartupRecoveryError::InvalidRecord);
                };
                self.outbox
                    .recover_replayable_post_commit_effect(record.effect_id(), instance_id)
                    .await
                    .map_err(|_| DesktopStartupRecoveryError::Outbox)
            }
        }
    }

    fn timestamp(&self) -> Result<DesktopPostCommitTimestamp, DesktopStartupRecoveryError> {
        self.clock
            .current_post_commit_timestamp()
            .map_err(|DesktopPostCommitWorkerClockError| DesktopStartupRecoveryError::Clock)
    }
}

#[cfg(test)]
mod tests;
