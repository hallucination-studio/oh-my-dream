use serde_json::Value;

use super::{
    AssistantProtocolError, AssistantProtocolFrame, AssistantProtocolPayload,
    MAX_ASSISTANT_PROTOCOL_FRAME_BYTES,
};

/// Encodes one already-typed frame as one bounded canonical NDJSON line.
pub fn encode_assistant_protocol_frame(
    frame: &AssistantProtocolFrame,
) -> Result<Vec<u8>, AssistantProtocolError> {
    if frame.protocol_version != super::ASSISTANT_PROTOCOL_VERSION
        || frame.direction_sequence == 0
        || !super::valid_invocation_id(&frame.invocation_id)
    {
        return Err(AssistantProtocolError::InvalidFrame);
    }
    super::validate_payload(&frame.payload)?;
    let (kind, payload) = encode_payload(&frame.payload)?;
    let value = serde_json::json!({
        "protocol_version": frame.protocol_version,
        "invocation_id": frame.invocation_id,
        "direction_sequence": frame.direction_sequence,
        "kind": kind,
        "payload": payload,
    });
    let mut encoded =
        serde_json::to_vec(&value).map_err(|_| AssistantProtocolError::InvalidFrame)?;
    encoded.push(b'\n');
    if encoded.len() > MAX_ASSISTANT_PROTOCOL_FRAME_BYTES {
        Err(AssistantProtocolError::FrameTooLarge)
    } else {
        Ok(encoded)
    }
}

fn encode_payload(
    payload: &AssistantProtocolPayload,
) -> Result<(&'static str, Value), AssistantProtocolError> {
    macro_rules! encoded {
        ($kind:literal, $value:expr) => {
            ($kind, serde_json::to_value($value).map_err(|_| AssistantProtocolError::InvalidFrame)?)
        };
    }
    Ok(match payload {
        AssistantProtocolPayload::InvocationStart(value) => encoded!("InvocationStart", value),
        AssistantProtocolPayload::ToolResult(value) => encoded!("ToolResult", value),
        AssistantProtocolPayload::ContinuationResume(value) => {
            encoded!("ContinuationResume", value)
        }
        AssistantProtocolPayload::InvocationCancel(value) => encoded!("InvocationCancel", value),
        AssistantProtocolPayload::InvocationAccepted(value) => {
            encoded!("InvocationAccepted", value)
        }
        AssistantProtocolPayload::ModelOutputDelta(value) => encoded!("ModelOutputDelta", value),
        AssistantProtocolPayload::ToolCall(value) => encoded!("ToolCall", value),
        AssistantProtocolPayload::ReviewerVerdict(value) => encoded!("ReviewerVerdict", value),
        AssistantProtocolPayload::ContinuationEnvelopeReady(value) => {
            encoded!("ContinuationEnvelopeReady", value)
        }
        AssistantProtocolPayload::InvocationCompleted(value) => {
            encoded!("InvocationCompleted", value)
        }
        AssistantProtocolPayload::InvocationFailed(value) => encoded!("InvocationFailed", value),
    })
}
