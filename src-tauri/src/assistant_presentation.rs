//! Typed Assistant presentation events emitted to the process-local React client.

use assistant::interfaces::{AssistantApplicationError, AssistantApplicationError::*};
use async_trait::async_trait;
use serde::Serialize;

use crate::{
    assistant_model_runner::{
        AssistantPresentationEvent, AssistantPresentationEventPayload,
        AssistantPresentationEventPublisherInterface, AssistantToolActivityState,
    },
    workflow_run_event_publisher::DesktopEventEmitterInterface,
};

pub const ASSISTANT_PRESENTATION_EVENT_NAME: &str = "assistant-presentation-event-v1";

#[derive(Clone)]
pub struct TauriAssistantPresentationEventPublisherAdapterImpl {
    emitter: std::sync::Arc<dyn DesktopEventEmitterInterface>,
}

impl TauriAssistantPresentationEventPublisherAdapterImpl {
    #[must_use]
    pub const fn new(emitter: std::sync::Arc<dyn DesktopEventEmitterInterface>) -> Self {
        Self { emitter }
    }
}

#[async_trait]
impl AssistantPresentationEventPublisherInterface
    for TauriAssistantPresentationEventPublisherAdapterImpl
{
    async fn publish_assistant_presentation_event(
        &self,
        event: AssistantPresentationEvent,
    ) -> Result<(), AssistantApplicationError> {
        let payload = AssistantPresentationEventDto::from_event(event);
        let value = serde_json::to_value(payload).map_err(|_| ProtocolViolation)?;
        if self.emitter.emit_desktop_event(ASSISTANT_PRESENTATION_EVENT_NAME, value).is_err() {
            tracing::warn!("Assistant presentation event delivery failed");
        }
        Ok(())
    }
}

#[derive(Serialize)]
pub struct AssistantPresentationEventDto {
    invocation_id: String,
    sequence: String,
    #[serde(flatten)]
    event: AssistantPresentationEventKindDto,
}

impl AssistantPresentationEventDto {
    fn from_event(event: AssistantPresentationEvent) -> Self {
        Self {
            invocation_id: event.invocation_id.as_uuid().to_string(),
            sequence: event.sequence.to_string(),
            event: AssistantPresentationEventKindDto::from_payload(event.payload),
        }
    }
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum AssistantPresentationEventKindDto {
    TextDelta { text: String },
    ToolActivity { tool_id: String, state: AssistantToolActivityStateDto },
    WorkflowChangeReady { workflow_change_id: String },
    InvocationCompleted,
    InvocationFailed { error: AssistantDesktopErrorDto },
}

impl AssistantPresentationEventKindDto {
    fn from_payload(payload: AssistantPresentationEventPayload) -> Self {
        match payload {
            AssistantPresentationEventPayload::TextDelta { text } => Self::TextDelta { text },
            AssistantPresentationEventPayload::ToolActivity { tool_id, state } => {
                Self::ToolActivity { tool_id, state: state.into() }
            }
            AssistantPresentationEventPayload::WorkflowChangeReady { workflow_change_id } => {
                Self::WorkflowChangeReady {
                    workflow_change_id: workflow_change_id.as_uuid().to_string(),
                }
            }
            AssistantPresentationEventPayload::InvocationCompleted => Self::InvocationCompleted,
            AssistantPresentationEventPayload::InvocationFailed { error } => {
                Self::InvocationFailed { error: AssistantDesktopErrorDto::from_error(error) }
            }
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum AssistantToolActivityStateDto {
    Started,
    Completed,
    Failed,
}

impl From<AssistantToolActivityState> for AssistantToolActivityStateDto {
    fn from(value: AssistantToolActivityState) -> Self {
        match value {
            AssistantToolActivityState::Started => Self::Started,
            AssistantToolActivityState::Completed => Self::Completed,
            AssistantToolActivityState::Failed => Self::Failed,
        }
    }
}

#[derive(Serialize)]
struct AssistantDesktopErrorDto {
    code: &'static str,
    message: &'static str,
}

impl AssistantDesktopErrorDto {
    const fn from_error(error: AssistantApplicationError) -> Self {
        Self { code: error_code(error), message: "Assistant invocation failed" }
    }
}

const fn error_code(error: AssistantApplicationError) -> &'static str {
    match error {
        ProtocolViolation => "assistant_protocol_violation",
        ModelUnavailable => "assistant_model_unavailable",
        DeadlineExceeded => "assistant_deadline_exceeded",
        ConcurrentInvocation => "assistant_concurrent_invocation",
        PendingApprovalExists => "assistant_pending_approval_exists",
        _ => "assistant_operation_failed",
    }
}
