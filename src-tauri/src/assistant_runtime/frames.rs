use serde_json::Value;

use crate::assistant_transport::{AssistantFrame, AssistantFrameKind};

use super::AssistantRuntime;
use super::InternalReviewSubmission;
use super::dispatch::{OutgoingSequence, dispatch_tool};
use super::error::AssistantRuntimeError;
use super::payload::{
    ApprovalRequestPayload, CompletedPayload, ErrorPayload, ResponsesEventPayload,
    ReviewCheckPayload, ReviewCheckResponsePayload, ReviewResponsePayload, SnapshotPayload,
    ToolRequestPayload,
};
use super::process::AssistantProcess;
use super::runner::RunMode;
use super::types::{
    AssistantCompleted, AssistantEventSink, AssistantInvocation, AssistantPendingApproval,
    AssistantRuntimeOutcome, AssistantSessionSnapshot, AssistantWaitingApproval,
    OperationCallEvidence, TrustedInvocationContext,
};

pub(super) struct FrameHandler<'a> {
    runtime: &'a AssistantRuntime,
    process: &'a mut dyn AssistantProcess,
    invocation: &'a AssistantInvocation,
    trusted: &'a TrustedInvocationContext,
    mode: &'a mut RunMode,
    outgoing: &'a mut OutgoingSequence,
    sink: &'a mut dyn AssistantEventSink,
    state: InvocationState,
}

impl<'a> FrameHandler<'a> {
    pub(super) fn new(
        runtime: &'a AssistantRuntime,
        process: &'a mut dyn AssistantProcess,
        invocation: &'a AssistantInvocation,
        trusted: &'a TrustedInvocationContext,
        mode: &'a mut RunMode,
        outgoing: &'a mut OutgoingSequence,
        sink: &'a mut dyn AssistantEventSink,
    ) -> Self {
        Self {
            runtime,
            process,
            invocation,
            trusted,
            mode,
            outgoing,
            sink,
            state: InvocationState::default(),
        }
    }

    pub(super) async fn next(
        &mut self,
    ) -> Result<Option<AssistantRuntimeOutcome>, AssistantRuntimeError> {
        let frame = self.process.read_frame().await?;
        self.state.observe_frame(self.runtime, &frame)?;
        self.handle(frame).await
    }

    async fn handle(
        &mut self,
        frame: AssistantFrame,
    ) -> Result<Option<AssistantRuntimeOutcome>, AssistantRuntimeError> {
        validate_frame_order(&self.state, frame.kind())?;
        match frame.kind() {
            AssistantFrameKind::ResponsesEvent => self.handle_responses_event(&frame),
            AssistantFrameKind::ToolRequest => self.handle_tool_request(&frame).await,
            AssistantFrameKind::ReviewSubmit => self.handle_review_submit(&frame).await,
            AssistantFrameKind::ReviewCheck => self.handle_review_check(&frame).await,
            AssistantFrameKind::ApprovalRequest => self.handle_approval(&frame),
            AssistantFrameKind::Snapshot => {
                handle_snapshot(self.invocation, self.trusted, &mut self.state, &frame)
            }
            AssistantFrameKind::Completed => self.handle_completed(&frame),
            AssistantFrameKind::Error => Err(sidecar_error(self.invocation, &frame)?),
            kind => Err(AssistantRuntimeError::UnexpectedFrame { kind }),
        }
    }

    async fn handle_review_submit(
        &mut self,
        frame: &AssistantFrame,
    ) -> Result<Option<AssistantRuntimeOutcome>, AssistantRuntimeError> {
        let submission: InternalReviewSubmission = decode_payload(frame)?;
        check_invocation(self.invocation, &submission.invocation_id)?;
        let handler = self.runtime.review_handler.as_ref().ok_or_else(|| {
            AssistantRuntimeError::InternalReview {
                message: "review handler is unavailable".to_owned(),
            }
        })?;
        let receipt = handler
            .record(&self.trusted.project_id, &self.invocation.session_id, submission)
            .map_err(|message| AssistantRuntimeError::InternalReview { message })?;
        let response = ReviewResponsePayload {
            invocation_id: &self.invocation.invocation_id,
            candidate_id: &receipt.candidate_id,
            review_receipt_id: &receipt.review_receipt_id,
        };
        self.outgoing
            .write(&mut *self.process, AssistantFrameKind::ReviewResponse, &response)
            .await?;
        Ok(None)
    }

    async fn handle_review_check(
        &mut self,
        frame: &AssistantFrame,
    ) -> Result<Option<AssistantRuntimeOutcome>, AssistantRuntimeError> {
        let payload: ReviewCheckPayload = decode_payload(frame)?;
        check_invocation(self.invocation, &payload.invocation_id)?;
        let handler = self.runtime.review_handler.as_ref().ok_or_else(|| {
            AssistantRuntimeError::InternalReview {
                message: "review handler is unavailable".to_owned(),
            }
        })?;
        let valid = handler
            .valid_for_approval(
                &self.trusted.project_id,
                &self.invocation.session_id,
                &payload.operation_id,
                &payload.arguments_json,
            )
            .map_err(|message| AssistantRuntimeError::InternalReview { message })?;
        let response = ReviewCheckResponsePayload {
            invocation_id: &self.invocation.invocation_id,
            operation_id: &payload.operation_id,
            valid,
        };
        self.outgoing
            .write(&mut *self.process, AssistantFrameKind::ReviewCheckResponse, &response)
            .await?;
        Ok(None)
    }

    async fn handle_tool_request(
        &mut self,
        frame: &AssistantFrame,
    ) -> Result<Option<AssistantRuntimeOutcome>, AssistantRuntimeError> {
        let payload: ToolRequestPayload = decode_payload(frame)?;
        check_invocation(self.invocation, &payload.invocation_id)?;
        let evidence = match dispatch_tool(
            self.runtime,
            &mut *self.process,
            self.invocation,
            self.trusted,
            payload,
            &mut *self.mode,
            &mut *self.outgoing,
        )
        .await
        {
            Ok(evidence) => evidence,
            Err(error) => return Err(error),
        };
        self.state.add_bytes(self.runtime, evidence.output_json.len())?;
        self.state.operation_calls.push(evidence);
        Ok(None)
    }

    fn handle_responses_event(
        &mut self,
        frame: &AssistantFrame,
    ) -> Result<Option<AssistantRuntimeOutcome>, AssistantRuntimeError> {
        let payload: ResponsesEventPayload = decode_payload(frame)?;
        check_invocation(self.invocation, &payload.invocation_id)?;
        if !payload.event.is_object() || payload.event.get("type").and_then(Value::as_str).is_none()
        {
            return Err(AssistantRuntimeError::InvalidPayload {
                kind: frame.kind(),
                message: "Responses event must be an object with a string type".to_owned(),
            });
        }
        self.sink.emit(payload.event)?;
        Ok(None)
    }

    fn handle_approval(
        &mut self,
        frame: &AssistantFrame,
    ) -> Result<Option<AssistantRuntimeOutcome>, AssistantRuntimeError> {
        let result = handle_approval(self.runtime, self.invocation, &mut self.state, frame)?;
        Ok(result)
    }

    fn handle_completed(
        &mut self,
        frame: &AssistantFrame,
    ) -> Result<Option<AssistantRuntimeOutcome>, AssistantRuntimeError> {
        let result = handle_completed(self.invocation, &mut self.state, frame)?;
        Ok(result)
    }
}

#[derive(Default)]
struct InvocationState {
    incoming_frames: usize,
    invocation_bytes: usize,
    snapshot: Option<AssistantSessionSnapshot>,
    pending: Option<(AssistantPendingApproval, Value)>,
    operation_calls: Vec<OperationCallEvidence>,
}

impl InvocationState {
    fn observe_frame(
        &mut self,
        runtime: &AssistantRuntime,
        frame: &AssistantFrame,
    ) -> Result<(), AssistantRuntimeError> {
        self.incoming_frames += 1;
        if self.incoming_frames > runtime.limits.max_incoming_frames {
            return Err(AssistantRuntimeError::ResourceLimit {
                resource: "incoming frames",
                maximum: runtime.limits.max_incoming_frames,
            });
        }
        let bytes = serde_json::to_vec(frame.payload())
            .map_err(|error| AssistantRuntimeError::InvalidPayload {
                kind: frame.kind(),
                message: error.to_string(),
            })?
            .len();
        self.add_bytes(runtime, bytes)
    }

    fn add_bytes(
        &mut self,
        runtime: &AssistantRuntime,
        bytes: usize,
    ) -> Result<(), AssistantRuntimeError> {
        self.invocation_bytes = self.invocation_bytes.checked_add(bytes).ok_or(
            AssistantRuntimeError::ResourceLimit {
                resource: "invocation bytes",
                maximum: runtime.limits.max_collected_bytes,
            },
        )?;
        if self.invocation_bytes > runtime.limits.max_collected_bytes {
            return Err(AssistantRuntimeError::ResourceLimit {
                resource: "invocation bytes",
                maximum: runtime.limits.max_collected_bytes,
            });
        }
        Ok(())
    }
}

fn validate_frame_order(
    state: &InvocationState,
    kind: AssistantFrameKind,
) -> Result<(), AssistantRuntimeError> {
    if state.snapshot.is_some() && kind != AssistantFrameKind::Completed {
        return Err(AssistantRuntimeError::InvalidStateTransition {
            event: "only completed may follow a completed snapshot",
        });
    }
    if state.pending.is_some() && kind != AssistantFrameKind::Snapshot {
        return Err(AssistantRuntimeError::InvalidStateTransition {
            event: "only snapshot may follow an approval request",
        });
    }
    Ok(())
}

fn handle_approval(
    runtime: &AssistantRuntime,
    invocation: &AssistantInvocation,
    state: &mut InvocationState,
    frame: &AssistantFrame,
) -> Result<Option<AssistantRuntimeOutcome>, AssistantRuntimeError> {
    let payload: ApprovalRequestPayload = decode_payload(frame)?;
    check_invocation(invocation, &payload.invocation_id)?;
    let registration = runtime.registration(&payload.operation_id)?;
    state.pending = Some((
        AssistantPendingApproval {
            call_id: payload.call_id,
            operation_id: payload.operation_id,
            operation_version: registration.version(),
            arguments_json: payload.arguments_json,
        },
        payload.state,
    ));
    Ok(None)
}

fn handle_snapshot(
    invocation: &AssistantInvocation,
    trusted: &TrustedInvocationContext,
    state: &mut InvocationState,
    frame: &AssistantFrame,
) -> Result<Option<AssistantRuntimeOutcome>, AssistantRuntimeError> {
    let payload: SnapshotPayload = decode_payload(frame)?;
    check_invocation(invocation, &payload.invocation_id)?;
    if payload.session_id != invocation.session_id {
        return Err(AssistantRuntimeError::SessionMismatch);
    }
    if payload.status == "waiting_approval" {
        return waiting_outcome(invocation, trusted, state.pending.take(), payload.state).map(Some);
    }
    if state.pending.is_some() {
        return Err(AssistantRuntimeError::InvalidStateTransition {
            event: "completed snapshot cannot follow an approval request",
        });
    }
    if payload.status != "completed" {
        return Err(AssistantRuntimeError::InvalidSnapshotStatus { status: payload.status });
    }
    state.snapshot = Some(AssistantSessionSnapshot {
        session_id: payload.session_id,
        status: payload.status,
        state: payload.state,
    });
    Ok(None)
}

fn handle_completed(
    invocation: &AssistantInvocation,
    state: &mut InvocationState,
    frame: &AssistantFrame,
) -> Result<Option<AssistantRuntimeOutcome>, AssistantRuntimeError> {
    let payload: CompletedPayload = decode_payload(frame)?;
    check_invocation(invocation, &payload.invocation_id)?;
    let completed = AssistantCompleted {
        final_output: payload.final_output,
        snapshot: state.snapshot.take().ok_or(AssistantRuntimeError::MissingSnapshot)?,
        operation_calls: std::mem::take(&mut state.operation_calls),
    };
    Ok(Some(AssistantRuntimeOutcome::Completed(completed)))
}

fn sidecar_error(
    invocation: &AssistantInvocation,
    frame: &AssistantFrame,
) -> Result<AssistantRuntimeError, AssistantRuntimeError> {
    let payload: ErrorPayload = decode_payload(frame)?;
    check_invocation(invocation, &payload.invocation_id)?;
    Ok(AssistantRuntimeError::SidecarReported { code: payload.code, message: payload.message })
}

fn waiting_outcome(
    invocation: &AssistantInvocation,
    trusted: &TrustedInvocationContext,
    pending: Option<(AssistantPendingApproval, Value)>,
    snapshot_state: Value,
) -> Result<AssistantRuntimeOutcome, AssistantRuntimeError> {
    let (pending, request_state) = pending.ok_or(AssistantRuntimeError::MissingApprovalSnapshot)?;
    if request_state != snapshot_state {
        return Err(AssistantRuntimeError::ApprovalStateMismatch);
    }
    Ok(AssistantRuntimeOutcome::WaitingApproval(AssistantWaitingApproval {
        state: snapshot_state,
        pending,
        project_id: trusted.project_id.clone(),
        session_id: invocation.session_id.clone(),
        session_path: invocation.session_path.clone(),
    }))
}

fn decode_payload<T: serde::de::DeserializeOwned>(
    frame: &AssistantFrame,
) -> Result<T, AssistantRuntimeError> {
    serde_json::from_value(frame.payload().clone()).map_err(|error| {
        AssistantRuntimeError::InvalidPayload { kind: frame.kind(), message: error.to_string() }
    })
}

fn check_invocation(
    invocation: &AssistantInvocation,
    actual: &str,
) -> Result<(), AssistantRuntimeError> {
    if actual == invocation.invocation_id {
        return Ok(());
    }
    Err(AssistantRuntimeError::InvocationMismatch {
        expected: invocation.invocation_id.clone(),
        actual: actual.to_owned(),
    })
}
