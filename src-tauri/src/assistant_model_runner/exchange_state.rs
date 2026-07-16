use assistant::{application::AssistantToolExecutionContext, domain::AssistantModelInvocationId};

pub(super) struct ExchangeContext<'a> {
    pub invocation_id: AssistantModelInvocationId,
    pub tool_context: &'a AssistantToolExecutionContext,
    pub presentation_sequence: &'a mut u64,
    pub rust_sequence: &'a mut u64,
}

pub(super) struct ExchangeProgress {
    pub rust_sequence: u64,
    pub pending_verdict: Option<assistant::protocol_v1::ReviewerVerdictPayload>,
}

pub(super) struct ExchangeScope<'a> {
    pub invocation_id: AssistantModelInvocationId,
    pub tool_context: &'a AssistantToolExecutionContext,
    pub presentation_sequence: &'a mut u64,
    pub progress: &'a mut ExchangeProgress,
}
