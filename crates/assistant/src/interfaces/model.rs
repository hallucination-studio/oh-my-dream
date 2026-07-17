use async_trait::async_trait;

use super::{
    AssistantApplicationError, AssistantModelResumeRequest, AssistantModelTurnRequest,
    AssistantModelTurnResult, AssistantStoredContinuation,
};
use crate::domain::AssistantModelContinuationRef;

/// Bounded model-turn boundary consumed by Assistant orchestration.
#[async_trait]
pub trait AssistantModelRunnerInterface: Send + Sync {
    async fn start_assistant_model_turn(
        &self,
        request: AssistantModelTurnRequest,
    ) -> Result<AssistantModelTurnResult, AssistantApplicationError>;

    async fn resume_assistant_model_turn(
        &self,
        request: AssistantModelResumeRequest,
    ) -> Result<AssistantModelTurnResult, AssistantApplicationError>;
}

/// Single-use opaque continuation storage consumed by Assistant orchestration.
#[async_trait]
pub trait AssistantModelContinuationStoreInterface: Send + Sync {
    async fn store_assistant_model_continuation(
        &self,
        continuation: AssistantStoredContinuation,
    ) -> Result<(), AssistantApplicationError>;

    async fn load_assistant_model_continuation(
        &self,
        continuation_ref: &AssistantModelContinuationRef,
    ) -> Result<Option<AssistantStoredContinuation>, AssistantApplicationError>;

    async fn consume_assistant_model_continuation(
        &self,
        continuation_ref: &AssistantModelContinuationRef,
    ) -> Result<Option<AssistantStoredContinuation>, AssistantApplicationError>;
}
