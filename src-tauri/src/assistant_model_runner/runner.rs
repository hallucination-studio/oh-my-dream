use assistant::{
    application::AssistantToolExecutionContext,
    domain::AssistantModelInvocationId,
    interfaces::{
        AssistantApplicationError, AssistantModelContinuationEnvelope, AssistantModelTurnResult,
    },
    protocol_v1::{
        ASSISTANT_PROTOCOL_VERSION, AssistantProtocolConversationValidator,
        AssistantProtocolDecoder, AssistantProtocolDirection, AssistantProtocolFrame,
        AssistantProtocolPayload, ToolResultPayload,
    },
};
use serde_json::Value;

use super::{
    AssistantPresentationEvent, AssistantPresentationEventPayload,
    AssistantPresentationEventPublisherInterface, AssistantProtocolProcessInterface,
    AssistantProtocolToolExecutorInterface, AssistantReviewerProtocolInterface,
    AssistantToolActivityState,
    exchange_state::{ExchangeContext, ExchangeProgress, ExchangeScope},
    frames::{map_protocol_error, write_frame},
    review_values::{parse_change_id, reviewer_change_id},
};

#[derive(Clone)]
pub struct PythonAgentsAssistantModelRunnerAdapterImpl<L, T, F, P, Q> {
    pub(super) launcher: L,
    pub(super) tool_executor: T,
    pub(super) context_factory: F,
    pub(super) presentation: P,
    pub(super) reviewer: Q,
}
impl<L, T, F, P, Q> PythonAgentsAssistantModelRunnerAdapterImpl<L, T, F, P, Q> {
    #[must_use]
    pub const fn new(
        launcher: L,
        tool_executor: T,
        context_factory: F,
        presentation: P,
        reviewer: Q,
    ) -> Self {
        Self { launcher, tool_executor, context_factory, presentation, reviewer }
    }
}

impl<L, T, F, P, Q> PythonAgentsAssistantModelRunnerAdapterImpl<L, T, F, P, Q>
where
    T: AssistantProtocolToolExecutorInterface,
    P: AssistantPresentationEventPublisherInterface,
    Q: AssistantReviewerProtocolInterface,
{
    pub(super) async fn exchange(
        &self,
        process: &mut dyn AssistantProtocolProcessInterface,
        invocation_id: AssistantModelInvocationId,
        first_frame: AssistantProtocolFrame,
        context: AssistantToolExecutionContext,
        presentation_sequence: &mut u64,
    ) -> Result<AssistantModelTurnResult, AssistantApplicationError> {
        let mut progress = ExchangeProgress { rust_sequence: 1, pending_verdict: None };
        let mut decoder = AssistantProtocolDecoder::new(AssistantProtocolDirection::PythonToRust);
        let mut conversation = AssistantProtocolConversationValidator::default();
        write_frame(process, &mut conversation, &first_frame).await?;
        loop {
            let encoded = process.read_assistant_protocol_line().await?;
            let frame = decoder.decode(&encoded).map_err(map_protocol_error)?;
            conversation
                .admit(AssistantProtocolDirection::PythonToRust, encoded.len(), &frame)
                .map_err(map_protocol_error)?;
            let scope = ExchangeScope {
                invocation_id,
                tool_context: &context,
                presentation_sequence,
                progress: &mut progress,
            };
            if let Some(result) =
                self.handle_payload(process, &mut conversation, frame, scope).await?
            {
                return Ok(result);
            }
        }
    }

    async fn handle_payload(
        &self,
        process: &mut dyn AssistantProtocolProcessInterface,
        conversation: &mut AssistantProtocolConversationValidator,
        frame: AssistantProtocolFrame,
        scope: ExchangeScope<'_>,
    ) -> Result<Option<AssistantModelTurnResult>, AssistantApplicationError> {
        match frame.payload {
            AssistantProtocolPayload::InvocationAccepted(_) => Ok(None),
            AssistantProtocolPayload::ModelOutputDelta(delta) => {
                self.publish_text_delta(
                    scope.invocation_id,
                    scope.presentation_sequence,
                    delta.text,
                )
                .await?;
                Ok(None)
            }
            AssistantProtocolPayload::ToolCall(call) => {
                self.handle_scoped_tool_call(
                    process,
                    conversation,
                    frame.invocation_id,
                    call,
                    scope,
                )
                .await?;
                Ok(None)
            }
            AssistantProtocolPayload::ReviewerVerdict(verdict) => {
                self.handle_reviewer_verdict(
                    scope.tool_context,
                    scope.invocation_id,
                    verdict,
                    &mut scope.progress.pending_verdict,
                )
                .await?;
                Ok(None)
            }
            AssistantProtocolPayload::ContinuationEnvelopeReady(ready) => {
                self.handle_scoped_continuation(ready, scope).await?;
                Ok(None)
            }
            AssistantProtocolPayload::InvocationCompleted(completed) => self
                .complete_invocation(
                    scope.invocation_id,
                    scope.presentation_sequence,
                    completed.final_text,
                    scope.progress.pending_verdict.is_some(),
                )
                .await
                .map(Some),
            AssistantProtocolPayload::InvocationFailed(_) => {
                Err(AssistantApplicationError::ModelUnavailable)
            }
            _ => Ok(None),
        }
    }

    async fn handle_scoped_continuation(
        &self,
        ready: assistant::protocol_v1::ContinuationEnvelopeReadyPayload,
        scope: ExchangeScope<'_>,
    ) -> Result<(), AssistantApplicationError> {
        self.handle_continuation_ready(
            scope.tool_context,
            scope.invocation_id,
            scope.presentation_sequence,
            ready,
            &mut scope.progress.pending_verdict,
        )
        .await
    }

    async fn complete_invocation(
        &self,
        invocation_id: AssistantModelInvocationId,
        sequence: &mut u64,
        final_text: String,
        has_pending_verdict: bool,
    ) -> Result<AssistantModelTurnResult, AssistantApplicationError> {
        if has_pending_verdict {
            return Err(AssistantApplicationError::ContinuationIncompatible);
        }
        let result = AssistantModelTurnResult::new(final_text.into_bytes())?;
        self.publish(
            invocation_id,
            sequence,
            AssistantPresentationEventPayload::InvocationCompleted,
        )
        .await?;
        Ok(result)
    }

    async fn publish_text_delta(
        &self,
        invocation_id: AssistantModelInvocationId,
        sequence: &mut u64,
        text: String,
    ) -> Result<(), AssistantApplicationError> {
        self.publish(invocation_id, sequence, AssistantPresentationEventPayload::TextDelta { text })
            .await
    }

    async fn handle_scoped_tool_call(
        &self,
        process: &mut dyn AssistantProtocolProcessInterface,
        conversation: &mut AssistantProtocolConversationValidator,
        frame_invocation_id: String,
        call: assistant::protocol_v1::ToolCallPayload,
        scope: ExchangeScope<'_>,
    ) -> Result<(), AssistantApplicationError> {
        self.handle_tool_call(
            process,
            conversation,
            frame_invocation_id,
            call,
            ExchangeContext {
                invocation_id: scope.invocation_id,
                tool_context: scope.tool_context,
                presentation_sequence: scope.presentation_sequence,
                rust_sequence: &mut scope.progress.rust_sequence,
            },
        )
        .await
    }

    async fn handle_tool_call(
        &self,
        process: &mut dyn AssistantProtocolProcessInterface,
        conversation: &mut AssistantProtocolConversationValidator,
        frame_invocation_id: String,
        call: assistant::protocol_v1::ToolCallPayload,
        mut exchange: ExchangeContext<'_>,
    ) -> Result<(), AssistantApplicationError> {
        let reviewer_change_id = reviewer_change_id(&call.tool_id, &call.arguments)?;
        self.publish_tool_activity(
            exchange.invocation_id,
            exchange.presentation_sequence,
            &call.tool_id,
            AssistantToolActivityState::Started,
        )
        .await?;
        if let Some(change_id) = reviewer_change_id {
            self.reviewer
                .record_assistant_reviewer_candidate_fetch(
                    exchange.tool_context,
                    exchange.invocation_id,
                    &call.call_id,
                    change_id,
                )
                .await?;
        }
        *exchange.rust_sequence += 1;
        let result = self.execute_tool(&call, &mut exchange).await?;
        let response = AssistantProtocolFrame {
            protocol_version: ASSISTANT_PROTOCOL_VERSION,
            invocation_id: frame_invocation_id,
            direction_sequence: *exchange.rust_sequence,
            payload: AssistantProtocolPayload::ToolResult(ToolResultPayload {
                call_id: call.call_id,
                tool_id: call.tool_id,
                result,
            }),
        };
        write_frame(process, conversation, &response).await
    }

    async fn execute_tool(
        &self,
        call: &assistant::protocol_v1::ToolCallPayload,
        exchange: &mut ExchangeContext<'_>,
    ) -> Result<Value, AssistantApplicationError> {
        let result = self
            .tool_executor
            .execute_assistant_protocol_tool(
                exchange.tool_context.clone(),
                &call.tool_id,
                call.arguments.clone(),
            )
            .await;
        match result {
            Ok(result) => {
                self.publish_tool_activity(
                    exchange.invocation_id,
                    exchange.presentation_sequence,
                    &call.tool_id,
                    AssistantToolActivityState::Completed,
                )
                .await?;
                Ok(result)
            }
            Err(error) => {
                self.publish_tool_activity(
                    exchange.invocation_id,
                    exchange.presentation_sequence,
                    &call.tool_id,
                    AssistantToolActivityState::Failed,
                )
                .await?;
                Err(error)
            }
        }
    }

    async fn publish_tool_activity(
        &self,
        invocation_id: AssistantModelInvocationId,
        sequence: &mut u64,
        tool_id: &str,
        state: AssistantToolActivityState,
    ) -> Result<(), AssistantApplicationError> {
        self.publish(
            invocation_id,
            sequence,
            AssistantPresentationEventPayload::ToolActivity { tool_id: tool_id.to_owned(), state },
        )
        .await
    }

    async fn handle_reviewer_verdict(
        &self,
        context: &AssistantToolExecutionContext,
        invocation_id: AssistantModelInvocationId,
        verdict: assistant::protocol_v1::ReviewerVerdictPayload,
        pending: &mut Option<assistant::protocol_v1::ReviewerVerdictPayload>,
    ) -> Result<(), AssistantApplicationError> {
        if pending.is_some() {
            return Err(AssistantApplicationError::ReviewEvidenceInvalid);
        }
        match verdict.verdict.as_str() {
            "Reject" => self.accept_reviewer_verdict(context, invocation_id, verdict, None).await,
            "Pass" => {
                *pending = Some(verdict);
                Ok(())
            }
            _ => Err(AssistantApplicationError::ReviewEvidenceInvalid),
        }
    }

    async fn handle_continuation_ready(
        &self,
        context: &AssistantToolExecutionContext,
        invocation_id: AssistantModelInvocationId,
        sequence: &mut u64,
        ready: assistant::protocol_v1::ContinuationEnvelopeReadyPayload,
        pending: &mut Option<assistant::protocol_v1::ReviewerVerdictPayload>,
    ) -> Result<(), AssistantApplicationError> {
        let verdict = pending.take().ok_or(AssistantApplicationError::ContinuationIncompatible)?;
        let envelope = AssistantModelContinuationEnvelope::new(
            serde_json::to_vec(&ready.envelope)
                .map_err(|_| AssistantApplicationError::ContinuationIncompatible)?,
        )?;
        let workflow_change_id = parse_change_id(&verdict.change_id)?;
        self.accept_reviewer_verdict(context, invocation_id, verdict, Some(envelope)).await?;
        self.publish(
            invocation_id,
            sequence,
            AssistantPresentationEventPayload::WorkflowChangeReady { workflow_change_id },
        )
        .await
    }

    async fn accept_reviewer_verdict(
        &self,
        context: &AssistantToolExecutionContext,
        invocation_id: AssistantModelInvocationId,
        verdict: assistant::protocol_v1::ReviewerVerdictPayload,
        continuation: Option<AssistantModelContinuationEnvelope>,
    ) -> Result<(), AssistantApplicationError> {
        let change_id = parse_change_id(&verdict.change_id)?;
        self.reviewer
            .accept_assistant_reviewer_verdict(
                context,
                invocation_id,
                change_id,
                &verdict.mutation_digest,
                &verdict.verdict,
                continuation,
            )
            .await
    }

    pub(super) async fn publish(
        &self,
        invocation_id: AssistantModelInvocationId,
        sequence: &mut u64,
        payload: AssistantPresentationEventPayload,
    ) -> Result<(), AssistantApplicationError> {
        *sequence = sequence.checked_add(1).ok_or(AssistantApplicationError::BudgetExceeded)?;
        self.presentation
            .publish_assistant_presentation_event(AssistantPresentationEvent {
                invocation_id,
                sequence: *sequence,
                payload,
            })
            .await
    }
}
