use std::sync::Arc;

use async_trait::async_trait;
use engine::node_capability::*;

use crate::generation_capability_execution::{
    generation_profile_readiness, invalid_invocation, origin_matches_contract, required_text_input,
    resolve_inputs_interruption, selected_generation_profile, start_generation_task,
};
use crate::{
    GenerationProfileAvailabilityReaderInterface, GenerationProfileCatalog,
    NodeCapabilityGenerationTaskRequest, NodeCapabilityGenerationTaskStartRequest,
    NodeCapabilityGenerationTaskStarterInterface,
};

/// Synthesizes one managed speech Audio value from structured Text.
pub struct TextToSpeechCapabilityImpl<A, S> {
    generation_profile_catalog: Arc<GenerationProfileCatalog>,
    generation_profile_availability_reader: A,
    generation_task_starter: S,
    contract: NodeCapabilityContract,
    output_key: NodeCapabilityOutputKey,
}

impl<A, S> TextToSpeechCapabilityImpl<A, S> {
    /// Builds the frozen `audio.synthesize_speech_from_text@1.0` capability.
    pub fn try_new(
        generation_profile_catalog: Arc<GenerationProfileCatalog>,
        generation_profile_availability_reader: A,
        generation_task_starter: S,
    ) -> Result<Self, NodeCapabilityContractError> {
        let output_key = NodeCapabilityOutputKey::new("audio")?;
        Ok(Self {
            generation_profile_catalog,
            generation_profile_availability_reader,
            generation_task_starter,
            contract: text_to_speech_contract(output_key.clone())?,
            output_key,
        })
    }
}

#[async_trait]
impl<A, S> WorkflowNodeCapabilityInterface for TextToSpeechCapabilityImpl<A, S>
where
    A: GenerationProfileAvailabilityReaderInterface,
    S: NodeCapabilityGenerationTaskStarterInterface,
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
        if let Some(error) = resolve_inputs_interruption(&self.contract, &request) {
            return Err(error);
        }
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
        let start_request = NodeCapabilityGenerationTaskStartRequest::try_new(
            request.context.clone(),
            request.origin.clone(),
            selected_profile.profile_ref,
            NodeCapabilityGenerationTaskRequest::Voice { text },
            self.output_key.clone(),
            Vec::new(),
        )
        .map_err(|failure| {
            NodeCapabilityExecutionError::generation_task_start_failed(
                self.contract.contract_ref().clone(),
                request.context.node_execution_id,
                failure,
            )
        })?;
        start_generation_task(
            &self.generation_task_starter,
            &self.contract,
            &request,
            start_request,
        )
        .await
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
