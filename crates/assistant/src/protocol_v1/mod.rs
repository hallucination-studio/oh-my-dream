//! Strict Assistant protocol version 1 frame shapes and inbound validation.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;

mod codec;
mod json;
mod state;
pub use codec::encode_assistant_protocol_frame;
use json::{decode_strict_json, json_depth};
pub use state::*;

pub const ASSISTANT_PROTOCOL_VERSION: u32 = 1;
pub const MAX_ASSISTANT_PROTOCOL_FRAME_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_ASSISTANT_PROTOCOL_JSON_DEPTH: usize = 32;
pub const MAX_ASSISTANT_PROTOCOL_INBOUND_EVENTS: usize = 512;
pub const MAX_ASSISTANT_PROTOCOL_TOOL_CALLS: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssistantProtocolDirection {
    RustToPython,
    PythonToRust,
}

#[derive(Clone, Copy, Debug, thiserror::Error, PartialEq, Eq)]
pub enum AssistantProtocolError {
    #[error("Assistant protocol frame is too large")]
    FrameTooLarge,
    #[error("Assistant protocol frame is partial")]
    PartialFrame,
    #[error("Assistant protocol JSON is invalid")]
    InvalidJson,
    #[error("Assistant protocol JSON is too deep")]
    JsonTooDeep,
    #[error("Assistant protocol frame is invalid")]
    InvalidFrame,
    #[error("Assistant protocol frame has the wrong direction")]
    WrongDirection,
    #[error("Assistant protocol sequence is invalid")]
    InvalidSequence,
    #[error("Assistant protocol call ID is duplicated")]
    DuplicateCallId,
    #[error("Assistant protocol budget is exceeded")]
    BudgetExceeded,
    #[error("Assistant protocol frame follows a terminal frame")]
    FrameAfterTerminal,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AssistantProtocolFrame {
    pub protocol_version: u32,
    pub invocation_id: String,
    pub direction_sequence: u64,
    pub payload: AssistantProtocolPayload,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AssistantProtocolPayload {
    InvocationStart(InvocationStartPayload),
    ToolResult(ToolResultPayload),
    ContinuationResume(ContinuationResumePayload),
    InvocationCancel(InvocationCancelPayload),
    InvocationAccepted(InvocationAcceptedPayload),
    ModelOutputDelta(ModelOutputDeltaPayload),
    ToolCall(ToolCallPayload),
    ReviewerVerdict(ReviewerVerdictPayload),
    ContinuationEnvelopeReady(ContinuationEnvelopeReadyPayload),
    InvocationCompleted(InvocationCompletedPayload),
    InvocationFailed(InvocationFailedPayload),
}

macro_rules! strict_payload {
    ($name:ident { $($field:ident : $ty:ty),* $(,)? }) => {
        #[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
        #[serde(deny_unknown_fields)]
        pub struct $name { $(pub $field: $ty),* }
    };
}

strict_payload!(InvocationStartPayload {
    start: InvocationStartKind,
    trusted_context: InvocationTrustedContext,
    tool_contracts: Vec<InvocationToolContract>,
    budgets: InvocationBudgets,
});
strict_payload!(ToolResultPayload { call_id: String, tool_id: String, result: Value });
strict_payload!(ContinuationResumePayload {
    envelope: ContinuationEnvelopePayload,
    trusted_result: Value,
});
strict_payload!(InvocationCancelPayload { reason: InvocationCancelReason });
strict_payload!(InvocationAcceptedPayload { agent_id: String });
strict_payload!(ModelOutputDeltaPayload { text: String });
strict_payload!(ToolCallPayload { call_id: String, tool_id: String, arguments: Value });
strict_payload!(ReviewerVerdictPayload {
    change_id: String,
    mutation_digest: String,
    verdict: String,
    prose: String,
});
strict_payload!(ContinuationEnvelopeReadyPayload { envelope: ContinuationEnvelopePayload });
strict_payload!(InvocationCompletedPayload { final_text: String });
strict_payload!(InvocationFailedPayload { category: String, safe_message: String });

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub enum InvocationCancelReason {
    Deadline,
    ProcessShutdown,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum InvocationStartKind {
    UserMessage {
        message: String,
    },
    RepairActivation {
        activation_id: String,
        failed_workflow_run_id: String,
        exact_failed_run_facts: Value,
    },
}

strict_payload!(InvocationTrustedContext {
    project_id: String,
    session_id: String,
    workspace_snapshot: Value,
});
strict_payload!(InvocationToolContract {
    tool_id: String,
    description: String,
    input_schema: Value,
    output_schema: Value,
    effect: String,
    requires_human_approval: bool,
});
strict_payload!(InvocationBudgets {
    maximum_frame_bytes: usize,
    maximum_events: usize,
    maximum_tool_calls: usize,
    maximum_model_turns: usize,
    maximum_direction_bytes: usize,
    deadline_ms: u64,
});
strict_payload!(ContinuationEnvelopePayload {
    protocol_version: u32,
    contract_epoch: u32,
    sdk_version: String,
    agent_id: String,
    tool_ids: Vec<String>,
    route_fingerprint: String,
    opaque_state: String,
});

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFrame {
    protocol_version: u32,
    invocation_id: String,
    direction_sequence: u64,
    kind: String,
    payload: Value,
}

pub struct AssistantProtocolDecoder {
    direction: AssistantProtocolDirection,
    next_sequence: u64,
    inbound_events: usize,
    tool_calls: usize,
    call_ids: BTreeSet<String>,
    terminal: bool,
}

impl AssistantProtocolDecoder {
    #[must_use]
    pub fn new(direction: AssistantProtocolDirection) -> Self {
        Self {
            direction,
            next_sequence: 1,
            inbound_events: 0,
            tool_calls: 0,
            call_ids: BTreeSet::new(),
            terminal: false,
        }
    }

    pub fn decode(
        &mut self,
        encoded: &[u8],
    ) -> Result<AssistantProtocolFrame, AssistantProtocolError> {
        if encoded.len() > MAX_ASSISTANT_PROTOCOL_FRAME_BYTES {
            return Err(AssistantProtocolError::FrameTooLarge);
        }
        if !encoded.ends_with(b"\n") {
            return Err(AssistantProtocolError::PartialFrame);
        }
        if self.terminal {
            return Err(AssistantProtocolError::FrameAfterTerminal);
        }
        let value = decode_strict_json(&encoded[..encoded.len() - 1])?;
        if json_depth(&value, 0) > MAX_ASSISTANT_PROTOCOL_JSON_DEPTH {
            return Err(AssistantProtocolError::JsonTooDeep);
        }
        let raw: RawFrame =
            serde_json::from_value(value).map_err(|_| AssistantProtocolError::InvalidFrame)?;
        if raw.protocol_version != ASSISTANT_PROTOCOL_VERSION
            || !valid_invocation_id(&raw.invocation_id)
        {
            return Err(AssistantProtocolError::InvalidFrame);
        }
        if raw.direction_sequence != self.next_sequence {
            return Err(AssistantProtocolError::InvalidSequence);
        }
        let payload = decode_payload(&raw.kind, raw.payload)?;
        if direction_of(&payload) != self.direction {
            return Err(AssistantProtocolError::WrongDirection);
        }
        validate_payload(&payload)?;
        self.record(&payload)?;
        self.next_sequence += 1;
        Ok(AssistantProtocolFrame {
            protocol_version: raw.protocol_version,
            invocation_id: raw.invocation_id,
            direction_sequence: raw.direction_sequence,
            payload,
        })
    }

    fn record(&mut self, payload: &AssistantProtocolPayload) -> Result<(), AssistantProtocolError> {
        self.inbound_events += 1;
        if self.inbound_events > MAX_ASSISTANT_PROTOCOL_INBOUND_EVENTS {
            return Err(AssistantProtocolError::BudgetExceeded);
        }
        if let AssistantProtocolPayload::ToolCall(call) = payload {
            self.tool_calls += 1;
            if self.tool_calls > MAX_ASSISTANT_PROTOCOL_TOOL_CALLS {
                return Err(AssistantProtocolError::BudgetExceeded);
            }
            if !valid_bounded_text(&call.call_id, 128)
                || !self.call_ids.insert(call.call_id.clone())
            {
                return Err(AssistantProtocolError::DuplicateCallId);
            }
        }
        self.terminal = matches!(
            payload,
            AssistantProtocolPayload::InvocationCompleted(_)
                | AssistantProtocolPayload::InvocationFailed(_)
        );
        Ok(())
    }
}

fn decode_payload(
    kind: &str,
    value: Value,
) -> Result<AssistantProtocolPayload, AssistantProtocolError> {
    macro_rules! payload {
        ($variant:ident, $type:ty) => {
            AssistantProtocolPayload::$variant(
                serde_json::from_value::<$type>(value)
                    .map_err(|_| AssistantProtocolError::InvalidFrame)?,
            )
        };
    }
    Ok(match kind {
        "InvocationStart" => payload!(InvocationStart, InvocationStartPayload),
        "ToolResult" => payload!(ToolResult, ToolResultPayload),
        "ContinuationResume" => payload!(ContinuationResume, ContinuationResumePayload),
        "InvocationCancel" => payload!(InvocationCancel, InvocationCancelPayload),
        "InvocationAccepted" => payload!(InvocationAccepted, InvocationAcceptedPayload),
        "ModelOutputDelta" => payload!(ModelOutputDelta, ModelOutputDeltaPayload),
        "ToolCall" => payload!(ToolCall, ToolCallPayload),
        "ReviewerVerdict" => payload!(ReviewerVerdict, ReviewerVerdictPayload),
        "ContinuationEnvelopeReady" => {
            payload!(ContinuationEnvelopeReady, ContinuationEnvelopeReadyPayload)
        }
        "InvocationCompleted" => payload!(InvocationCompleted, InvocationCompletedPayload),
        "InvocationFailed" => payload!(InvocationFailed, InvocationFailedPayload),
        _ => return Err(AssistantProtocolError::InvalidFrame),
    })
}

fn direction_of(payload: &AssistantProtocolPayload) -> AssistantProtocolDirection {
    match payload {
        AssistantProtocolPayload::InvocationStart(_)
        | AssistantProtocolPayload::ToolResult(_)
        | AssistantProtocolPayload::ContinuationResume(_)
        | AssistantProtocolPayload::InvocationCancel(_) => AssistantProtocolDirection::RustToPython,
        _ => AssistantProtocolDirection::PythonToRust,
    }
}

fn valid_invocation_id(value: &str) -> bool {
    uuid::Uuid::parse_str(value).is_ok_and(|value| !value.is_nil())
}

fn valid_bounded_text(value: &str, maximum: usize) -> bool {
    !value.is_empty() && value.len() <= maximum
}

fn validate_payload(payload: &AssistantProtocolPayload) -> Result<(), AssistantProtocolError> {
    let valid = match payload {
        AssistantProtocolPayload::InvocationAccepted(value) => {
            matches!(value.agent_id.as_str(), "workflow_coauthor@1" | "workflow_change_reviewer@1")
        }
        AssistantProtocolPayload::ModelOutputDelta(value) => {
            valid_bounded_text(&value.text, 1024 * 1024)
        }
        AssistantProtocolPayload::ToolCall(value) => {
            valid_bounded_text(&value.call_id, 128)
                && crate::application::AssistantToolId::try_new(&value.tool_id).is_ok()
        }
        AssistantProtocolPayload::ToolResult(value) => {
            valid_bounded_text(&value.call_id, 128)
                && crate::application::AssistantToolId::try_new(&value.tool_id).is_ok()
        }
        AssistantProtocolPayload::InvocationCompleted(value) => {
            valid_bounded_text(&value.final_text, 16 * 1024 * 1024)
        }
        AssistantProtocolPayload::InvocationFailed(value) => {
            valid_bounded_text(&value.category, 128)
                && valid_bounded_text(&value.safe_message, 4096)
        }
        AssistantProtocolPayload::InvocationStart(value) => {
            validate_start(&value.start)
                && value.tool_contracts.len() == 11
                && value.tool_contracts.iter().all(|contract| {
                    crate::application::AssistantToolId::try_new(&contract.tool_id).is_ok()
                })
                && value
                    .tool_contracts
                    .iter()
                    .map(|contract| contract.tool_id.as_str())
                    .collect::<BTreeSet<_>>()
                    .len()
                    == 11
                && valid_budgets(&value.budgets)
        }
        AssistantProtocolPayload::ContinuationResume(value) => valid_continuation(&value.envelope),
        AssistantProtocolPayload::ContinuationEnvelopeReady(value) => {
            valid_continuation(&value.envelope)
        }
        AssistantProtocolPayload::ReviewerVerdict(value) => {
            valid_invocation_id(&value.change_id)
                && value.mutation_digest.len() == 64
                && value.mutation_digest.bytes().all(|byte| byte.is_ascii_hexdigit())
                && matches!(value.verdict.as_str(), "Pass" | "Reject")
                && valid_bounded_text(&value.prose, 64 * 1024)
        }
        AssistantProtocolPayload::InvocationCancel(_) => true,
    };
    if valid { Ok(()) } else { Err(AssistantProtocolError::InvalidFrame) }
}

fn validate_start(start: &InvocationStartKind) -> bool {
    match start {
        InvocationStartKind::UserMessage { message } => valid_bounded_text(message, 16 * 1024),
        InvocationStartKind::RepairActivation { activation_id, failed_workflow_run_id, .. } => {
            valid_invocation_id(activation_id) && valid_invocation_id(failed_workflow_run_id)
        }
    }
}

fn valid_budgets(value: &InvocationBudgets) -> bool {
    value.maximum_frame_bytes == MAX_ASSISTANT_PROTOCOL_FRAME_BYTES
        && value.maximum_events == MAX_ASSISTANT_PROTOCOL_INBOUND_EVENTS
        && value.maximum_tool_calls == MAX_ASSISTANT_PROTOCOL_TOOL_CALLS
        && value.maximum_model_turns == 16
        && value.maximum_direction_bytes == 16 * 1024 * 1024
        && value.deadline_ms == 10 * 60 * 1000
}

fn valid_continuation(value: &ContinuationEnvelopePayload) -> bool {
    value.protocol_version == ASSISTANT_PROTOCOL_VERSION
        && value.contract_epoch == 2
        && value.sdk_version == "0.18.1"
        && matches!(value.agent_id.as_str(), "workflow_coauthor@1" | "workflow_change_reviewer@1")
        && value.tool_ids.len() == 11
        && value
            .tool_ids
            .iter()
            .all(|tool_id| crate::application::AssistantToolId::try_new(tool_id).is_ok())
        && value.tool_ids.iter().collect::<BTreeSet<_>>().len() == 11
        && value.route_fingerprint.len() == 64
        && value
            .route_fingerprint
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        && valid_bounded_text(&value.opaque_state, 4 * 1024 * 1024)
}

#[cfg(test)]
mod tests;
