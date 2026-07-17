use assistant::{
    application::{AssistantToolCatalog, AssistantToolEffect},
    interfaces::{
        AssistantApplicationError, AssistantModelResumeRequest, AssistantModelTurnRequest,
        AssistantModelTurnStart,
    },
    protocol_v1::{
        ASSISTANT_PROTOCOL_VERSION, AssistantProtocolConversationValidator,
        AssistantProtocolDirection, AssistantProtocolFrame, AssistantProtocolPayload,
        ContinuationEnvelopePayload, ContinuationResumePayload, InvocationBudgets,
        InvocationStartKind, InvocationStartPayload, InvocationToolContract,
        InvocationTrustedContext, encode_assistant_protocol_frame,
    },
};
use serde_json::Value;

use super::AssistantProtocolProcessInterface;

pub(super) async fn write_frame(
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

pub(super) fn start_frame(
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

pub(super) fn resume_frame(
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

pub(super) fn map_protocol_error(
    error: assistant::protocol_v1::AssistantProtocolError,
) -> AssistantApplicationError {
    match error {
        assistant::protocol_v1::AssistantProtocolError::BudgetExceeded => {
            AssistantApplicationError::BudgetExceeded
        }
        _ => AssistantApplicationError::ProtocolViolation,
    }
}
