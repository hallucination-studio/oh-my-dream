use assistant::{
    application::AssistantToolExecutionContext,
    domain::{AssistantModelInvocationId, AssistantWorkflowChangeId},
    interfaces::{
        AssistantApplicationError, AssistantModelContinuationEnvelope, AssistantModelResumeRequest,
        AssistantModelTurnRequest,
    },
};
use async_trait::async_trait;
use serde_json::Value;

/// Executes one exact model-requested tool through Rust-owned handlers.
#[async_trait]
pub trait AssistantProtocolToolExecutorInterface: Send + Sync {
    async fn execute_assistant_protocol_tool(
        &self,
        context: AssistantToolExecutionContext,
        tool_id: &str,
        arguments: Value,
    ) -> Result<Value, AssistantApplicationError>;
}

/// Creates one complete Rust-trusted tool context for a model turn.
pub trait AssistantToolExecutionContextFactoryInterface: Send + Sync {
    fn create_assistant_tool_execution_context(
        &self,
        request: &AssistantModelTurnRequest,
    ) -> Result<AssistantToolExecutionContext, AssistantApplicationError>;

    fn create_resumed_assistant_tool_execution_context(
        &self,
        request: &AssistantModelResumeRequest,
    ) -> Result<AssistantToolExecutionContext, AssistantApplicationError>;
}

/// Closed process-scoped presentation event publisher.
#[async_trait]
pub trait AssistantPresentationEventPublisherInterface: Send + Sync {
    async fn publish_assistant_presentation_event(
        &self,
        event: AssistantPresentationEvent,
    ) -> Result<(), AssistantApplicationError>;
}

/// Rust-owned Reviewer evidence and continuation persistence boundary.
#[async_trait]
pub trait AssistantReviewerProtocolInterface: Send + Sync {
    async fn record_assistant_reviewer_candidate_fetch(
        &self,
        context: &AssistantToolExecutionContext,
        invocation_id: AssistantModelInvocationId,
        tool_call_id: &str,
        change_id: AssistantWorkflowChangeId,
    ) -> Result<(), AssistantApplicationError>;

    async fn accept_assistant_reviewer_verdict(
        &self,
        context: &AssistantToolExecutionContext,
        invocation_id: AssistantModelInvocationId,
        change_id: AssistantWorkflowChangeId,
        mutation_digest_hex: &str,
        verdict: &str,
        continuation: Option<AssistantModelContinuationEnvelope>,
    ) -> Result<(), AssistantApplicationError>;
}

/// Exact typed event union delivered to React.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssistantPresentationEvent {
    pub invocation_id: assistant::domain::AssistantModelInvocationId,
    pub sequence: u64,
    pub payload: AssistantPresentationEventPayload,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AssistantPresentationEventPayload {
    TextDelta { text: String },
    ToolActivity { tool_id: String, state: AssistantToolActivityState },
    WorkflowChangeReady { workflow_change_id: assistant::domain::AssistantWorkflowChangeId },
    InvocationCompleted,
    InvocationFailed { error: AssistantApplicationError },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssistantToolActivityState {
    Started,
    Completed,
    Failed,
}

/// One isolated raw Assistant protocol process.
#[async_trait]
pub trait AssistantProtocolProcessInterface: Send {
    async fn read_assistant_protocol_line(&mut self) -> Result<Vec<u8>, AssistantApplicationError>;
    async fn write_assistant_protocol_line(
        &mut self,
        encoded: &[u8],
    ) -> Result<(), AssistantApplicationError>;
    async fn shutdown_assistant_protocol_process(
        &mut self,
    ) -> Result<(), AssistantApplicationError>;
    async fn abort_assistant_protocol_process(&mut self);
}

/// Launches one isolated Assistant protocol process per invocation.
#[async_trait]
pub trait AssistantProtocolProcessLauncherInterface: Send + Sync {
    async fn launch_assistant_protocol_process(
        &self,
    ) -> Result<Box<dyn AssistantProtocolProcessInterface>, AssistantApplicationError>;
}
