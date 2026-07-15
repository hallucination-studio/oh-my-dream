use serde::Serialize;

use crate::assistant_operations::{
    ApprovedEffect, OperationDispatchError, OperationEffect, RequestContext,
};
use crate::assistant_transport::{AssistantFrame, AssistantFrameKind};

use super::AssistantRuntime;
use super::error::AssistantRuntimeError;
use super::payload::{ToolRequestPayload, ToolResponsePayload};
use super::process::AssistantProcess;
use super::runner::RunMode;
use super::types::{AssistantInvocation, OperationCallEvidence, TrustedInvocationContext};

pub(super) async fn dispatch_tool(
    runtime: &AssistantRuntime,
    process: &mut dyn AssistantProcess,
    invocation: &AssistantInvocation,
    trusted: &TrustedInvocationContext,
    request: ToolRequestPayload,
    mode: &mut RunMode,
    outgoing: &mut OutgoingSequence,
) -> Result<OperationCallEvidence, AssistantRuntimeError> {
    let registration = runtime.registration(&request.operation_id)?;
    let approved_effect =
        approval_for_request(mode, registration.effect(), registration.version(), &request)?;
    let input = serde_json::from_str(&request.arguments_json).map_err(|source| {
        AssistantRuntimeError::InvalidArguments {
            operation_id: request.operation_id.clone(),
            source,
        }
    })?;
    let context = RequestContext::new(
        &trusted.project_id,
        &invocation.session_id,
        &trusted.request_id,
        registration.version(),
        approved_effect,
    )
    .with_workspace_selection(trusted.selected_node_ids.clone(), trusted.selected_asset_ids.clone())
    .with_user_request(invocation.input().map(str::to_owned));
    let output_json = match registration.dispatch(&context, input).await {
        Ok(output) => serde_json::to_string(&output).map_err(|source| {
            AssistantRuntimeError::OutputSerialization {
                operation_id: request.operation_id.clone(),
                source,
            }
        })?,
        Err(error) => recoverable_tool_error(error)?,
    };
    let response = ToolResponsePayload {
        invocation_id: &invocation.invocation_id,
        call_id: &request.call_id,
        output_json: &output_json,
    };
    outgoing.write(process, AssistantFrameKind::ToolResponse, &response).await?;
    Ok(OperationCallEvidence {
        operation_id: request.operation_id,
        operation_version: registration.version(),
        call_id: request.call_id,
        arguments_json: request.arguments_json,
        output_json,
    })
}

fn recoverable_tool_error(error: OperationDispatchError) -> Result<String, AssistantRuntimeError> {
    let (code, message, details) = match error {
        OperationDispatchError::SchemaValidation { violations, .. } => (
            "TOOL_SCHEMA_VALIDATION".to_owned(),
            "tool input failed schema validation".to_owned(),
            Some(serde_json::json!(
                violations
                    .iter()
                    .map(|item| serde_json::json!({
                        "instance_path": item.instance_path,
                        "schema_path": item.schema_path,
                        "message": item.message,
                    }))
                    .collect::<Vec<_>>()
            )),
        ),
        OperationDispatchError::InvalidInput { message, .. } => {
            ("TOOL_INVALID_INPUT".to_owned(), message, None)
        }
        OperationDispatchError::Handler { source, .. } => {
            (source.code().to_owned(), source.message().to_owned(), None)
        }
        OperationDispatchError::ApprovalRequired { .. } => (
            "TOOL_APPROVAL_REQUIRED".to_owned(),
            "tool requires a valid reviewed approval receipt".to_owned(),
            None,
        ),
        internal => return Err(AssistantRuntimeError::Operation(internal)),
    };
    serde_json::to_string(&serde_json::json!({
        "ok": false,
        "error": { "code": code, "message": message, "details": details },
    }))
    .map_err(|source| AssistantRuntimeError::OutputSerialization {
        operation_id: "tool_error".to_owned(),
        source,
    })
}

fn approval_for_request(
    mode: &mut RunMode,
    effect: OperationEffect,
    operation_version: u32,
    request: &ToolRequestPayload,
) -> Result<Option<ApprovedEffect>, AssistantRuntimeError> {
    if effect != OperationEffect::PreparedApprovalExecution {
        return Ok(None);
    }
    let RunMode::Resume { waiting, approved, consumed } = mode else {
        return Ok(None);
    };
    if !*approved {
        return Err(AssistantRuntimeError::RejectedApprovalExecution);
    }
    if *consumed {
        return Err(AssistantRuntimeError::ApprovalReuse);
    }
    let pending = waiting.pending();
    if pending.call_id() != request.call_id
        || pending.operation_id() != request.operation_id
        || pending.operation_version() != operation_version
        || pending.arguments_json() != request.arguments_json
    {
        return Err(AssistantRuntimeError::ApprovalMismatch);
    }
    *consumed = true;
    Ok(Some(ApprovedEffect::new(&request.operation_id, operation_version, &request.call_id)))
}

#[derive(Default)]
pub(super) struct OutgoingSequence(u64);

impl OutgoingSequence {
    pub(super) async fn write(
        &mut self,
        process: &mut dyn AssistantProcess,
        kind: AssistantFrameKind,
        payload: &impl Serialize,
    ) -> Result<(), AssistantRuntimeError> {
        let payload = serde_json::to_value(payload).map_err(|error| {
            AssistantRuntimeError::InvalidPayload { kind, message: error.to_string() }
        })?;
        let frame = AssistantFrame::new(self.0, kind, payload)?;
        process.write_frame(&frame).await?;
        self.0 = self.0.checked_add(1).ok_or(AssistantRuntimeError::UnexpectedFrame { kind })?;
        Ok(())
    }
}
