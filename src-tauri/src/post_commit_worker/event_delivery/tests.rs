use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use engine::{
    node_capability::WorkflowRunId,
    workflow::{
        WorkflowApplicationError, WorkflowRunEvent, WorkflowRunEventPayload,
        WorkflowRunEventPublisherInterface, WorkflowRunEventSequence, WorkflowRunTime,
    },
};
use uuid::Uuid;

use super::*;

#[derive(Default)]
struct FakeEventRepositoryImpl {
    events: Mutex<Vec<WorkflowRunEvent>>,
    attempts: Mutex<Vec<(u64, bool)>>,
}

#[async_trait]
impl DesktopCommittedWorkflowEventRepositoryInterface for FakeEventRepositoryImpl {
    async fn list_undelivered_committed_workflow_run_events(
        &self,
        limit: usize,
    ) -> Result<Vec<WorkflowRunEvent>, WorkflowApplicationError> {
        Ok(self.events.lock().expect("event lock").iter().take(limit).cloned().collect())
    }

    async fn record_committed_workflow_run_event_delivery_attempt(
        &self,
        event: &WorkflowRunEvent,
        delivered: bool,
    ) -> Result<(), WorkflowApplicationError> {
        self.attempts.lock().expect("attempt lock").push((event.sequence().get(), delivered));
        Ok(())
    }
}

struct FakePublisherImpl {
    fail_sequence: Option<u64>,
}

#[async_trait]
impl WorkflowRunEventPublisherInterface for FakePublisherImpl {
    async fn publish_committed_workflow_run_event(
        &self,
        event: WorkflowRunEvent,
    ) -> Result<(), WorkflowApplicationError> {
        if self.fail_sequence == Some(event.sequence().get()) {
            Err(WorkflowApplicationError::WorkflowRunEventPublishFailure)
        } else {
            Ok(())
        }
    }
}

#[tokio::test]
async fn records_every_successful_delivery_attempt() {
    let repository = Arc::new(FakeEventRepositoryImpl {
        events: Mutex::new(vec![event(1), event(2)]),
        ..FakeEventRepositoryImpl::default()
    });
    let delivery = DesktopCommittedWorkflowEventDeliveryAdapterImpl::new(
        repository.clone(),
        Arc::new(FakePublisherImpl { fail_sequence: None }),
    );

    assert_eq!(delivery.deliver_committed_workflow_run_events(2).await, Ok(2));
    assert_eq!(*repository.attempts.lock().expect("attempt lock"), vec![(1, true), (2, true)]);
}

#[tokio::test]
async fn records_failed_attempt_and_stops_before_later_event() {
    let repository = Arc::new(FakeEventRepositoryImpl {
        events: Mutex::new(vec![event(1), event(2), event(3)]),
        ..FakeEventRepositoryImpl::default()
    });
    let delivery = DesktopCommittedWorkflowEventDeliveryAdapterImpl::new(
        repository.clone(),
        Arc::new(FakePublisherImpl { fail_sequence: Some(2) }),
    );

    assert_eq!(
        delivery.deliver_committed_workflow_run_events(3).await,
        Err(WorkflowApplicationError::WorkflowRunEventPublishFailure)
    );
    assert_eq!(*repository.attempts.lock().expect("attempt lock"), vec![(1, true), (2, false)]);
}

#[tokio::test]
async fn rejects_delivery_limit_outside_frozen_bound() {
    let delivery = DesktopCommittedWorkflowEventDeliveryAdapterImpl::new(
        Arc::new(FakeEventRepositoryImpl::default()),
        Arc::new(FakePublisherImpl { fail_sequence: None }),
    );

    assert!(delivery.deliver_committed_workflow_run_events(0).await.is_err());
    assert!(delivery.deliver_committed_workflow_run_events(501).await.is_err());
}

fn event(sequence: u64) -> WorkflowRunEvent {
    WorkflowRunEvent::restore(
        WorkflowRunId::from_uuid(Uuid::from_u128(0x123e_4567_e89b_42d3_a456_4266_0000_0010))
            .expect("run ID"),
        WorkflowRunEventSequence::new(sequence).expect("sequence"),
        WorkflowRunTime::from_utc_milliseconds(sequence as i64).expect("time"),
        WorkflowRunEventPayload::WorkflowRunQueuedEvent,
    )
}
