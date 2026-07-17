use super::{
    AssistantProtocolDirection, AssistantProtocolError, AssistantProtocolFrame,
    AssistantProtocolPayload,
};

const MAX_DIRECTION_BYTES: usize = 16 * 1024 * 1024;
const MAX_MODEL_TURNS: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ConversationState {
    Initial,
    AwaitingAcceptance,
    Active,
    AwaitingToolResult,
    AwaitingFailure,
    Terminal,
}

/// Validates the legal cross-direction order and per-invocation budgets.
pub struct AssistantProtocolConversationValidator {
    invocation_id: Option<String>,
    state: ConversationState,
    pending_call_id: Option<String>,
    rust_bytes: usize,
    python_bytes: usize,
    model_turns: usize,
}

impl Default for AssistantProtocolConversationValidator {
    fn default() -> Self {
        Self {
            invocation_id: None,
            state: ConversationState::Initial,
            pending_call_id: None,
            rust_bytes: 0,
            python_bytes: 0,
            model_turns: 0,
        }
    }
}

impl AssistantProtocolConversationValidator {
    pub fn admit(
        &mut self,
        direction: AssistantProtocolDirection,
        encoded_bytes: usize,
        frame: &AssistantProtocolFrame,
    ) -> Result<(), AssistantProtocolError> {
        self.validate_identity(frame)?;
        self.record_bytes(direction, encoded_bytes)?;
        self.transition(&frame.payload)
    }

    fn validate_identity(
        &mut self,
        frame: &AssistantProtocolFrame,
    ) -> Result<(), AssistantProtocolError> {
        match &self.invocation_id {
            Some(value) if value != &frame.invocation_id => {
                Err(AssistantProtocolError::InvalidFrame)
            }
            Some(_) => Ok(()),
            None => {
                self.invocation_id = Some(frame.invocation_id.clone());
                Ok(())
            }
        }
    }

    fn record_bytes(
        &mut self,
        direction: AssistantProtocolDirection,
        encoded_bytes: usize,
    ) -> Result<(), AssistantProtocolError> {
        let total = match direction {
            AssistantProtocolDirection::RustToPython => &mut self.rust_bytes,
            AssistantProtocolDirection::PythonToRust => &mut self.python_bytes,
        };
        *total = total.saturating_add(encoded_bytes);
        if *total > MAX_DIRECTION_BYTES {
            Err(AssistantProtocolError::BudgetExceeded)
        } else {
            Ok(())
        }
    }

    fn transition(
        &mut self,
        payload: &AssistantProtocolPayload,
    ) -> Result<(), AssistantProtocolError> {
        use AssistantProtocolPayload as Payload;
        let next = match (self.state, payload) {
            (
                ConversationState::Initial,
                Payload::InvocationStart(_) | Payload::ContinuationResume(_),
            ) => {
                self.model_turns += 1;
                ConversationState::AwaitingAcceptance
            }
            (ConversationState::AwaitingAcceptance, Payload::InvocationAccepted(_)) => {
                ConversationState::Active
            }
            (
                ConversationState::Active,
                Payload::ModelOutputDelta(_)
                | Payload::ReviewerVerdict(_)
                | Payload::ContinuationEnvelopeReady(_),
            ) => ConversationState::Active,
            (ConversationState::Active, Payload::ToolCall(call)) => {
                self.pending_call_id = Some(call.call_id.clone());
                ConversationState::AwaitingToolResult
            }
            (ConversationState::AwaitingToolResult, Payload::ToolResult(result))
                if self.pending_call_id.as_deref() == Some(result.call_id.as_str()) =>
            {
                self.pending_call_id = None;
                ConversationState::Active
            }
            (ConversationState::Active, Payload::InvocationCancel(_)) => {
                ConversationState::AwaitingFailure
            }
            (ConversationState::AwaitingFailure, Payload::InvocationFailed(_))
            | (
                ConversationState::Active,
                Payload::InvocationCompleted(_) | Payload::InvocationFailed(_),
            ) => ConversationState::Terminal,
            (ConversationState::Terminal, _) => {
                return Err(AssistantProtocolError::FrameAfterTerminal);
            }
            _ => return Err(AssistantProtocolError::InvalidFrame),
        };
        if self.model_turns > MAX_MODEL_TURNS {
            return Err(AssistantProtocolError::BudgetExceeded);
        }
        self.state = next;
        Ok(())
    }
}
