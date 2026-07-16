use super::*;

#[derive(Clone, Copy)]
struct ReviewerToolExecutorFakeImpl;

#[async_trait]
impl AssistantProtocolToolExecutorInterface for ReviewerToolExecutorFakeImpl {
    async fn execute_assistant_protocol_tool(
        &self,
        _context: AssistantToolExecutionContext,
        tool_id: &str,
        arguments: Value,
    ) -> Result<Value, AssistantApplicationError> {
        assert_eq!(tool_id, "assistant.workflow.get_change@1");
        assert_eq!(arguments, json!({"change_id": uuid(7).to_string()}));
        Ok(json!({
            "change_id": uuid(7).to_string(),
            "mutation_digest_hex": "00".repeat(32),
        }))
    }
}

#[derive(Clone, Default)]
struct ReviewerRecorderImpl {
    fetched: Arc<Mutex<Vec<String>>>,
    accepted: Arc<Mutex<Vec<String>>>,
}

#[async_trait]
impl AssistantReviewerProtocolInterface for ReviewerRecorderImpl {
    async fn record_assistant_reviewer_candidate_fetch(
        &self,
        _context: &AssistantToolExecutionContext,
        _invocation_id: AssistantModelInvocationId,
        tool_call_id: &str,
        _change_id: assistant::domain::AssistantWorkflowChangeId,
    ) -> Result<(), AssistantApplicationError> {
        self.fetched.lock().unwrap().push(tool_call_id.to_owned());
        Ok(())
    }

    async fn accept_assistant_reviewer_verdict(
        &self,
        _context: &AssistantToolExecutionContext,
        _invocation_id: AssistantModelInvocationId,
        _change_id: assistant::domain::AssistantWorkflowChangeId,
        _mutation_digest_hex: &str,
        verdict: &str,
        continuation: Option<assistant::interfaces::AssistantModelContinuationEnvelope>,
    ) -> Result<(), AssistantApplicationError> {
        assert!(continuation.is_some());
        self.accepted.lock().unwrap().push(verdict.to_owned());
        Ok(())
    }
}

#[derive(Clone, Default)]
struct PresentationRecorderImpl(Arc<Mutex<Vec<AssistantPresentationEvent>>>);

#[async_trait]
impl AssistantPresentationEventPublisherInterface for PresentationRecorderImpl {
    async fn publish_assistant_presentation_event(
        &self,
        event: AssistantPresentationEvent,
    ) -> Result<(), AssistantApplicationError> {
        self.0.lock().unwrap().push(event);
        Ok(())
    }
}

#[tokio::test]
async fn runner_persists_exact_reviewer_evidence_before_announcing_pending_change() {
    let tool_ids = tool_ids();
    let (launcher, _) = launcher([
        incoming(1, "InvocationAccepted", json!({"agent_id": "workflow_coauthor@1"})),
        incoming(
            2,
            "ToolCall",
            json!({
                "call_id": "review-fetch",
                "tool_id": "assistant.workflow.get_change@1",
                "arguments": {"change_id": uuid(7).to_string()},
            }),
        ),
        incoming(
            3,
            "ReviewerVerdict",
            json!({
                "change_id": uuid(7).to_string(),
                "mutation_digest": "00".repeat(32),
                "verdict": "Pass",
                "prose": "Ready",
            }),
        ),
        incoming(
            4,
            "ContinuationEnvelopeReady",
            json!({"envelope": {
                "protocol_version": 1,
                "contract_epoch": 1,
                "sdk_version": "0.18.1",
                "agent_id": "workflow_coauthor@1",
                "tool_ids": tool_ids,
                "opaque_state": "state",
            }}),
        ),
        incoming(5, "InvocationCompleted", json!({"final_text": "Awaiting human approval"})),
    ]);
    let reviewer = ReviewerRecorderImpl::default();
    let presentation = PresentationRecorderImpl::default();
    let runner = PythonAgentsAssistantModelRunnerAdapterImpl::new(
        launcher,
        ReviewerToolExecutorFakeImpl,
        crate::assistant_tool_runtime::DesktopAssistantToolExecutionContextFactoryAdapterImpl::new(
            Arc::new(ClockFakeImpl),
            60_000,
        ),
        presentation.clone(),
        reviewer.clone(),
    );

    runner.start_assistant_model_turn(request()).await.unwrap();

    assert_eq!(*reviewer.fetched.lock().unwrap(), ["review-fetch"]);
    assert_eq!(*reviewer.accepted.lock().unwrap(), ["Pass"]);
    assert!(presentation.0.lock().unwrap().iter().any(|event| {
        matches!(event.payload, AssistantPresentationEventPayload::WorkflowChangeReady { .. })
    }));
}

fn tool_ids() -> Vec<String> {
    assistant::application::AssistantToolCatalog::try_new()
        .unwrap()
        .contracts()
        .iter()
        .map(|contract| contract.id().as_str().to_owned())
        .collect()
}
