use std::sync::Arc;

use async_trait::async_trait;
use engine::node_capability::*;

use crate::generation_capability_execution::{
    complete_single_output, generation_profile_readiness, invalid_invocation,
    origin_matches_contract, provider_call_interruption, required_text_input,
    selected_generation_profile, write_generated_media,
};
use crate::{
    GenerationProfileAvailabilityReaderInterface, GenerationProfileCatalog,
    NodeCapabilityProducedMediaPayload, NodeCapabilityProducedMediaProvenance,
    NodeCapabilityProducedMediaWriterInterface, TextToSpeechProviderInterface,
    TextToSpeechProviderRequest,
};

/// Synthesizes one managed speech Audio value from structured Text.
pub struct TextToSpeechCapabilityImpl<A, P, W> {
    generation_profile_catalog: Arc<GenerationProfileCatalog>,
    generation_profile_availability_reader: A,
    text_to_speech_provider: P,
    produced_media_writer: W,
    contract: NodeCapabilityContract,
    output_key: NodeCapabilityOutputKey,
}

impl<A, P, W> TextToSpeechCapabilityImpl<A, P, W> {
    /// Builds the frozen `audio.synthesize_speech_from_text@1.0` capability.
    pub fn try_new(
        generation_profile_catalog: Arc<GenerationProfileCatalog>,
        generation_profile_availability_reader: A,
        text_to_speech_provider: P,
        produced_media_writer: W,
    ) -> Result<Self, NodeCapabilityContractError> {
        let output_key = NodeCapabilityOutputKey::new("audio")?;
        Ok(Self {
            generation_profile_catalog,
            generation_profile_availability_reader,
            text_to_speech_provider,
            produced_media_writer,
            contract: text_to_speech_contract(output_key.clone())?,
            output_key,
        })
    }
}

#[async_trait]
impl<A, P, W> WorkflowNodeCapabilityInterface for TextToSpeechCapabilityImpl<A, P, W>
where
    A: GenerationProfileAvailabilityReaderInterface,
    P: TextToSpeechProviderInterface,
    W: NodeCapabilityProducedMediaWriterInterface,
{
    fn node_capability_contract(&self) -> &NodeCapabilityContract {
        &self.contract
    }

    fn normalize_node_parameters(
        &self,
        parameters: &NodeCapabilityParameterSet,
    ) -> Result<NodeCapabilityNormalizedParameters, NodeCapabilityParameterError> {
        self.contract.normalize_node_parameters(parameters)
    }

    async fn check_node_external_readiness(
        &self,
        request: NodeCapabilityReadinessRequest,
    ) -> Vec<NodeCapabilityReadinessIssue> {
        generation_profile_readiness(
            &self.generation_profile_catalog,
            &self.generation_profile_availability_reader,
            &self.contract,
            selected_generation_profile(&request.normalized_parameters, 1),
            request.deadline,
        )
        .await
    }

    async fn execute_node_capability(
        &self,
        request: NodeCapabilityExecutionRequest,
    ) -> Result<WorkflowNodeCapabilityExecutionOutcome, NodeCapabilityExecutionError> {
        let Some(selected_profile) = selected_generation_profile(&request.normalized_parameters, 1)
        else {
            return Err(invalid_invocation(&self.contract, &request));
        };
        let Some(text) = required_text_input(&request.inputs, "text") else {
            return Err(invalid_invocation(&self.contract, &request));
        };
        if request.inputs.len() != 1 || !origin_matches_contract(&self.contract, &request) {
            return Err(invalid_invocation(&self.contract, &request));
        }
        if let Some(error) = provider_call_interruption(&self.contract, &request) {
            return Err(error);
        }
        let result = self
            .text_to_speech_provider
            .synthesize_speech_from_text(TextToSpeechProviderRequest::new(
                selected_profile.profile_ref.clone(),
                request.context.clone(),
                text,
            ))
            .await;
        if let Some(error) = provider_call_interruption(&self.contract, &request) {
            return Err(error);
        }
        let payload = result.map_err(|failure| {
            NodeCapabilityExecutionError::provider_call_failed(
                self.contract.contract_ref().clone(),
                request.context.node_execution_id,
                failure,
            )
        })?;
        let value = write_generated_media(
            &self.produced_media_writer,
            &self.contract,
            &request,
            &self.output_key,
            "Synthesized Speech",
            NodeCapabilityProducedMediaProvenance::provider_generated(selected_profile.profile_ref),
            NodeCapabilityProducedMediaPayload::SynthesizedSpeech(payload),
        )
        .await?;
        complete_single_output(&self.contract, &request, &self.output_key, value)
            .map(WorkflowNodeCapabilityExecutionOutcome::Completed)
    }
}

fn text_to_speech_contract(
    output_key: NodeCapabilityOutputKey,
) -> Result<NodeCapabilityContract, NodeCapabilityContractError> {
    NodeCapabilityContract::try_new(
        NodeCapabilityContractRef::new(
            NodeCapabilityContractId::new("audio.synthesize_speech_from_text")?,
            NodeCapabilityContractVersion::new(1, 0)?,
        ),
        vec![NodeCapabilityParameterContract::required(
            NodeCapabilityParameterKey::new("generation_profile_ref")?,
            NodeCapabilityParameterConstraint::GenerationProfileRef,
        )],
        vec![NodeCapabilityInputContract::new(
            NodeCapabilityInputKey::new("text")?,
            NodeCapabilityInputBindingContract::RequiredSingleValue {
                data_type: WorkflowDataType::Text,
            },
        )?],
        vec![NodeCapabilityOutputContract::new(output_key, WorkflowDataType::Audio, true)],
        NodeCapabilityExecutionKind::ContentGeneration,
    )
}
