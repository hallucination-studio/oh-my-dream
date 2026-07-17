use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use async_trait::async_trait;
use tasks::generation_task::*;
use tokio::sync::{Barrier, Notify};
use uuid::Uuid;

use super::*;

struct OutboxFakeImpl {
    ready: Mutex<VecDeque<GenerationTaskClaimedEffect>>,
    claims: Mutex<Vec<GenerationTaskEffectId>>,
}

#[async_trait]
impl GenerationTaskOutboxReaderInterface for OutboxFakeImpl {
    async fn claim_next_generation_task_effect(
        &self,
        _: GenerationTaskTimestamp,
    ) -> Result<Option<GenerationTaskClaimedEffect>, GenerationTaskRepositoryError> {
        let claimed = self.ready.lock().unwrap().pop_front();
        if let Some(value) = &claimed {
            self.claims.lock().unwrap().push(value.claim().effect_id());
        }
        Ok(claimed)
    }

    async fn reset_claimed_generation_task_effects(
        &self,
    ) -> Result<u64, GenerationTaskRepositoryError> {
        panic!("a live worker must never reset claims")
    }
}

struct ExecutorFakeImpl {
    active: AtomicUsize,
    maximum_active: AtomicUsize,
    completed: AtomicUsize,
    slow_task: GenerationTaskId,
    failed_task: Option<GenerationTaskId>,
    slow_started: Notify,
    release_slow: Barrier,
}

#[async_trait]
impl GenerationTaskEffectExecutorInterface for ExecutorFakeImpl {
    async fn execute_generation_task_effect(
        &self,
        claimed: GenerationTaskClaimedEffect,
    ) -> Result<(), GenerationTaskApplicationError> {
        let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.maximum_active.fetch_max(active, Ordering::SeqCst);
        if claimed.effect().task_id() == self.slow_task {
            self.slow_started.notify_one();
            self.release_slow.wait().await;
        }
        if Some(claimed.effect().task_id()) == self.failed_task {
            self.active.fetch_sub(1, Ordering::SeqCst);
            return Err(GenerationTaskApplicationError::InvalidEffect);
        }
        self.completed.fetch_add(1, Ordering::SeqCst);
        self.active.fetch_sub(1, Ordering::SeqCst);
        Ok(())
    }
}

struct ClockFakeImpl;

impl GenerationTaskClockInterface for ClockFakeImpl {
    fn observe_generation_task_time(
        &self,
    ) -> Result<GenerationTaskTimestamp, GenerationTaskBoundaryError> {
        GenerationTaskTimestamp::from_utc_milliseconds(100)
            .map_err(|_| GenerationTaskBoundaryError::Permanent)
    }
}

#[tokio::test]
async fn serial_claims_feed_bounded_pool_without_slow_task_head_of_line_blocking() {
    let slow_task = task_id(1);
    let outbox = Arc::new(OutboxFakeImpl {
        ready: Mutex::new(VecDeque::from([
            claimed(1, slow_task),
            claimed(2, task_id(2)),
            claimed(3, task_id(3)),
            claimed(4, task_id(4)),
            claimed(5, task_id(5)),
        ])),
        claims: Mutex::new(Vec::new()),
    });
    let executor = Arc::new(ExecutorFakeImpl {
        active: AtomicUsize::new(0),
        maximum_active: AtomicUsize::new(0),
        completed: AtomicUsize::new(0),
        slow_task,
        failed_task: None,
        slow_started: Notify::new(),
        release_slow: Barrier::new(2),
    });
    let worker = GenerationTaskEffectWorkerImpl::try_new(
        Arc::clone(&outbox),
        Arc::clone(&executor),
        Arc::new(ClockFakeImpl),
        3,
    )
    .unwrap();

    let running = worker.clone();
    let batch = tokio::spawn(async move { running.run_effect_batch().await });
    executor.slow_started.notified().await;
    tokio::task::yield_now().await;
    tokio::time::timeout(std::time::Duration::from_millis(100), async {
        while executor.completed.load(Ordering::SeqCst) < 4 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        outbox.claims.lock().unwrap().as_slice(),
        [effect_id(1), effect_id(2), effect_id(3), effect_id(4), effect_id(5)]
    );
    assert!(executor.maximum_active.load(Ordering::SeqCst) <= 3);
    executor.release_slow.wait().await;
    assert_eq!(batch.await.unwrap().unwrap().len(), 5);
}

#[tokio::test]
async fn execution_failure_still_joins_other_claimed_effects() {
    let slow_task = task_id(2);
    let outbox = Arc::new(OutboxFakeImpl {
        ready: Mutex::new(VecDeque::from([claimed(1, task_id(1)), claimed(2, slow_task)])),
        claims: Mutex::new(Vec::new()),
    });
    let executor = Arc::new(ExecutorFakeImpl {
        active: AtomicUsize::new(0),
        maximum_active: AtomicUsize::new(0),
        completed: AtomicUsize::new(0),
        slow_task,
        failed_task: Some(task_id(1)),
        slow_started: Notify::new(),
        release_slow: Barrier::new(2),
    });
    let worker = GenerationTaskEffectWorkerImpl::try_new(
        outbox,
        Arc::clone(&executor),
        Arc::new(ClockFakeImpl),
        2,
    )
    .unwrap();

    let batch = tokio::spawn(async move { worker.run_effect_batch().await });
    executor.slow_started.notified().await;
    executor.release_slow.wait().await;

    assert_eq!(batch.await.unwrap(), Err(GenerationTaskEffectWorkerError::Execution));
    assert_eq!(executor.completed.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn cancellation_stops_new_claims() {
    let (worker, outbox) = idle_worker(1);
    worker.cancel();

    assert_eq!(
        worker.run_effect_batch().await.unwrap(),
        vec![GenerationTaskEffectWorkerStep::Idle]
    );
    assert!(outbox.claims.lock().unwrap().is_empty());
}

#[test]
fn rejects_concurrency_outside_frozen_bounds() {
    assert!(idle_worker_result(0).is_err());
    assert!(idle_worker_result(9).is_err());
}

fn idle_worker(
    concurrency: usize,
) -> (
    GenerationTaskEffectWorkerImpl<OutboxFakeImpl, ExecutorFakeImpl, ClockFakeImpl>,
    Arc<OutboxFakeImpl>,
) {
    let outbox = Arc::new(OutboxFakeImpl {
        ready: Mutex::new(VecDeque::from([claimed(1, task_id(1))])),
        claims: Mutex::new(Vec::new()),
    });
    let worker = GenerationTaskEffectWorkerImpl::try_new(
        Arc::clone(&outbox),
        executor(task_id(9)),
        Arc::new(ClockFakeImpl),
        concurrency,
    )
    .unwrap();
    (worker, outbox)
}

fn idle_worker_result(
    concurrency: usize,
) -> Result<
    GenerationTaskEffectWorkerImpl<OutboxFakeImpl, ExecutorFakeImpl, ClockFakeImpl>,
    GenerationTaskEffectWorkerConfigurationError,
> {
    GenerationTaskEffectWorkerImpl::try_new(
        Arc::new(OutboxFakeImpl {
            ready: Mutex::new(VecDeque::new()),
            claims: Mutex::new(Vec::new()),
        }),
        executor(task_id(9)),
        Arc::new(ClockFakeImpl),
        concurrency,
    )
}

fn executor(slow_task: GenerationTaskId) -> Arc<ExecutorFakeImpl> {
    Arc::new(ExecutorFakeImpl {
        active: AtomicUsize::new(0),
        maximum_active: AtomicUsize::new(0),
        completed: AtomicUsize::new(0),
        slow_task,
        failed_task: None,
        slow_started: Notify::new(),
        release_slow: Barrier::new(1),
    })
}

fn claimed(seed: u64, task_id: GenerationTaskId) -> GenerationTaskClaimedEffect {
    GenerationTaskClaimedEffect::new(
        GenerationTaskEffectClaim::new(effect_id(seed)),
        GenerationTaskEffect::new(
            task_id,
            GenerationTaskEffectKind::SubmitTask,
            GenerationTaskTimestamp::from_utc_milliseconds(1).unwrap(),
        ),
    )
}

fn effect_id(value: u64) -> GenerationTaskEffectId {
    GenerationTaskEffectId::try_new(value).unwrap()
}

fn task_id(seed: u8) -> GenerationTaskId {
    let mut bytes = [seed; 16];
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    GenerationTaskId::from_uuid(Uuid::from_bytes(bytes)).unwrap()
}
