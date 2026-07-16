use std::sync::Arc;

use async_trait::async_trait;
use engine::workflow::{
    WorkflowApplicationError, WorkflowRunEvent, WorkflowRunEventPublisherInterface,
};

use crate::workflow_storage_adapters::SqliteWorkflowRunRepositoryAdapterImpl;

/// Bounded committed Workflow event repair boundary consumed by the worker.
#[async_trait]
pub trait DesktopCommittedWorkflowEventDeliveryInterface: Send + Sync {
    /// Publishes up to `limit` undelivered events and records every attempt.
    async fn deliver_committed_workflow_run_events(
        &self,
        limit: usize,
    ) -> Result<usize, WorkflowApplicationError>;
}

/// Committed event-outbox operations consumed by Desktop delivery.
#[async_trait]
pub trait DesktopCommittedWorkflowEventRepositoryInterface: Send + Sync {
    /// Loads only committed events not yet successfully delivered.
    async fn list_undelivered_committed_workflow_run_events(
        &self,
        limit: usize,
    ) -> Result<Vec<WorkflowRunEvent>, WorkflowApplicationError>;

    /// Records one publication attempt and its exact success outcome.
    async fn record_committed_workflow_run_event_delivery_attempt(
        &self,
        event: &WorkflowRunEvent,
        delivered: bool,
    ) -> Result<(), WorkflowApplicationError>;
}

/// SQLite event-outbox reader joined to one Desktop publisher.
pub struct DesktopCommittedWorkflowEventDeliveryAdapterImpl {
    repository: Arc<dyn DesktopCommittedWorkflowEventRepositoryInterface>,
    publisher: Arc<dyn WorkflowRunEventPublisherInterface>,
}

impl DesktopCommittedWorkflowEventDeliveryAdapterImpl {
    /// Wires the event outbox and projection publisher.
    #[must_use]
    pub fn new(
        repository: Arc<dyn DesktopCommittedWorkflowEventRepositoryInterface>,
        publisher: Arc<dyn WorkflowRunEventPublisherInterface>,
    ) -> Self {
        Self { repository, publisher }
    }
}

#[async_trait]
impl DesktopCommittedWorkflowEventDeliveryInterface
    for DesktopCommittedWorkflowEventDeliveryAdapterImpl
{
    async fn deliver_committed_workflow_run_events(
        &self,
        limit: usize,
    ) -> Result<usize, WorkflowApplicationError> {
        if limit == 0 || limit > 500 {
            return Err(WorkflowApplicationError::WorkflowRunEventLimitOutOfBounds {
                requested_limit: u16::try_from(limit).unwrap_or(u16::MAX),
            });
        }
        let events = self.repository.list_undelivered_committed_workflow_run_events(limit).await?;
        let mut delivered = 0;
        for event in events {
            let result = self.publisher.publish_committed_workflow_run_event(event.clone()).await;
            self.repository
                .record_committed_workflow_run_event_delivery_attempt(&event, result.is_ok())
                .await?;
            match result {
                Ok(()) => delivered += 1,
                Err(error) => return Err(error),
            }
        }
        Ok(delivered)
    }
}

#[async_trait]
impl DesktopCommittedWorkflowEventRepositoryInterface for SqliteWorkflowRunRepositoryAdapterImpl {
    async fn list_undelivered_committed_workflow_run_events(
        &self,
        limit: usize,
    ) -> Result<Vec<WorkflowRunEvent>, WorkflowApplicationError> {
        self.list_undelivered_workflow_run_events(limit).await
    }

    async fn record_committed_workflow_run_event_delivery_attempt(
        &self,
        event: &WorkflowRunEvent,
        delivered: bool,
    ) -> Result<(), WorkflowApplicationError> {
        self.record_workflow_run_event_delivery_attempt(event.run_id(), event.sequence(), delivered)
            .await
    }
}

#[cfg(test)]
mod tests;
