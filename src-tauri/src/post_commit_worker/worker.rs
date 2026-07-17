use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use async_trait::async_trait;

use crate::post_commit_effect::{
    DesktopApplicationInstanceId, DesktopPostCommitEffectAbandonReason,
    DesktopPostCommitEffectOutboxInterface, DesktopPostCommitEffectRecord,
    DesktopPostCommitTimestamp,
};

use super::{
    DesktopCommittedWorkflowEventDeliveryInterface, DesktopPostCommitEffectExecutionOutcome,
    DesktopPostCommitEffectExecutorInterface,
};

/// The process clock could not produce a valid durable timestamp.
#[derive(Clone, Copy, Debug, thiserror::Error, PartialEq, Eq)]
#[error("Desktop post-commit worker clock failed")]
pub struct DesktopPostCommitWorkerClockError;

/// Invalid process-scoped post-commit worker configuration.
#[derive(Clone, Copy, Debug, thiserror::Error, PartialEq, Eq)]
#[error("Desktop post-commit effect concurrency must be in 1..=8")]
pub struct DesktopPostCommitWorkerConfigurationError;

/// One bounded post-commit worker step could not complete safely.
#[derive(Clone, Copy, Debug, thiserror::Error, PartialEq, Eq)]
pub enum DesktopPostCommitWorkerError {
    /// A durable timestamp could not be obtained.
    #[error("Desktop post-commit worker clock failed")]
    Clock,
    /// An outbox operation failed or lost its expected claim.
    #[error("Desktop post-commit effect outbox operation failed")]
    Outbox,
    /// Committed Workflow event delivery failed.
    #[error("Desktop committed Workflow event delivery failed")]
    EventDelivery,
    /// A bounded worker task failed to join.
    #[error("Desktop post-commit worker task failed")]
    Task,
}

/// Clock and retry wait boundary owned by the worker.
#[async_trait]
pub trait DesktopPostCommitWorkerClockInterface: Send + Sync {
    /// Returns non-negative UTC milliseconds for durable outbox transitions.
    fn current_post_commit_timestamp(
        &self,
    ) -> Result<DesktopPostCommitTimestamp, DesktopPostCommitWorkerClockError>;
    /// Waits the frozen one-second transient retry interval.
    async fn wait_after_transient_failure(&self);
}

/// System time and Tokio wait implementation.
pub struct SystemDesktopPostCommitWorkerClockAdapterImpl;

#[async_trait]
impl DesktopPostCommitWorkerClockInterface for SystemDesktopPostCommitWorkerClockAdapterImpl {
    fn current_post_commit_timestamp(
        &self,
    ) -> Result<DesktopPostCommitTimestamp, DesktopPostCommitWorkerClockError> {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| DesktopPostCommitWorkerClockError)?
            .as_millis();
        let millis = i64::try_from(millis).map_err(|_| DesktopPostCommitWorkerClockError)?;
        DesktopPostCommitTimestamp::from_epoch_millis(millis)
            .map_err(|_| DesktopPostCommitWorkerClockError)
    }

    async fn wait_after_transient_failure(&self) {
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

/// Result of one bounded worker claim attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DesktopPostCommitWorkerStep {
    /// No Ready effect existed.
    Idle,
    /// One effect completed or was abandoned.
    Progressed,
    /// One transient failure was released to Ready.
    ReleasedForRetry,
}

/// Single closed effect worker with durable claim transitions.
#[derive(Clone)]
pub struct DesktopPostCommitEffectWorker {
    instance_id: DesktopApplicationInstanceId,
    outbox: Arc<dyn DesktopPostCommitEffectOutboxInterface>,
    executor: Arc<dyn DesktopPostCommitEffectExecutorInterface>,
    event_delivery: Arc<dyn DesktopCommittedWorkflowEventDeliveryInterface>,
    clock: Arc<dyn DesktopPostCommitWorkerClockInterface>,
    event_delivery_guard: Arc<tokio::sync::Mutex<()>>,
    cancellation_requested: Arc<AtomicBool>,
    maximum_concurrency: usize,
}

impl DesktopPostCommitEffectWorker {
    /// Wires one process instance and the three focused worker boundaries.
    pub fn try_new(
        instance_id: DesktopApplicationInstanceId,
        outbox: Arc<dyn DesktopPostCommitEffectOutboxInterface>,
        executor: Arc<dyn DesktopPostCommitEffectExecutorInterface>,
        event_delivery: Arc<dyn DesktopCommittedWorkflowEventDeliveryInterface>,
        clock: Arc<dyn DesktopPostCommitWorkerClockInterface>,
        maximum_concurrency: usize,
    ) -> Result<Self, DesktopPostCommitWorkerConfigurationError> {
        if maximum_concurrency == 0 || maximum_concurrency > 8 {
            return Err(DesktopPostCommitWorkerConfigurationError);
        }
        Ok(Self {
            instance_id,
            outbox,
            executor,
            event_delivery,
            clock,
            event_delivery_guard: Arc::new(tokio::sync::Mutex::new(())),
            cancellation_requested: Arc::new(AtomicBool::new(false)),
            maximum_concurrency,
        })
    }

    /// Requests a graceful stop before another effect claim begins.
    pub fn cancel(&self) {
        self.cancellation_requested.store(true, Ordering::Release);
    }

    /// Reports whether graceful worker cancellation has been requested.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancellation_requested.load(Ordering::Acquire)
    }

    /// Repairs committed events, claims at most one effect, then repairs newly committed events.
    pub async fn run_one_effect(
        &self,
    ) -> Result<DesktopPostCommitWorkerStep, DesktopPostCommitWorkerError> {
        if self.is_cancelled() {
            return Ok(DesktopPostCommitWorkerStep::Idle);
        }
        self.deliver_events().await?;
        if self.is_cancelled() {
            return Ok(DesktopPostCommitWorkerStep::Idle);
        }
        let claimed_at = self.timestamp()?;
        let Some(record) = self
            .outbox
            .claim_next_post_commit_effect(self.instance_id, claimed_at)
            .await
            .map_err(|_| DesktopPostCommitWorkerError::Outbox)?
        else {
            return Ok(DesktopPostCommitWorkerStep::Idle);
        };
        let outcome = self.executor.execute_desktop_post_commit_effect(record.effect()).await;
        self.finish_claim(record, outcome).await
    }

    async fn finish_claim(
        &self,
        record: DesktopPostCommitEffectRecord,
        outcome: DesktopPostCommitEffectExecutionOutcome,
    ) -> Result<DesktopPostCommitWorkerStep, DesktopPostCommitWorkerError> {
        match outcome {
            DesktopPostCommitEffectExecutionOutcome::Completed => {
                self.outbox
                    .complete_claimed_post_commit_effect(
                        record.effect_id(),
                        self.instance_id,
                        self.timestamp()?,
                    )
                    .await
                    .map_err(|_| DesktopPostCommitWorkerError::Outbox)?;
                self.deliver_events().await?;
                Ok(DesktopPostCommitWorkerStep::Progressed)
            }
            DesktopPostCommitEffectExecutionOutcome::OwningStateAlreadyTerminal => {
                self.outbox
                    .abandon_claimed_post_commit_effect(
                        record.effect_id(),
                        self.instance_id,
                        self.timestamp()?,
                        DesktopPostCommitEffectAbandonReason::OwningStateAlreadyTerminal,
                    )
                    .await
                    .map_err(|_| DesktopPostCommitWorkerError::Outbox)?;
                self.deliver_events().await?;
                Ok(DesktopPostCommitWorkerStep::Progressed)
            }
            DesktopPostCommitEffectExecutionOutcome::TransientFailure => {
                self.outbox
                    .release_claimed_post_commit_effect(record.effect_id(), self.instance_id)
                    .await
                    .map_err(|_| DesktopPostCommitWorkerError::Outbox)?;
                self.clock.wait_after_transient_failure().await;
                Ok(DesktopPostCommitWorkerStep::ReleasedForRetry)
            }
        }
    }

    fn timestamp(&self) -> Result<DesktopPostCommitTimestamp, DesktopPostCommitWorkerError> {
        self.clock.current_post_commit_timestamp().map_err(|_| DesktopPostCommitWorkerError::Clock)
    }

    async fn deliver_events(&self) -> Result<(), DesktopPostCommitWorkerError> {
        let _guard = self.event_delivery_guard.lock().await;
        self.event_delivery
            .deliver_committed_workflow_run_events(500)
            .await
            .map(|_| ())
            .map_err(|_| DesktopPostCommitWorkerError::EventDelivery)
    }

    /// Processes up to the configured concurrency bound through atomic outbox claims.
    pub async fn run_effect_batch(
        &self,
    ) -> Result<Vec<DesktopPostCommitWorkerStep>, DesktopPostCommitWorkerError> {
        let mut tasks = tokio::task::JoinSet::new();
        for _ in 0..self.maximum_concurrency {
            if self.is_cancelled() {
                break;
            }
            let worker = self.clone();
            tasks.spawn(async move { worker.run_one_effect().await });
        }
        let mut steps = Vec::with_capacity(self.maximum_concurrency);
        while let Some(result) = tasks.join_next().await {
            steps.push(result.map_err(|_| DesktopPostCommitWorkerError::Task)??);
        }
        Ok(steps)
    }
}

#[cfg(test)]
mod tests;
