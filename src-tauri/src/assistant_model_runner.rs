mod interfaces;
mod process;

use std::time::Duration;

use assistant::{
    application::{AssistantToolCatalog, AssistantToolEffect},
    domain::AssistantSessionId,
    interfaces::{
        AssistantApplicationError, AssistantModelResumeRequest, AssistantModelRunnerInterface,
        AssistantModelTurnRequest, AssistantModelTurnResult, AssistantModelTurnStart,
    },
    protocol_v1::{
        ASSISTANT_PROTOCOL_VERSION, AssistantProtocolConversationValidator,
        AssistantProtocolDecoder, AssistantProtocolDirection, AssistantProtocolFrame,
        AssistantProtocolPayload, ContinuationEnvelopePayload, ContinuationResumePayload,
        InvocationBudgets, InvocationStartKind, InvocationStartPayload, InvocationToolContract,
        InvocationTrustedContext, ToolResultPayload, encode_assistant_protocol_frame,
    },
};
use async_trait::async_trait;
use projects::project::domain::ProjectId;
use serde_json::Value;
use tokio::time::timeout;

pub use interfaces::*;
pub use process::AssistantSidecarCommandProcessLauncherImpl;

const INVOCATION_DEADLINE: Duration = Duration::from_secs(10 * 60);

/// Isolated Python Agents SDK process adapter for one bounded model invocation.
pub struct PythonAgentsAssistantModelRunnerAdapterImpl<L, T> {
    launcher: L,
    tool_executor: T,
}

impl<L, T> PythonAgentsAssistantModelRunnerAdapterImpl<L, T> {
    #[must_use]
    pub const fn new(launcher: L, tool_executor: T) -> Self {
        Self { launcher, tool_executor }
    }
}

#[async_trait]
impl<L, T> AssistantModelRunnerInterface for PythonAgentsAssistantModelRunnerAdapterImpl<L, T>
where
    L: AssistantProtocolProcessLauncherInterface,
    T: AssistantProtocolToolExecutorInterface,
{
    async fn start_assistant_model_turn(
        &self,
        request: AssistantModelTurnRequest,
    ) -> Result<AssistantModelTurnResult, AssistantApplicationError> {
        let start = start_frame(&request)?;
        self.run(request.project_id, request.session_id, start).await
    }

    async fn resume_assistant_model_turn(
        &self,
        request: AssistantModelResumeRequest,
    ) -> Result<AssistantModelTurnResult, AssistantApplicationError> {
        let start = resume_frame(&request)?;
        self.run(request.project_id, request.session_id, start).await
    }
}

impl<L, T> PythonAgentsAssistantModelRunnerAdapterImpl<L, T>
where
    L: AssistantProtocolProcessLauncherInterface,
    T: AssistantProtocolToolExecutorInterface,
{
    async fn run(
        &self,
        project_id: ProjectId,
        session_id: AssistantSessionId,
        first_frame: AssistantProtocolFrame,
    ) -> Result<AssistantModelTurnResult, AssistantApplicationError> {
        let invocation_id = first_frame.invocation_id.clone();
        let mut process = self.launcher.launch_assistant_protocol_process().await?;
        let result = timeout(
            INVOCATION_DEADLINE,
            self.exchange(&mut *process, project_id, session_id, first_frame),
        )
        .await;
        let outcome = match result {
            Ok(Ok(value)) => {
                match timeout(INVOCATION_DEADLINE, process.shutdown_assistant_protocol_process())
                    .await
                {
                    Ok(Ok(())) => Ok(value),
                    Ok(Err(error)) => {
                        process.abort_assistant_protocol_process().await;
                        Err(error)
                    }
                    Err(_) => {
                        process.abort_assistant_protocol_process().await;
                        Err(AssistantApplicationError::DeadlineExceeded)
                    }
                }
            }
            Ok(Err(error)) => {
                process.abort_assistant_protocol_process().await;
                Err(error)
            }
            Err(_) => {
                process.abort_assistant_protocol_process().await;
                Err(AssistantApplicationError::DeadlineExceeded)
            }
        };
        outcome.inspect_err(|_| {
            tracing::warn!(invocation_id, "Assistant model invocation failed");
        })
    }

    async fn exchange(
        &self,
        process: &mut dyn AssistantProtocolProcessInterface,
        project_id: ProjectId,
        session_id: AssistantSessionId,
        first_frame: AssistantProtocolFrame,
    ) -> Result<AssistantModelTurnResult, AssistantApplicationError> {
        let mut rust_sequence = 1_u64;
        let mut decoder = AssistantProtocolDecoder::new(AssistantProtocolDirection::PythonToRust);
        let mut conversation = AssistantProtocolConversationValidator::default();
        write_frame(process, &mut conversation, &first_frame).await?;
        loop {
            let encoded = process.read_assistant_protocol_line().await?;
            let frame = decoder.decode(&encoded).map_err(map_protocol_error)?;
            conversation
                .admit(AssistantProtocolDirection::PythonToRust, encoded.len(), &frame)
                .map_err(map_protocol_error)?;
            match frame.payload {
                AssistantProtocolPayload::ToolCall(call) => {
                    rust_sequence += 1;
                    let result = self
                        .tool_executor
                        .execute_assistant_protocol_tool(
                            project_id,
                            session_id,
                            &call.tool_id,
                            call.arguments,
                        )
                        .await?;
                    let response = AssistantProtocolFrame {
                        protocol_version: ASSISTANT_PROTOCOL_VERSION,
                        invocation_id: frame.invocation_id,
                        direction_sequence: rust_sequence,
                        payload: AssistantProtocolPayload::ToolResult(ToolResultPayload {
                            call_id: call.call_id,
                            tool_id: call.tool_id,
                            result,
                        }),
                    };
                    write_frame(process, &mut conversation, &response).await?;
                }
                AssistantProtocolPayload::InvocationCompleted(completed) => {
                    return AssistantModelTurnResult::new(completed.final_text.into_bytes());
                }
                AssistantProtocolPayload::InvocationFailed(_) => {
                    return Err(AssistantApplicationError::ModelUnavailable);
                }
                _ => {}
            }
        }
    }
}

async fn write_frame(
    process: &mut dyn AssistantProtocolProcessInterface,
    conversation: &mut AssistantProtocolConversationValidator,
    frame: &AssistantProtocolFrame,
) -> Result<(), AssistantApplicationError> {
    let encoded = encode_assistant_protocol_frame(frame).map_err(map_protocol_error)?;
    conversation
        .admit(AssistantProtocolDirection::RustToPython, encoded.len(), frame)
        .map_err(map_protocol_error)?;
    process.write_assistant_protocol_line(&encoded).await
}

fn start_frame(
    request: &AssistantModelTurnRequest,
) -> Result<AssistantProtocolFrame, AssistantApplicationError> {
    let start = match &request.start {
        AssistantModelTurnStart::UserMessage(intent) => {
            InvocationStartKind::UserMessage { message: intent.as_str().to_owned() }
        }
        AssistantModelTurnStart::RepairActivation(activation) => {
            InvocationStartKind::RepairActivation {
                activation_id: activation.id().as_uuid().to_string(),
                failed_workflow_run_id: uuid::Uuid::from_bytes(
                    activation.failed_workflow_run_id().0,
                )
                .to_string(),
                exact_failed_run_facts: parse_json(activation.exact_failed_run_facts())?,
            }
        }
    };
    Ok(AssistantProtocolFrame {
        protocol_version: ASSISTANT_PROTOCOL_VERSION,
        invocation_id: request.invocation_id.as_uuid().to_string(),
        direction_sequence: 1,
        payload: AssistantProtocolPayload::InvocationStart(InvocationStartPayload {
            start,
            trusted_context: InvocationTrustedContext {
                project_id: request.project_id.as_uuid().to_string(),
                session_id: request.session_id.as_uuid().to_string(),
                workspace_snapshot: parse_json(request.workspace_snapshot.as_bytes())?,
            },
            tool_contracts: tool_contracts()?,
            budgets: InvocationBudgets {
                maximum_frame_bytes: 8 * 1024 * 1024,
                maximum_events: 512,
                maximum_tool_calls: 64,
                maximum_model_turns: 16,
                maximum_direction_bytes: 16 * 1024 * 1024,
                deadline_ms: 600_000,
            },
        }),
    })
}

fn resume_frame(
    request: &AssistantModelResumeRequest,
) -> Result<AssistantProtocolFrame, AssistantApplicationError> {
    let envelope: ContinuationEnvelopePayload =
        serde_json::from_slice(request.continuation.as_bytes())
            .map_err(|_| AssistantApplicationError::ContinuationIncompatible)?;
    Ok(AssistantProtocolFrame {
        protocol_version: ASSISTANT_PROTOCOL_VERSION,
        invocation_id: request.invocation_id.as_uuid().to_string(),
        direction_sequence: 1,
        payload: AssistantProtocolPayload::ContinuationResume(ContinuationResumePayload {
            envelope,
            trusted_result: parse_json(request.input.as_bytes())?,
        }),
    })
}

fn tool_contracts() -> Result<Vec<InvocationToolContract>, AssistantApplicationError> {
    AssistantToolCatalog::try_new()?
        .contracts()
        .iter()
        .map(|contract| {
            Ok(InvocationToolContract {
                tool_id: contract.id().as_str().to_owned(),
                description: contract.description().to_owned(),
                input_schema: contract.input_schema().clone(),
                output_schema: contract.output_schema().clone(),
                effect: match contract.effect() {
                    AssistantToolEffect::AuthoritativeRead => "AuthoritativeRead",
                    AssistantToolEffect::AssistantStateMutation => "AssistantStateMutation",
                    AssistantToolEffect::HumanApprovalRequest => "HumanApprovalRequest",
                }
                .to_owned(),
                requires_human_approval: contract.requires_human_approval(),
            })
        })
        .collect()
}

fn parse_json(bytes: &[u8]) -> Result<Value, AssistantApplicationError> {
    serde_json::from_slice(bytes).map_err(|_| AssistantApplicationError::ProtocolViolation)
}

fn map_protocol_error(
    error: assistant::protocol_v1::AssistantProtocolError,
) -> AssistantApplicationError {
    match error {
        assistant::protocol_v1::AssistantProtocolError::BudgetExceeded => {
            AssistantApplicationError::BudgetExceeded
        }
        _ => AssistantApplicationError::ProtocolViolation,
    }
}

#[cfg(test)]
mod tests;
