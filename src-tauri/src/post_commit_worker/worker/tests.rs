use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use assets::asset::{application::AssetFinalizeContentEffect, domain::AssetContentFinalizationId};
use assistant::{
    application::AssistantApplyWorkflowChangeEffect, domain::AssistantWorkflowChangeId,
};
use async_trait::async_trait;
use engine::{
    node_capability::WorkflowRunId,
    workflow::{WorkflowApplicationError, WorkflowExecuteRunEffect},
};
use tokio::sync::Barrier;
use uuid::Uuid;

use super::*;
use crate::post_commit_effect::{
    DesktopPostCommitEffect, DesktopPostCommitEffectId, DesktopPostCommitEffectOutboxError,
    DesktopPostCommitEffectRecord, DesktopPostCommitEffectState, DesktopPostCommitRecoveryCursor,
    DesktopPostCommitRecoveryLimit, DesktopPostCommitRecoveryPage,
};

#[derive(Default)]
struct FakeOutbox {
    ready: Mutex<VecDeque<DesktopPostCommitEffectRecord>>,
    completed: Mutex<Vec<DesktopPostCommitEffectId>>,
    released: Mutex<Vec<DesktopPostCommitEffectId>>,
    abandoned: Mutex<Vec<DesktopPostCommitEffectId>>,
    claims: AtomicUsize,
}

#[async_trait]
impl DesktopPostCommitEffectOutboxInterface for FakeOutbox {
    async fn claim_next_post_commit_effect(
        &self,
        _instance_id: DesktopApplicationInstanceId,
        _claimed_at: DesktopPostCommitTimestamp,
    ) -> Result<Option<DesktopPostCommitEffectRecord>, DesktopPostCommitEffectOutboxError> {
        self.claims.fetch_add(1, Ordering::SeqCst);
        Ok(self.ready.lock().map_err(|_| storage_error())?.pop_front())
    }

    async fn complete_claimed_post_commit_effect(
        &self,
        effect_id: DesktopPostCommitEffectId,
        _instance_id: DesktopApplicationInstanceId,
        _completed_at: DesktopPostCommitTimestamp,
    ) -> Result<(), DesktopPostCommitEffectOutboxError> {
        self.completed.lock().map_err(|_| storage_error())?.push(effect_id);
        Ok(())
    }

    async fn release_claimed_post_commit_effect(
        &self,
        effect_id: DesktopPostCommitEffectId,
        _instance_id: DesktopApplicationInstanceId,
    ) -> Result<(), DesktopPostCommitEffectOutboxError> {
        self.released.lock().map_err(|_| storage_error())?.push(effect_id);
        Ok(())
    }

    async fn abandon_claimed_post_commit_effect(
        &self,
        effect_id: DesktopPostCommitEffectId,
        _instance_id: DesktopApplicationInstanceId,
        _abandoned_at: DesktopPostCommitTimestamp,
        _reason: DesktopPostCommitEffectAbandonReason,
    ) -> Result<(), DesktopPostCommitEffectOutboxError> {
        self.abandoned.lock().map_err(|_| storage_error())?.push(effect_id);
        Ok(())
    }

    async fn list_recoverable_post_commit_effects(
        &self,
        _current_instance_id: DesktopApplicationInstanceId,
        _cursor: Option<DesktopPostCommitRecoveryCursor>,
        limit: DesktopPostCommitRecoveryLimit,
    ) -> Result<DesktopPostCommitRecoveryPage, DesktopPostCommitEffectOutboxError> {
        DesktopPostCommitRecoveryPage::try_new(Vec::new(), None, limit)
    }

    async fn recover_replayable_post_commit_effect(
        &self,
        _effect_id: DesktopPostCommitEffectId,
        _prior_instance_id: DesktopApplicationInstanceId,
    ) -> Result<(), DesktopPostCommitEffectOutboxError> {
        Ok(())
    }

    async fn recover_abandoned_post_commit_effect(
        &self,
        _effect_id: DesktopPostCommitEffectId,
        _expected_state: DesktopPostCommitEffectState,
        _abandoned_at: DesktopPostCommitTimestamp,
        _reason: DesktopPostCommitEffectAbandonReason,
    ) -> Result<(), DesktopPostCommitEffectOutboxError> {
        Ok(())
    }
}

struct FakeExecutor {
    outcomes: Mutex<VecDeque<DesktopPostCommitEffectExecutionOutcome>>,
    effects: Mutex<Vec<DesktopPostCommitEffect>>,
    barrier: Option<Arc<Barrier>>,
}

#[async_trait]
impl DesktopPostCommitEffectExecutorInterface for FakeExecutor {
    async fn execute_desktop_post_commit_effect(
        &self,
        effect: DesktopPostCommitEffect,
    ) -> DesktopPostCommitEffectExecutionOutcome {
        self.effects.lock().expect("effect lock").push(effect);
        if let Some(barrier) = &self.barrier {
            barrier.wait().await;
        }
        self.outcomes
            .lock()
            .expect("outcome lock")
            .pop_front()
            .unwrap_or(DesktopPostCommitEffectExecutionOutcome::Completed)
    }
}

#[derive(Default)]
struct FakeEventDelivery {
    calls: AtomicUsize,
}

#[async_trait]
impl DesktopCommittedWorkflowEventDeliveryInterface for FakeEventDelivery {
    async fn deliver_committed_workflow_run_events(
        &self,
        _limit: usize,
    ) -> Result<usize, WorkflowApplicationError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(0)
    }
}

#[derive(Default)]
struct FakeClock {
    waits: AtomicUsize,
}

#[async_trait]
impl DesktopPostCommitWorkerClockInterface for FakeClock {
    fn current_post_commit_timestamp(
        &self,
    ) -> Result<DesktopPostCommitTimestamp, DesktopPostCommitWorkerClockError> {
        DesktopPostCommitTimestamp::from_epoch_millis(10)
            .map_err(|_| DesktopPostCommitWorkerClockError)
    }

    async fn wait_after_transient_failure(&self) {
        self.waits.fetch_add(1, Ordering::SeqCst);
    }
}

#[tokio::test]
async fn routes_exact_three_effects_and_completes_each_claim() {
    let records = vec![workflow_record(1), asset_record(2), assistant_record(3)];
    let (worker, outbox, executor, events, _) = worker(records, outcomes(3), None, 1);

    for _ in 0..3 {
        assert_eq!(worker.run_one_effect().await, Ok(DesktopPostCommitWorkerStep::Progressed));
    }

    let effects = executor.effects.lock().expect("effect lock");
    assert!(matches!(effects[0], DesktopPostCommitEffect::Workflow(_)));
    assert!(matches!(effects[1], DesktopPostCommitEffect::Asset(_)));
    assert!(matches!(effects[2], DesktopPostCommitEffect::Assistant(_)));
    assert_eq!(outbox.completed.lock().expect("completed lock").len(), 3);
    assert_eq!(events.calls.load(Ordering::SeqCst), 6);
}

#[tokio::test]
async fn releases_transient_failure_and_waits_once() {
    let (worker, outbox, _, _, clock) = worker(
        vec![workflow_record(4)],
        VecDeque::from([DesktopPostCommitEffectExecutionOutcome::TransientFailure]),
        None,
        1,
    );

    assert_eq!(worker.run_one_effect().await, Ok(DesktopPostCommitWorkerStep::ReleasedForRetry));
    assert_eq!(outbox.released.lock().expect("released lock").len(), 1);
    assert_eq!(clock.waits.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn abandons_effect_when_owning_state_is_terminal() {
    let (worker, outbox, _, _, _) = worker(
        vec![assistant_record(5)],
        VecDeque::from([DesktopPostCommitEffectExecutionOutcome::OwningStateAlreadyTerminal]),
        None,
        1,
    );

    assert_eq!(worker.run_one_effect().await, Ok(DesktopPostCommitWorkerStep::Progressed));
    assert_eq!(outbox.abandoned.lock().expect("abandoned lock").len(), 1);
}

#[tokio::test]
async fn batch_honors_configured_concurrency_and_cancellation_stops_new_claims() {
    let barrier = Arc::new(Barrier::new(4));
    let records = vec![workflow_record(6), asset_record(7), assistant_record(8)];
    let (worker, outbox, _, _, _) = worker(records, outcomes(3), Some(Arc::clone(&barrier)), 3);
    let running_worker = worker.clone();
    let batch = tokio::spawn(async move { running_worker.run_effect_batch().await });
    barrier.wait().await;
    assert_eq!(outbox.claims.load(Ordering::SeqCst), 3);
    assert_eq!(batch.await.expect("batch task").expect("batch").len(), 3);

    worker.cancel();
    assert!(worker.is_cancelled());
    assert!(worker.run_effect_batch().await.expect("cancelled batch").is_empty());
    assert_eq!(outbox.claims.load(Ordering::SeqCst), 3);
}

#[test]
fn rejects_concurrency_outside_frozen_range() {
    let outbox = Arc::new(FakeOutbox::default());
    let executor = Arc::new(FakeExecutor {
        outcomes: Mutex::new(VecDeque::new()),
        effects: Mutex::new(Vec::new()),
        barrier: None,
    });
    let events = Arc::new(FakeEventDelivery::default());
    let clock = Arc::new(FakeClock::default());
    assert!(
        make_worker(outbox.clone(), executor.clone(), events.clone(), clock.clone(), 0).is_err()
    );
    assert!(make_worker(outbox, executor, events, clock, 9).is_err());
}

fn worker(
    records: Vec<DesktopPostCommitEffectRecord>,
    outcomes: VecDeque<DesktopPostCommitEffectExecutionOutcome>,
    barrier: Option<Arc<Barrier>>,
    concurrency: usize,
) -> (
    DesktopPostCommitEffectWorker,
    Arc<FakeOutbox>,
    Arc<FakeExecutor>,
    Arc<FakeEventDelivery>,
    Arc<FakeClock>,
) {
    let outbox =
        Arc::new(FakeOutbox { ready: Mutex::new(records.into()), ..FakeOutbox::default() });
    let executor = Arc::new(FakeExecutor {
        outcomes: Mutex::new(outcomes),
        effects: Mutex::new(Vec::new()),
        barrier,
    });
    let events = Arc::new(FakeEventDelivery::default());
    let clock = Arc::new(FakeClock::default());
    let worker =
        make_worker(outbox.clone(), executor.clone(), events.clone(), clock.clone(), concurrency)
            .expect("worker");
    (worker, outbox, executor, events, clock)
}

fn make_worker(
    outbox: Arc<FakeOutbox>,
    executor: Arc<FakeExecutor>,
    events: Arc<FakeEventDelivery>,
    clock: Arc<FakeClock>,
    concurrency: usize,
) -> Result<DesktopPostCommitEffectWorker, DesktopPostCommitWorkerConfigurationError> {
    DesktopPostCommitEffectWorker::try_new(
        instance_id(90),
        outbox,
        executor,
        events,
        clock,
        concurrency,
    )
}

fn outcomes(count: usize) -> VecDeque<DesktopPostCommitEffectExecutionOutcome> {
    std::iter::repeat_n(DesktopPostCommitEffectExecutionOutcome::Completed, count).collect()
}

fn workflow_record(seed: u128) -> DesktopPostCommitEffectRecord {
    record(
        seed,
        DesktopPostCommitEffect::Workflow(WorkflowExecuteRunEffect {
            workflow_run_id: WorkflowRunId::from_uuid(uuid(seed + 20)).expect("run ID"),
        }),
    )
}

fn asset_record(seed: u128) -> DesktopPostCommitEffectRecord {
    let id = AssetContentFinalizationId::from_uuid(uuid(seed + 30)).expect("finalization ID");
    record(seed, DesktopPostCommitEffect::Asset(AssetFinalizeContentEffect::new(id)))
}

fn assistant_record(seed: u128) -> DesktopPostCommitEffectRecord {
    let id = AssistantWorkflowChangeId::from_uuid(uuid(seed + 40)).expect("change ID");
    record(seed, DesktopPostCommitEffect::Assistant(AssistantApplyWorkflowChangeEffect::new(id)))
}

fn record(seed: u128, effect: DesktopPostCommitEffect) -> DesktopPostCommitEffectRecord {
    DesktopPostCommitEffectRecord::new(
        effect_id(seed),
        effect,
        DesktopPostCommitEffectState::Ready,
        0,
        DesktopPostCommitTimestamp::from_epoch_millis(seed as i64).expect("timestamp"),
    )
}

fn effect_id(seed: u128) -> DesktopPostCommitEffectId {
    DesktopPostCommitEffectId::from_uuid(uuid(seed)).expect("effect ID")
}

fn instance_id(seed: u128) -> DesktopApplicationInstanceId {
    DesktopApplicationInstanceId::from_uuid(uuid(seed)).expect("instance ID")
}

fn uuid(seed: u128) -> Uuid {
    Uuid::from_u128(0x123e_4567_e89b_42d3_a456_4266_0000_0000 | seed)
}

const fn storage_error() -> DesktopPostCommitEffectOutboxError {
    DesktopPostCommitEffectOutboxError::StorageFailure
}
