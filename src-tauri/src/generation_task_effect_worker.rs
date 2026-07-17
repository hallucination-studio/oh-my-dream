//! Claim-serial, bounded Generation Task effect worker.

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use tasks::generation_task::{
    GenerationTaskClockInterface, GenerationTaskEffectExecutorInterface,
    GenerationTaskOutboxReaderInterface,
};

mod executor;
pub use executor::*;

/// Invalid Task worker concurrency configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("Generation Task effect concurrency must be in 1..=8")]
pub struct GenerationTaskEffectWorkerConfigurationError;

/// One worker batch could not progress safely.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum GenerationTaskEffectWorkerError {
    /// The Task clock or outbox could not produce the next claim.
    #[error("Generation Task effect claim failed")]
    Claim,
    /// One claimed effect execution failed.
    #[error("Generation Task effect execution failed")]
    Execution,
    /// One in-flight execution could not be joined.
    #[error("Generation Task effect task failed")]
    Task,
}

/// Result of one bounded claim-and-execute batch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GenerationTaskEffectWorkerStep {
    /// No due effect was available.
    Idle,
    /// One claimed effect completed its durable disposition.
    Progressed,
}

/// Claims Task effects serially and executes them on one bounded pool.
pub struct GenerationTaskEffectWorkerImpl<R, E, C> {
    outbox: Arc<R>,
    executor: Arc<E>,
    clock: Arc<C>,
    cancellation_requested: Arc<AtomicBool>,
    maximum_concurrency: usize,
}

impl<R, E, C> Clone for GenerationTaskEffectWorkerImpl<R, E, C> {
    fn clone(&self) -> Self {
        Self {
            outbox: Arc::clone(&self.outbox),
            executor: Arc::clone(&self.executor),
            clock: Arc::clone(&self.clock),
            cancellation_requested: Arc::clone(&self.cancellation_requested),
            maximum_concurrency: self.maximum_concurrency,
        }
    }
}

impl<R, E, C> GenerationTaskEffectWorkerImpl<R, E, C>
where
    R: GenerationTaskOutboxReaderInterface + 'static,
    E: GenerationTaskEffectExecutorInterface + 'static,
    C: GenerationTaskClockInterface + 'static,
{
    /// Creates the one process worker with the frozen `1..=8` concurrency bound.
    pub fn try_new(
        outbox: Arc<R>,
        executor: Arc<E>,
        clock: Arc<C>,
        maximum_concurrency: usize,
    ) -> Result<Self, GenerationTaskEffectWorkerConfigurationError> {
        if !(1..=8).contains(&maximum_concurrency) {
            return Err(GenerationTaskEffectWorkerConfigurationError);
        }
        Ok(Self {
            outbox,
            executor,
            clock,
            cancellation_requested: Arc::new(AtomicBool::new(false)),
            maximum_concurrency,
        })
    }

    /// Stops new claims; already claimed executions are joined by the active batch.
    pub fn cancel(&self) {
        self.cancellation_requested.store(true, Ordering::Release);
    }

    /// Claims serially, then joins every execution admitted to this batch.
    pub async fn run_effect_batch(
        &self,
    ) -> Result<Vec<GenerationTaskEffectWorkerStep>, GenerationTaskEffectWorkerError> {
        let mut tasks = tokio::task::JoinSet::new();
        for _ in 0..self.maximum_concurrency {
            if self.cancellation_requested.load(Ordering::Acquire) {
                break;
            }
            let Some(claimed) = self.claim_next().await? else {
                break;
            };
            spawn_execution(&mut tasks, Arc::clone(&self.executor), claimed);
        }
        let mut steps = Vec::with_capacity(tasks.len());
        let mut failure = None;
        while let Some(result) = tasks.join_next().await {
            match result {
                Ok(Ok(step)) => steps.push(step),
                Ok(Err(error)) => {
                    failure.get_or_insert(error);
                }
                Err(_) => {
                    failure.get_or_insert(GenerationTaskEffectWorkerError::Task);
                }
            };
            if failure.is_none()
                && !self.cancellation_requested.load(Ordering::Acquire)
                && let Some(claimed) = self.claim_next().await?
            {
                spawn_execution(&mut tasks, Arc::clone(&self.executor), claimed);
            }
        }
        if let Some(error) = failure {
            return Err(error);
        }
        if steps.is_empty() {
            steps.push(GenerationTaskEffectWorkerStep::Idle);
        }
        Ok(steps)
    }

    async fn claim_next(
        &self,
    ) -> Result<
        Option<tasks::generation_task::GenerationTaskClaimedEffect>,
        GenerationTaskEffectWorkerError,
    > {
        let now = self
            .clock
            .observe_generation_task_time()
            .map_err(|_| GenerationTaskEffectWorkerError::Claim)?;
        self.outbox
            .claim_next_generation_task_effect(now)
            .await
            .map_err(|_| GenerationTaskEffectWorkerError::Claim)
    }
}

fn spawn_execution<E: GenerationTaskEffectExecutorInterface + 'static>(
    tasks: &mut tokio::task::JoinSet<
        Result<GenerationTaskEffectWorkerStep, GenerationTaskEffectWorkerError>,
    >,
    executor: Arc<E>,
    claimed: tasks::generation_task::GenerationTaskClaimedEffect,
) {
    tasks.spawn(async move {
        executor
            .execute_generation_task_effect(claimed)
            .await
            .map(|_| GenerationTaskEffectWorkerStep::Progressed)
            .map_err(|_| GenerationTaskEffectWorkerError::Execution)
    });
}

#[cfg(test)]
mod tests;
