use std::sync::Arc;

use async_trait::async_trait;
use engine::{
    node_capability::WorkflowRunId,
    workflow::{
        WorkflowApplicationError, WorkflowClassifyRunsAfterRestartUseCase, WorkflowClockInterface,
        WorkflowExecuteRunUseCase, WorkflowGenerationTaskRecoveryReaderInterface,
        WorkflowRunEventPublisherInterface, WorkflowRunFailure, WorkflowRunLoadKey,
        WorkflowRunRepositoryInterface, WorkflowRunRestartDisposition, WorkflowRunState,
    },
};
use tasks::generation_task::GenerationTaskOutboxReaderInterface;

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

/// Recovery action for one durable Workflow execution effect.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DesktopWorkflowEffectRecovery {
    /// The owning Run remains non-terminal and its effect must remain executable.
    ReplaySafe,
    /// The owning Run is terminal and the stale effect must be abandoned.
    Abandon(DesktopPostCommitEffectAbandonReason),
}

/// Workflow-owned facts required by Desktop startup recovery.
#[async_trait]
pub trait DesktopWorkflowRestartRecoveryInterface: Send + Sync {
    /// Classifies every non-terminal Run and interrupts only unsafe in-process work.
    async fn classify_all_non_terminal_workflow_runs(
        &self,
    ) -> Result<(), DesktopWorkflowRestartRecoveryError>;

    /// Selects replay for an active safe Run or a terminal abandon reason.
    async fn workflow_effect_recovery(
        &self,
        run_id: WorkflowRunId,
    ) -> Result<DesktopWorkflowEffectRecovery, DesktopWorkflowRestartRecoveryError>;
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
    task_recovery: Arc<dyn WorkflowGenerationTaskRecoveryReaderInterface>,
}

impl DesktopWorkflowRestartRecoveryAdapterImpl {
    /// Wires the Desktop-only global Run enumeration to the Workflow transition owner.
    #[must_use]
    pub fn new(
        repository: Arc<SqliteWorkflowRunRepositoryAdapterImpl>,
        interrupter: Arc<dyn DesktopWorkflowRunRestartInterrupterInterface>,
        task_recovery: Arc<dyn WorkflowGenerationTaskRecoveryReaderInterface>,
    ) -> Self {
        Self { repository, interrupter, task_recovery }
    }
}

#[async_trait]
impl DesktopWorkflowRestartRecoveryInterface for DesktopWorkflowRestartRecoveryAdapterImpl {
    async fn classify_all_non_terminal_workflow_runs(
        &self,
    ) -> Result<(), DesktopWorkflowRestartRecoveryError> {
        let classifier = WorkflowClassifyRunsAfterRestartUseCase::new(self.task_recovery.clone());
        let mut after = None;
        loop {
            let run_ids = self
                .repository
                .list_active_workflow_run_ids_after(after, 100)
                .await
                .map_err(|_| DesktopWorkflowRestartRecoveryError)?;
            for run_id in &run_ids {
                let run = self
                    .repository
                    .load_workflow_run(WorkflowRunLoadKey::Run(*run_id))
                    .await
                    .map_err(|_| DesktopWorkflowRestartRecoveryError)?
                    .ok_or(DesktopWorkflowRestartRecoveryError)?;
                if classifier
                    .classify_workflow_run_after_restart(&run)
                    .await
                    .map_err(|_| DesktopWorkflowRestartRecoveryError)?
                    == WorkflowRunRestartDisposition::InterruptUnsafe
                {
                    self.interrupter
                        .interrupt_workflow_run_after_restart(*run_id)
                        .await
                        .map_err(|_| DesktopWorkflowRestartRecoveryError)?;
                }
            }
            let Some(last) = run_ids.last().copied() else {
                return Ok(());
            };
            after = Some(last);
        }
    }

    async fn workflow_effect_recovery(
        &self,
        run_id: WorkflowRunId,
    ) -> Result<DesktopWorkflowEffectRecovery, DesktopWorkflowRestartRecoveryError> {
        let run = self
            .repository
            .load_workflow_run(WorkflowRunLoadKey::Run(run_id))
            .await
            .map_err(|_| DesktopWorkflowRestartRecoveryError)?
            .ok_or(DesktopWorkflowRestartRecoveryError)?;
        if matches!(run.state(), WorkflowRunState::Queued | WorkflowRunState::Running) {
            return Ok(DesktopWorkflowEffectRecovery::ReplaySafe);
        }
        Ok(DesktopWorkflowEffectRecovery::Abandon(
            if run.failure() == Some(&WorkflowRunFailure::InterruptedByRestart) {
                DesktopPostCommitEffectAbandonReason::WorkflowInterruptedByRestart
            } else {
                DesktopPostCommitEffectAbandonReason::OwningStateAlreadyTerminal
            },
        ))
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
    task_outbox: Arc<dyn GenerationTaskOutboxReaderInterface>,
}

impl DesktopStartupRecovery {
    /// Wires the exact three startup recovery boundaries.
    #[must_use]
    pub fn new(
        instance_id: DesktopApplicationInstanceId,
        outbox: Arc<dyn DesktopPostCommitEffectOutboxInterface>,
        workflow: Arc<dyn DesktopWorkflowRestartRecoveryInterface>,
        clock: Arc<dyn DesktopPostCommitWorkerClockInterface>,
        task_outbox: Arc<dyn GenerationTaskOutboxReaderInterface>,
    ) -> Self {
        Self { instance_id, outbox, workflow, clock, task_outbox }
    }

    /// Resets Task claims, classifies Runs, then repairs every recoverable Desktop effect page.
    pub async fn recover_before_accepting_commands(
        &self,
    ) -> Result<(), DesktopStartupRecoveryError> {
        self.task_outbox
            .reset_claimed_generation_task_effects()
            .await
            .map_err(|_| DesktopStartupRecoveryError::Outbox)?;
        self.workflow
            .classify_all_non_terminal_workflow_runs()
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
                let recovery = self
                    .workflow
                    .workflow_effect_recovery(effect.workflow_run_id)
                    .await
                    .map_err(|_| DesktopStartupRecoveryError::Workflow)?;
                match (recovery, record.state()) {
                    (
                        DesktopWorkflowEffectRecovery::ReplaySafe,
                        DesktopPostCommitEffectState::Ready,
                    ) => Ok(()),
                    (
                        DesktopWorkflowEffectRecovery::ReplaySafe,
                        DesktopPostCommitEffectState::Claimed { instance_id, .. },
                    ) => self
                        .outbox
                        .recover_replayable_post_commit_effect(record.effect_id(), instance_id)
                        .await
                        .map_err(|_| DesktopStartupRecoveryError::Outbox),
                    (DesktopWorkflowEffectRecovery::Abandon(reason), state) => self
                        .outbox
                        .recover_abandoned_post_commit_effect(
                            record.effect_id(),
                            state,
                            self.timestamp()?,
                            reason,
                        )
                        .await
                        .map_err(|_| DesktopStartupRecoveryError::Outbox),
                    (DesktopWorkflowEffectRecovery::ReplaySafe, _) => {
                        Err(DesktopStartupRecoveryError::InvalidRecord)
                    }
                }
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
