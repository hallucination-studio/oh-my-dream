use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

use assistant::{
    application::AssistantToolExecutionContext,
    domain::{
        AssistantModelInvocationId, AssistantReviewedAt, AssistantSessionId, AssistantUserIntent,
    },
    interfaces::{
        AssistantApplicationError, AssistantClockInterface, AssistantModelRunnerInterface,
        AssistantModelTurnRequest, AssistantModelTurnStart, AssistantWorkspaceSnapshot,
        AssistantWorkspaceSnapshotRequest,
    },
};
use async_trait::async_trait;
use projects::project::domain::ProjectId;
use serde_json::{Value, json};
use uuid::Uuid;

use super::*;

#[derive(Clone)]
struct LauncherFakeImpl {
    state: Arc<Mutex<ProcessState>>,
}

struct ProcessState {
    reads: VecDeque<Vec<u8>>,
    writes: Vec<Vec<u8>>,
    shutdown: bool,
    shutdown_error: bool,
    aborted: bool,
}

struct ProcessFakeImpl {
    state: Arc<Mutex<ProcessState>>,
}

#[async_trait]
impl AssistantProtocolProcessLauncherInterface for LauncherFakeImpl {
    async fn launch_assistant_protocol_process(
        &self,
    ) -> Result<Box<dyn AssistantProtocolProcessInterface>, AssistantApplicationError> {
        Ok(Box::new(ProcessFakeImpl { state: Arc::clone(&self.state) }))
    }
}

#[async_trait]
impl AssistantProtocolProcessInterface for ProcessFakeImpl {
    async fn read_assistant_protocol_line(&mut self) -> Result<Vec<u8>, AssistantApplicationError> {
        self.state
            .lock()
            .map_err(|_| AssistantApplicationError::ExternalBoundaryFailed)?
            .reads
            .pop_front()
            .ok_or(AssistantApplicationError::ProtocolViolation)
    }

    async fn write_assistant_protocol_line(
        &mut self,
        encoded: &[u8],
    ) -> Result<(), AssistantApplicationError> {
        self.state
            .lock()
            .map_err(|_| AssistantApplicationError::ExternalBoundaryFailed)?
            .writes
            .push(encoded.to_vec());
        Ok(())
    }

    async fn shutdown_assistant_protocol_process(
        &mut self,
    ) -> Result<(), AssistantApplicationError> {
        self.state
            .lock()
            .map_err(|_| AssistantApplicationError::ExternalBoundaryFailed)?
            .shutdown = true;
        if self
            .state
            .lock()
            .map_err(|_| AssistantApplicationError::ExternalBoundaryFailed)?
            .shutdown_error
        {
            Err(AssistantApplicationError::ProtocolViolation)
        } else {
            Ok(())
        }
    }

    async fn abort_assistant_protocol_process(&mut self) {
        if let Ok(mut state) = self.state.lock() {
            state.aborted = true;
        }
    }
}

#[derive(Clone, Copy)]
struct ToolExecutorFakeImpl;

#[async_trait]
impl AssistantProtocolToolExecutorInterface for ToolExecutorFakeImpl {
    async fn execute_assistant_protocol_tool(
        &self,
        _context: AssistantToolExecutionContext,
        tool_id: &str,
        arguments: Value,
    ) -> Result<Value, AssistantApplicationError> {
        assert_eq!(tool_id, "assistant.workspace.get_snapshot@1");
        assert_eq!(arguments, json!({}));
        Ok(json!({"snapshot": {}}))
    }
}

#[derive(Clone, Copy)]
struct ClockFakeImpl;

impl AssistantClockInterface for ClockFakeImpl {
    fn current_assistant_time(&self) -> Result<AssistantReviewedAt, AssistantApplicationError> {
        AssistantReviewedAt::new(1_000)
            .map_err(|_| AssistantApplicationError::ExternalBoundaryFailed)
    }
}

#[derive(Clone, Copy)]
struct PresentationFakeImpl;

#[async_trait]
impl AssistantPresentationEventPublisherInterface for PresentationFakeImpl {
    async fn publish_assistant_presentation_event(
        &self,
        _event: AssistantPresentationEvent,
    ) -> Result<(), AssistantApplicationError> {
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct ReviewerFakeImpl;

#[async_trait]
impl AssistantReviewerProtocolInterface for ReviewerFakeImpl {
    async fn record_assistant_reviewer_candidate_fetch(
        &self,
        _context: &AssistantToolExecutionContext,
        _invocation_id: AssistantModelInvocationId,
        _tool_call_id: &str,
        _change_id: assistant::domain::AssistantWorkflowChangeId,
    ) -> Result<(), AssistantApplicationError> {
        Ok(())
    }

    async fn accept_assistant_reviewer_verdict(
        &self,
        _context: &AssistantToolExecutionContext,
        _invocation_id: AssistantModelInvocationId,
        _change_id: assistant::domain::AssistantWorkflowChangeId,
        _mutation_digest_hex: &str,
        _verdict: &str,
        _continuation: Option<assistant::interfaces::AssistantModelContinuationEnvelope>,
    ) -> Result<(), AssistantApplicationError> {
        Ok(())
    }
}

#[tokio::test]
async fn runner_launches_one_process_executes_serial_tool_call_and_shuts_down() {
    let (launcher, state) = launcher([
        incoming(1, "InvocationAccepted", json!({"agent_id": "workflow_coauthor@1"})),
        incoming(
            2,
            "ToolCall",
            json!({
                "call_id": "call-1",
                "tool_id": "assistant.workspace.get_snapshot@1",
                "arguments": {}
            }),
        ),
        incoming(3, "InvocationCompleted", json!({"final_text": "done"})),
    ]);
    let runner = runner(launcher);

    let result = runner.start_assistant_model_turn(request()).await.unwrap();

    assert_eq!(result.as_bytes(), b"done");
    let state = state.lock().unwrap();
    assert_eq!(state.writes.len(), 2);
    assert!(String::from_utf8_lossy(&state.writes[0]).contains("\"InvocationStart\""));
    assert!(String::from_utf8_lossy(&state.writes[1]).contains("\"ToolResult\""));
    assert!(state.shutdown);
    assert!(!state.aborted);
}

#[tokio::test]
async fn runner_aborts_process_after_protocol_failure() {
    let (launcher, state) = launcher([b"{invalid}\n".to_vec()]);
    let runner = runner(launcher);

    assert_eq!(
        runner.start_assistant_model_turn(request()).await,
        Err(AssistantApplicationError::ProtocolViolation)
    );
    let state = state.lock().unwrap();
    assert!(state.aborted);
    assert!(!state.shutdown);
}

#[tokio::test]
async fn runner_maps_route_mismatch_to_continuation_incompatible() {
    let (launcher, _state) = launcher([
        incoming(1, "InvocationAccepted", json!({"agent_id": "workflow_coauthor@1"})),
        incoming(
            2,
            "InvocationFailed",
            json!({
                "category": "ContinuationIncompatible",
                "safe_message": "Assistant continuation is incompatible",
            }),
        ),
    ]);

    assert_eq!(
        runner(launcher).start_assistant_model_turn(request()).await,
        Err(AssistantApplicationError::ContinuationIncompatible)
    );
}

#[tokio::test]
async fn runner_aborts_process_when_strict_shutdown_rejects_trailing_output() {
    let (launcher, state) = launcher([
        incoming(1, "InvocationAccepted", json!({"agent_id": "workflow_coauthor@1"})),
        incoming(2, "InvocationCompleted", json!({"final_text": "done"})),
    ]);
    state.lock().unwrap().shutdown_error = true;
    let runner = runner(launcher);

    assert_eq!(
        runner.start_assistant_model_turn(request()).await,
        Err(AssistantApplicationError::ProtocolViolation)
    );
    let state = state.lock().unwrap();
    assert!(state.shutdown);
    assert!(state.aborted);
}

fn launcher<const N: usize>(reads: [Vec<u8>; N]) -> (LauncherFakeImpl, Arc<Mutex<ProcessState>>) {
    let state = Arc::new(Mutex::new(ProcessState {
        reads: reads.into(),
        writes: Vec::new(),
        shutdown: false,
        shutdown_error: false,
        aborted: false,
    }));
    (LauncherFakeImpl { state: Arc::clone(&state) }, state)
}

fn incoming(sequence: u64, kind: &str, payload: Value) -> Vec<u8> {
    format!(
        "{}\n",
        json!({
            "protocol_version": 1,
            "invocation_id": uuid(3),
            "direction_sequence": sequence,
            "kind": kind,
            "payload": payload,
        })
    )
    .into_bytes()
}

fn request() -> AssistantModelTurnRequest {
    let project_id = ProjectId::from_uuid(uuid(1)).unwrap();
    let session_id = AssistantSessionId::from_uuid(uuid(2)).unwrap();
    AssistantModelTurnRequest {
        project_id,
        session_id,
        invocation_id: AssistantModelInvocationId::from_uuid(uuid(3)).unwrap(),
        start: AssistantModelTurnStart::UserMessage(
            AssistantUserIntent::new("Create a scene").unwrap(),
        ),
        workspace_request: AssistantWorkspaceSnapshotRequest::try_new(
            project_id,
            session_id,
            None,
            Vec::new(),
            Vec::new(),
        )
        .unwrap(),
        workspace_snapshot: AssistantWorkspaceSnapshot::new(b"{}".to_vec()).unwrap(),
    }
}

fn runner(
    launcher: LauncherFakeImpl,
) -> PythonAgentsAssistantModelRunnerAdapterImpl<
    LauncherFakeImpl,
    ToolExecutorFakeImpl,
    crate::assistant_tool_runtime::DesktopAssistantToolExecutionContextFactoryAdapterImpl,
    PresentationFakeImpl,
    ReviewerFakeImpl,
> {
    PythonAgentsAssistantModelRunnerAdapterImpl::new(
        launcher,
        ToolExecutorFakeImpl,
        crate::assistant_tool_runtime::DesktopAssistantToolExecutionContextFactoryAdapterImpl::new(
            Arc::new(ClockFakeImpl),
            60_000,
        ),
        PresentationFakeImpl,
        ReviewerFakeImpl,
    )
}

fn uuid(seed: u8) -> Uuid {
    Uuid::from_bytes([seed, 0, 0, 0, 0, 0, 0x40, 0, 0x80, 0, 0, 0, 0, 0, 0, seed])
}

#[path = "tests/reviewer.rs"]
mod reviewer;
