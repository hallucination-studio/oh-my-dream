//! Tauri delivery adapter for exact committed Workflow Run events.

use std::sync::Arc;

use async_trait::async_trait;
use engine::workflow::{
    WorkflowApplicationError, WorkflowRunEvent, WorkflowRunEventPublisherInterface,
};
use serde_json::Value;
use tauri::Emitter;

/// Desktop event emission failed at the Tauri boundary.
#[derive(Clone, Copy, Debug, thiserror::Error, PartialEq, Eq)]
#[error("Desktop event emission failed")]
pub struct DesktopEventEmissionError;

/// Minimal event-emission boundary used for deterministic publisher tests.
pub trait DesktopEventEmitterInterface: Send + Sync {
    /// Emits one serializable value under an exact event name.
    fn emit_desktop_event(
        &self,
        event_name: &str,
        payload: Value,
    ) -> Result<(), DesktopEventEmissionError>;
}

/// Tauri AppHandle-backed event emitter.
pub struct TauriAppHandleEventEmitterAdapterImpl {
    app_handle: tauri::AppHandle,
}

impl TauriAppHandleEventEmitterAdapterImpl {
    /// Wraps the running Tauri application handle.
    #[must_use]
    pub fn new(app_handle: tauri::AppHandle) -> Self {
        Self { app_handle }
    }
}

impl DesktopEventEmitterInterface for TauriAppHandleEventEmitterAdapterImpl {
    fn emit_desktop_event(
        &self,
        event_name: &str,
        payload: Value,
    ) -> Result<(), DesktopEventEmissionError> {
        self.app_handle.emit(event_name, payload).map_err(|_| DesktopEventEmissionError)
    }
}

/// Publishes the exact `workflow-run-event-v1` projection without changing sequence.
pub struct TauriWorkflowRunEventPublisherAdapterImpl {
    emitter: Arc<dyn DesktopEventEmitterInterface>,
}

impl TauriWorkflowRunEventPublisherAdapterImpl {
    /// Wires one Desktop event emitter.
    #[must_use]
    pub fn new(emitter: Arc<dyn DesktopEventEmitterInterface>) -> Self {
        Self { emitter }
    }
}

#[async_trait]
impl WorkflowRunEventPublisherInterface for TauriWorkflowRunEventPublisherAdapterImpl {
    async fn publish_committed_workflow_run_event(
        &self,
        event: WorkflowRunEvent,
    ) -> Result<(), WorkflowApplicationError> {
        let payload = crate::workflow_command_dto::event_dto(&event);
        self.emitter
            .emit_desktop_event("workflow-run-event-v1", payload)
            .map_err(|_| WorkflowApplicationError::WorkflowRunEventPublishFailure)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use engine::{
        node_capability::WorkflowRunId,
        workflow::{WorkflowRunEventPayload, WorkflowRunEventSequence, WorkflowRunTime},
    };

    use super::*;

    #[derive(Default)]
    struct EmitterFakeImpl {
        events: Mutex<Vec<(String, Value)>>,
        fail: bool,
    }

    impl DesktopEventEmitterInterface for EmitterFakeImpl {
        fn emit_desktop_event(
            &self,
            event_name: &str,
            payload: Value,
        ) -> Result<(), DesktopEventEmissionError> {
            if self.fail {
                return Err(DesktopEventEmissionError);
            }
            self.events
                .lock()
                .map_err(|_| DesktopEventEmissionError)?
                .push((event_name.to_owned(), payload));
            Ok(())
        }
    }

    #[tokio::test]
    async fn publisher_preserves_committed_identity_sequence_and_failure() {
        let emitter = Arc::new(EmitterFakeImpl::default());
        let publisher = TauriWorkflowRunEventPublisherAdapterImpl::new(emitter.clone());
        let event = event();

        publisher.publish_committed_workflow_run_event(event.clone()).await.unwrap();
        {
            let emitted = emitter.events.lock().unwrap();
            assert_eq!(emitted[0].0, "workflow-run-event-v1");
            assert_eq!(emitted[0].1["sequence"], "7");
            assert_eq!(
                emitted[0].1["workflow_run_id"],
                event.run_id().as_uuid().hyphenated().to_string()
            );
            assert_eq!(emitted[0].1["payload"]["type"], "run_started");
        }

        let failing = TauriWorkflowRunEventPublisherAdapterImpl::new(Arc::new(EmitterFakeImpl {
            events: Mutex::default(),
            fail: true,
        }));
        assert_eq!(
            failing.publish_committed_workflow_run_event(event).await,
            Err(WorkflowApplicationError::WorkflowRunEventPublishFailure)
        );
    }

    fn event() -> WorkflowRunEvent {
        let run_id = WorkflowRunId::from_uuid(uuid::Uuid::from_bytes([
            1, 0, 0, 0, 0, 0, 0x40, 0, 0x80, 0, 0, 0, 0, 0, 0, 1,
        ]))
        .unwrap();
        WorkflowRunEvent::restore(
            run_id,
            WorkflowRunEventSequence::new(7).unwrap(),
            WorkflowRunTime::from_utc_milliseconds(9).unwrap(),
            WorkflowRunEventPayload::WorkflowRunStartedEvent,
        )
    }
}
