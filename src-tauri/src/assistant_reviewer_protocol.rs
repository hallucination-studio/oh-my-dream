//! Reviewer protocol translation into Assistant-owned evidence and continuation use cases.

use assistant::{
    application::{
        AssistantReviewWorkflowChangeUseCase, AssistantReviewerFetchCommand,
        AssistantReviewerVerdictCommand, AssistantToolExecutionContext,
    },
    domain::{
        AssistantContractEpoch, AssistantModelContinuationRef, AssistantModelIdentity,
        AssistantModelInvocationId, AssistantReviewVerdict, AssistantToolCallId,
        AssistantWorkflowChangeId, AssistantWorkflowMutationDigest,
    },
    interfaces::{
        AssistantApplicationError, AssistantClockInterface, AssistantModelContinuationEnvelope,
        AssistantModelContinuationStoreInterface, AssistantStoredContinuation,
        AssistantWorkflowChangeRepositoryInterface,
    },
};
use async_trait::async_trait;
use sha2::{Digest, Sha256};

use crate::assistant_model_runner::AssistantReviewerProtocolInterface;

#[derive(Clone)]
pub struct DesktopAssistantReviewerProtocolAdapterImpl<R, C, S> {
    review: AssistantReviewWorkflowChangeUseCase<R, C>,
    continuations: S,
}

impl<R, C, S> DesktopAssistantReviewerProtocolAdapterImpl<R, C, S> {
    #[must_use]
    pub const fn new(review: AssistantReviewWorkflowChangeUseCase<R, C>, continuations: S) -> Self {
        Self { review, continuations }
    }
}

#[async_trait]
impl<R, C, S> AssistantReviewerProtocolInterface
    for DesktopAssistantReviewerProtocolAdapterImpl<R, C, S>
where
    R: AssistantWorkflowChangeRepositoryInterface,
    C: AssistantClockInterface,
    S: AssistantModelContinuationStoreInterface,
{
    async fn record_assistant_reviewer_candidate_fetch(
        &self,
        context: &AssistantToolExecutionContext,
        invocation_id: AssistantModelInvocationId,
        tool_call_id: &str,
        change_id: AssistantWorkflowChangeId,
    ) -> Result<(), AssistantApplicationError> {
        self.review
            .record_candidate_fetch(AssistantReviewerFetchCommand {
                project_id: context.project_id,
                session_id: context.session_id,
                invocation_id,
                tool_call_id: AssistantToolCallId::new(tool_call_id)
                    .map_err(|_| AssistantApplicationError::ReviewEvidenceInvalid)?,
                change_id,
            })
            .await
            .map(|_| ())
    }

    async fn accept_assistant_reviewer_verdict(
        &self,
        context: &AssistantToolExecutionContext,
        invocation_id: AssistantModelInvocationId,
        change_id: AssistantWorkflowChangeId,
        mutation_digest_hex: &str,
        verdict: &str,
        continuation: Option<AssistantModelContinuationEnvelope>,
    ) -> Result<(), AssistantApplicationError> {
        let continuation_ref = match continuation {
            Some(envelope) => {
                let continuation_ref = continuation_ref(envelope.as_bytes())?;
                self.continuations
                    .store_assistant_model_continuation(AssistantStoredContinuation {
                        continuation_ref: continuation_ref.clone(),
                        project_id: context.project_id,
                        session_id: context.session_id,
                        invocation_id,
                        envelope,
                    })
                    .await?;
                Some(continuation_ref)
            }
            None => None,
        };
        self.review
            .accept_reviewer_verdict(AssistantReviewerVerdictCommand {
                project_id: context.project_id,
                session_id: context.session_id,
                invocation_id,
                change_id,
                mutation_digest: mutation_digest(mutation_digest_hex)?,
                verdict: review_verdict(verdict)?,
                reviewer_contract_epoch: AssistantContractEpoch::new(2)
                    .map_err(|_| AssistantApplicationError::ReviewEvidenceInvalid)?,
                reviewer_model: AssistantModelIdentity::new("workflow_change_reviewer@1")
                    .map_err(|_| AssistantApplicationError::ReviewEvidenceInvalid)?,
                continuation_ref,
            })
            .await
            .map(|_| ())
    }
}

fn continuation_ref(
    bytes: &[u8],
) -> Result<AssistantModelContinuationRef, AssistantApplicationError> {
    AssistantModelContinuationRef::new(format!("{:x}", Sha256::digest(bytes)))
        .map_err(|_| AssistantApplicationError::ContinuationIncompatible)
}

fn mutation_digest(
    value: &str,
) -> Result<AssistantWorkflowMutationDigest, AssistantApplicationError> {
    if value.len() != 64 {
        return Err(AssistantApplicationError::ReviewEvidenceInvalid);
    }
    let mut bytes = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        bytes[index] = decode_hex(pair)?;
    }
    Ok(AssistantWorkflowMutationDigest::new(bytes))
}

fn decode_hex(pair: &[u8]) -> Result<u8, AssistantApplicationError> {
    let high = digit(pair[0])?;
    let low = digit(pair[1])?;
    Ok((high << 4) | low)
}

const fn digit(value: u8) -> Result<u8, AssistantApplicationError> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        _ => Err(AssistantApplicationError::ReviewEvidenceInvalid),
    }
}

const fn review_verdict(value: &str) -> Result<AssistantReviewVerdict, AssistantApplicationError> {
    match value.as_bytes() {
        b"Pass" => Ok(AssistantReviewVerdict::Pass),
        b"Reject" => Ok(AssistantReviewVerdict::Reject),
        _ => Err(AssistantApplicationError::ReviewEvidenceInvalid),
    }
}
