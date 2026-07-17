use std::time::{SystemTime, UNIX_EPOCH};

use assistant::{
    domain::AssistantReviewedAt,
    interfaces::{AssistantApplicationError, AssistantClockInterface},
};

/// System UTC clock implementation of the Assistant time boundary.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemAssistantClockAdapterImpl;

impl AssistantClockInterface for SystemAssistantClockAdapterImpl {
    fn current_assistant_time(&self) -> Result<AssistantReviewedAt, AssistantApplicationError> {
        let milliseconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| AssistantApplicationError::ExternalBoundaryFailed)?
            .as_millis();
        let milliseconds = i64::try_from(milliseconds)
            .map_err(|_| AssistantApplicationError::ExternalBoundaryFailed)?;
        AssistantReviewedAt::new(milliseconds)
            .map_err(|_| AssistantApplicationError::ExternalBoundaryFailed)
    }
}
