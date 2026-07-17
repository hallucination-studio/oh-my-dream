use std::sync::Arc;

use async_trait::async_trait;
use engine::node_capability::*;

use crate::generation_capability_execution::{
    SelectedGenerationProfile, complete_single_output, generation_profile_readiness,
    invalid_invocation, origin_matches_contract, provider_call_interruption, required_text_input,
    selected_generation_profile, write_generated_media,
};
use crate::{
    GenerationProfileAvailabilityReaderInterface, GenerationProfileCatalog, ImageAspectRatio,
    NodeCapabilityProducedMediaPayload, NodeCapabilityProducedMediaProvenance,
    NodeCapabilityProducedMediaWriterInterface, TextToImageProviderInterface,
    TextToImageProviderRequest,
};

/// Generates one managed Image from structured Text.
pub struct TextToImageCapabilityImpl<A, P, W> {
    generation_profile_catalog: Arc<GenerationProfileCatalog>,
    generation_profile_availability_reader: A,
    text_to_image_provider: P,
    produced_media_writer: W,
    contract: NodeCapabilityContract,
    output_key: NodeCapabilityOutputKey,
}

impl<A, P, W> TextToImageCapabilityImpl<A, P, W> {
    /// Builds the frozen `image.generate_from_text@1.0` capability.
    pub fn try_new(
        generation_profile_catalog: Arc<GenerationProfileCatalog>,
        generation_profile_availability_reader: A,
        text_to_image_provider: P,
        produced_media_writer: W,
    ) -> Result<Self, NodeCapabilityContractError> {
        let output_key = NodeCapabilityOutputKey::new("image")?;
        Ok(Self {
            generation_profile_catalog,
            generation_profile_availability_reader,
            text_to_image_provider,
            produced_media_writer,
            contract: text_to_image_contract(output_key.clone())?,
            output_key,
        })
    }
}

#[async_trait]
impl<A, P, W> WorkflowNodeCapabilityInterface for TextToImageCapabilityImpl<A, P, W>
where
    A: GenerationProfileAvailabilityReaderInterface,
    P: TextToImageProviderInterface,
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
        let selected = text_to_image_parameters(&request.normalized_parameters)
            .map(|parameters| parameters.selected_profile);
        generation_profile_readiness(
            &self.generation_profile_catalog,
            &self.generation_profile_availability_reader,
            &self.contract,
            selected,
            request.deadline,
        )
        .await
    }

    async fn execute_node_capability(
        &self,
        request: NodeCapabilityExecutionRequest,
    ) -> Result<WorkflowNodeCapabilityExecutionOutcome, NodeCapabilityExecutionError> {
        let Some(parameters) = text_to_image_parameters(&request.normalized_parameters) else {
            return Err(invalid_invocation(&self.contract, &request));
        };
        let Some(prompt) = required_text_input(&request.inputs, "prompt") else {
            return Err(invalid_invocation(&self.contract, &request));
        };
        if request.inputs.len() != 1 || !origin_matches_contract(&self.contract, &request) {
            return Err(invalid_invocation(&self.contract, &request));
        }
        if let Some(error) = provider_call_interruption(&self.contract, &request) {
            return Err(error);
        }
        let result = self
            .text_to_image_provider
            .generate_image_from_text(TextToImageProviderRequest::new(
                parameters.selected_profile.profile_ref.clone(),
                request.context.clone(),
                prompt,
                parameters.aspect_ratio,
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
            "Generated Image",
            NodeCapabilityProducedMediaProvenance::provider_generated(
                parameters.selected_profile.profile_ref,
            ),
            NodeCapabilityProducedMediaPayload::GeneratedImage(payload),
        )
        .await?;
        complete_single_output(&self.contract, &request, &self.output_key, value)
            .map(WorkflowNodeCapabilityExecutionOutcome::Completed)
    }
}

struct TextToImageParameters {
    selected_profile: SelectedGenerationProfile,
    aspect_ratio: ImageAspectRatio,
}

fn text_to_image_parameters(
    parameters: &NodeCapabilityNormalizedParameters,
) -> Option<TextToImageParameters> {
    let selected_profile = selected_generation_profile(parameters, 2)?;
    let aspect_ratio_key = NodeCapabilityParameterKey::new("aspect_ratio").ok()?;
    let NodeCapabilityParameterValue::Choice(aspect_ratio) = parameters.get(&aspect_ratio_key)?
    else {
        return None;
    };
    let aspect_ratio = match aspect_ratio.as_str() {
        "square" => ImageAspectRatio::Square,
        "landscape_4_3" => ImageAspectRatio::LandscapeFourByThree,
        "portrait_3_4" => ImageAspectRatio::PortraitThreeByFour,
        "landscape_16_9" => ImageAspectRatio::LandscapeSixteenByNine,
        "portrait_9_16" => ImageAspectRatio::PortraitNineBySixteen,
        _ => return None,
    };
    Some(TextToImageParameters { selected_profile, aspect_ratio })
}

fn text_to_image_contract(
    output_key: NodeCapabilityOutputKey,
) -> Result<NodeCapabilityContract, NodeCapabilityContractError> {
    let aspect_choices =
        ["square", "landscape_4_3", "portrait_3_4", "landscape_16_9", "portrait_9_16"]
            .into_iter()
            .map(NodeCapabilityChoiceKey::new)
            .collect::<Result<Vec<_>, _>>()?;
    NodeCapabilityContract::try_new(
        capability_ref("image.generate_from_text")?,
        vec![
            NodeCapabilityParameterContract::required(
                NodeCapabilityParameterKey::new("generation_profile_ref")?,
                NodeCapabilityParameterConstraint::GenerationProfileRef,
            ),
            NodeCapabilityParameterContract::optional_with_default(
                NodeCapabilityParameterKey::new("aspect_ratio")?,
                NodeCapabilityParameterConstraint::choice_allowed_keys(aspect_choices)?,
                NodeCapabilityParameterValue::Choice(NodeCapabilityChoiceKey::new("square")?),
            )?,
        ],
        vec![NodeCapabilityInputContract::new(
            NodeCapabilityInputKey::new("prompt")?,
            NodeCapabilityInputBindingContract::RequiredSingleValue {
                data_type: WorkflowDataType::Text,
            },
        )?],
        vec![NodeCapabilityOutputContract::new(output_key, WorkflowDataType::Image, true)],
        NodeCapabilityExecutionKind::ContentGeneration,
    )
}

fn capability_ref(id: &str) -> Result<NodeCapabilityContractRef, NodeCapabilityContractError> {
    Ok(NodeCapabilityContractRef::new(
        NodeCapabilityContractId::new(id)?,
        NodeCapabilityContractVersion::new(1, 0)?,
    ))
}
