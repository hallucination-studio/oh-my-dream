use serde_json::Value;
use tokio::time::{Instant, timeout, timeout_at};

use crate::assistant_transport::AssistantFrameKind;

use super::AssistantRuntime;
use super::dispatch::OutgoingSequence;
use super::error::AssistantRuntimeError;
use super::frames::FrameHandler;
use super::payload::{ApprovalResponsePayload, InvokePayload};
use super::process::AssistantProcess;
use super::types::{
    AssistantInvocation, AssistantRuntimeOutcome, AssistantWaitingApproval,
    TrustedInvocationContext,
};

pub(super) async fn invoke(
    runtime: &AssistantRuntime,
    invocation: AssistantInvocation,
    trusted: TrustedInvocationContext,
) -> Result<AssistantRuntimeOutcome, AssistantRuntimeError> {
    run_process(runtime, invocation, trusted, RunMode::New).await
}

pub(super) async fn resume(
    runtime: &AssistantRuntime,
    invocation: AssistantInvocation,
    trusted: TrustedInvocationContext,
    waiting: AssistantWaitingApproval,
    approved: bool,
) -> Result<AssistantRuntimeOutcome, AssistantRuntimeError> {
    if invocation.input.is_some() {
        return Err(AssistantRuntimeError::ResumeInputMustBeNull);
    }
    if trusted.project_id != waiting.project_id
        || invocation.session_id != waiting.session_id
        || invocation.session_path != waiting.session_path
    {
        return Err(AssistantRuntimeError::ApprovalScopeMismatch);
    }
    let registration = runtime.registration(waiting.pending.operation_id())?;
    if registration.version() != waiting.pending.operation_version() {
        return Err(AssistantRuntimeError::ApprovalMismatch);
    }
    run_process(
        runtime,
        invocation,
        trusted,
        RunMode::Resume { waiting, approved, consumed: false },
    )
    .await
}

pub(super) enum RunMode {
    New,
    Resume { waiting: AssistantWaitingApproval, approved: bool, consumed: bool },
}

async fn run_process(
    runtime: &AssistantRuntime,
    invocation: AssistantInvocation,
    trusted: TrustedInvocationContext,
    mode: RunMode,
) -> Result<AssistantRuntimeOutcome, AssistantRuntimeError> {
    let invocation_deadline = Instant::now() + runtime.limits.invocation_timeout;
    let mut process = timeout_at(invocation_deadline, runtime.launcher.launch())
        .await
        .map_err(|_| AssistantRuntimeError::InvocationTimeout)??;
    let result = timeout_at(
        invocation_deadline,
        run_loop(runtime, process.as_mut(), &invocation, &trusted, mode),
    )
    .await;
    match result {
        Ok(Ok(outcome)) => {
            match timeout(runtime.limits.shutdown_timeout, process.shutdown()).await {
                Ok(result) => match result {
                    Ok(()) => Ok(outcome),
                    Err(error) => {
                        abort_bounded(process.as_mut(), runtime.limits.shutdown_timeout).await;
                        Err(error)
                    }
                },
                Err(_) => {
                    abort_bounded(process.as_mut(), runtime.limits.shutdown_timeout).await;
                    Err(AssistantRuntimeError::ShutdownTimeout)
                }
            }
        }
        Ok(Err(error)) => {
            abort_bounded(process.as_mut(), runtime.limits.shutdown_timeout).await;
            Err(error)
        }
        Err(_) => {
            abort_bounded(process.as_mut(), runtime.limits.shutdown_timeout).await;
            Err(AssistantRuntimeError::InvocationTimeout)
        }
    }
}

async fn abort_bounded(process: &mut dyn AssistantProcess, deadline: std::time::Duration) {
    let _ = timeout(deadline, process.abort()).await;
}

async fn run_loop(
    runtime: &AssistantRuntime,
    process: &mut dyn AssistantProcess,
    invocation: &AssistantInvocation,
    trusted: &TrustedInvocationContext,
    mut mode: RunMode,
) -> Result<AssistantRuntimeOutcome, AssistantRuntimeError> {
    let mut outgoing = OutgoingSequence::default();
    send_invoke(runtime, process, invocation, &mode, &mut outgoing).await?;
    send_approval_response(process, invocation, &mode, &mut outgoing).await?;
    let mut frames =
        FrameHandler::new(runtime, process, invocation, trusted, &mut mode, &mut outgoing);
    loop {
        if let Some(outcome) = frames.next().await? {
            return Ok(outcome);
        }
    }
}

async fn send_invoke(
    runtime: &AssistantRuntime,
    process: &mut dyn AssistantProcess,
    invocation: &AssistantInvocation,
    mode: &RunMode,
    outgoing: &mut OutgoingSequence,
) -> Result<(), AssistantRuntimeError> {
    let session_path =
        invocation.session_path.to_str().ok_or(AssistantRuntimeError::InvalidSessionPath)?;
    let state = match mode {
        RunMode::New => Value::Null,
        RunMode::Resume { waiting, .. } => waiting.state.clone(),
    };
    let payload = InvokePayload {
        invocation_id: &invocation.invocation_id,
        session_id: &invocation.session_id,
        session_path,
        input: invocation.input.as_deref(),
        operations: runtime
            .registrations
            .iter()
            .map(crate::assistant_operations::OperationRegistration::contract)
            .collect(),
        state,
    };
    outgoing.write(process, AssistantFrameKind::Invoke, &payload).await
}

async fn send_approval_response(
    process: &mut dyn AssistantProcess,
    invocation: &AssistantInvocation,
    mode: &RunMode,
    outgoing: &mut OutgoingSequence,
) -> Result<(), AssistantRuntimeError> {
    let RunMode::Resume { waiting, approved, .. } = mode else {
        return Ok(());
    };
    let payload = ApprovalResponsePayload {
        invocation_id: &invocation.invocation_id,
        call_id: waiting.pending.call_id(),
        approved: *approved,
    };
    outgoing.write(process, AssistantFrameKind::ApprovalResponse, &payload).await
}
