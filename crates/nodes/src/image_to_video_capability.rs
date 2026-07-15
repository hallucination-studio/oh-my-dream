use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use engine::node_capability::*;

use crate::generation_capability_execution::{
    SelectedGenerationProfile, complete_single_output, generation_profile_readiness,
    invalid_invocation, origin_matches_contract, provider_call_interruption,
    selected_generation_profile, write_generated_media,
};
use crate::{
    GenerationProfileAvailabilityReaderInterface, GenerationProfileCatalog,
    ImageToVideoDurationSeconds, ImageToVideoProviderInterface, ImageToVideoProviderRequest,
    NodeCapabilityManagedMediaReadRequest, NodeCapabilityManagedMediaReadSelection,
    NodeCapabilityManagedMediaReaderInterface, NodeCapabilityManagedMediaReference,
    NodeCapabilityMediaBoundaryError, NodeCapabilityProducedMediaPayload,
    NodeCapabilityProducedMediaProvenance, NodeCapabilityProducedMediaWriterInterface,
    NodeCapabilityReadableImageInput, NodeCapabilityReadableMediaInput,
};

/// Generates one managed Video from an exact managed Image and optional Text.
pub struct ImageToVideoCapabilityImpl<R, A, P, W> {
    generation_profile_catalog: Arc<GenerationProfileCatalog>,
    generation_profile_availability_reader: A,
    managed_media_reader: R,
    image_to_video_provider: P,
    produced_media_writer: W,
    contract: NodeCapabilityContract,
    image_input_key: NodeCapabilityInputKey,
    output_key: NodeCapabilityOutputKey,
}

impl<R, A, P, W> ImageToVideoCapabilityImpl<R, A, P, W> {
    /// Builds the frozen `video.generate_from_image@1.0` capability.
    pub fn try_new(
        generation_profile_catalog: Arc<GenerationProfileCatalog>,
        generation_profile_availability_reader: A,
        managed_media_reader: R,
        image_to_video_provider: P,
        produced_media_writer: W,
    ) -> Result<Self, NodeCapabilityContractError> {
        let image_input_key = NodeCapabilityInputKey::new("image")?;
        let output_key = NodeCapabilityOutputKey::new("video")?;
        Ok(Self {
            generation_profile_catalog,
            generation_profile_availability_reader,
            managed_media_reader,
            image_to_video_provider,
            produced_media_writer,
            contract: image_to_video_contract(image_input_key.clone(), output_key.clone())?,
            image_input_key,
            output_key,
        })
    }
}

#[async_trait]
impl<R, A, P, W> WorkflowNodeCapabilityInterface for ImageToVideoCapabilityImpl<R, A, P, W>
where
    R: NodeCapabilityManagedMediaReaderInterface,
    A: GenerationProfileAvailabilityReaderInterface,
    P: ImageToVideoProviderInterface,
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
        let selected = image_to_video_parameters(&request.normalized_parameters)
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
    ) -> Result<WorkflowNodeOutputSet, NodeCapabilityExecutionError> {
        let Some(parameters) = image_to_video_parameters(&request.normalized_parameters) else {
            return Err(invalid_invocation(&self.contract, &request));
        };
        let Some(inputs) = image_to_video_inputs(&request.inputs) else {
            return Err(invalid_invocation(&self.contract, &request));
        };
        if !origin_matches_contract(&self.contract, &request) {
            return Err(invalid_invocation(&self.contract, &request));
        }
        let readable_image = self.read_exact_input_image(&request, inputs.image).await?;
        if let Some(error) = provider_call_interruption(&self.contract, &request) {
            return Err(error);
        }
        let result = self
            .image_to_video_provider
            .generate_video_from_image(ImageToVideoProviderRequest::new(
                parameters.selected_profile.profile_ref.clone(),
                request.context.clone(),
                readable_image,
                inputs.prompt,
                parameters.duration_seconds,
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
        let provenance = NodeCapabilityProducedMediaProvenance::try_provider_derived(
            vec![NodeCapabilityManagedMediaReference::Image(inputs.image)],
            parameters.selected_profile.profile_ref,
        )
        .map_err(|_| self.invalid_media_write_request(&request))?;
        let value = write_generated_media(
            &self.produced_media_writer,
            &self.contract,
            &request,
            &self.output_key,
            "Generated Video",
            provenance,
            NodeCapabilityProducedMediaPayload::GeneratedVideo(payload),
        )
        .await?;
        complete_single_output(&self.contract, &request, &self.output_key, value)
    }
}

impl<R, A, P, W> ImageToVideoCapabilityImpl<R, A, P, W>
where
    R: NodeCapabilityManagedMediaReaderInterface,
{
    async fn read_exact_input_image(
        &self,
        request: &NodeCapabilityExecutionRequest,
        expected_reference: WorkflowManagedImageRef,
    ) -> Result<NodeCapabilityReadableImageInput, NodeCapabilityExecutionError> {
        if let Some(error) = self.image_read_interruption(request) {
            return Err(error);
        }
        let result = self
            .managed_media_reader
            .read_managed_media(NodeCapabilityManagedMediaReadRequest::new(
                request.context.project_id,
                NodeCapabilityManagedMediaReadSelection::ExactReference(
                    NodeCapabilityManagedMediaReference::Image(expected_reference),
                ),
                request.context.deadline.monotonic_instant(),
            ))
            .await;
        if let Some(error) = self.image_read_interruption(request) {
            return Err(error);
        }
        match result {
            Ok(NodeCapabilityReadableMediaInput::Image(value))
                if value.media_reference() == expected_reference =>
            {
                Ok(value)
            }
            Ok(NodeCapabilityReadableMediaInput::Image(_)) => {
                Err(self.image_read_failed(request, NodeCapabilityMediaFailure::DigestMismatch))
            }
            Ok(value) => Err(self.image_read_failed(
                request,
                NodeCapabilityMediaFailure::KindMismatch {
                    expected: WorkflowDataType::Image,
                    observed: value.media_kind().to_workflow_data_type(),
                },
            )),
            Err(error) => Err(self.image_boundary_failed(request, error)),
        }
    }

    fn image_read_interruption(
        &self,
        request: &NodeCapabilityExecutionRequest,
    ) -> Option<NodeCapabilityExecutionError> {
        if request.context.cancellation.is_cancelled() {
            return Some(NodeCapabilityExecutionError::cancelled_while_resolving_input(
                self.contract.contract_ref().clone(),
                request.context.node_execution_id,
                self.image_input_key.clone(),
            ));
        }
        if request.context.deadline.is_reached_at(Instant::now()) {
            return Some(NodeCapabilityExecutionError::deadline_exceeded_while_resolving_input(
                self.contract.contract_ref().clone(),
                request.context.node_execution_id,
                self.image_input_key.clone(),
            ));
        }
        None
    }

    fn image_boundary_failed(
        &self,
        request: &NodeCapabilityExecutionRequest,
        error: NodeCapabilityMediaBoundaryError,
    ) -> NodeCapabilityExecutionError {
        match error {
            NodeCapabilityMediaBoundaryError::Cancelled => {
                NodeCapabilityExecutionError::cancelled_while_resolving_input(
                    self.contract.contract_ref().clone(),
                    request.context.node_execution_id,
                    self.image_input_key.clone(),
                )
            }
            NodeCapabilityMediaBoundaryError::DeadlineExceeded => {
                NodeCapabilityExecutionError::deadline_exceeded_while_resolving_input(
                    self.contract.contract_ref().clone(),
                    request.context.node_execution_id,
                    self.image_input_key.clone(),
                )
            }
            NodeCapabilityMediaBoundaryError::Media(failure) => {
                self.image_read_failed(request, failure)
            }
        }
    }

    fn image_read_failed(
        &self,
        request: &NodeCapabilityExecutionRequest,
        failure: NodeCapabilityMediaFailure,
    ) -> NodeCapabilityExecutionError {
        NodeCapabilityExecutionError::managed_media_input_resolution_failed(
            self.contract.contract_ref().clone(),
            request.context.node_execution_id,
            self.image_input_key.clone(),
            failure,
        )
    }

    fn invalid_media_write_request(
        &self,
        request: &NodeCapabilityExecutionRequest,
    ) -> NodeCapabilityExecutionError {
        NodeCapabilityExecutionError::invalid_result_while_constructing_media_write(
            self.contract.contract_ref().clone(),
            request.context.node_execution_id,
            self.output_key.clone(),
        )
    }
}

struct ImageToVideoParameters {
    selected_profile: SelectedGenerationProfile,
    duration_seconds: ImageToVideoDurationSeconds,
}

struct ImageToVideoInputs {
    image: WorkflowManagedImageRef,
    prompt: Option<WorkflowTextValue>,
}

fn image_to_video_parameters(
    parameters: &NodeCapabilityNormalizedParameters,
) -> Option<ImageToVideoParameters> {
    let selected_profile = selected_generation_profile(parameters, 2)?;
    let duration_key = NodeCapabilityParameterKey::new("duration_seconds").ok()?;
    let NodeCapabilityParameterValue::UnsignedInteger(duration) = parameters.get(&duration_key)?
    else {
        return None;
    };
    let duration_seconds = match duration {
        5 => ImageToVideoDurationSeconds::Five,
        10 => ImageToVideoDurationSeconds::Ten,
        _ => return None,
    };
    Some(ImageToVideoParameters { selected_profile, duration_seconds })
}

fn image_to_video_inputs(inputs: &WorkflowNodeInputSet) -> Option<ImageToVideoInputs> {
    let image_key = NodeCapabilityInputKey::new("image").ok()?;
    let WorkflowNodeInputValue::Single(image_item) = inputs.get(&image_key)? else { return None };
    let WorkflowRuntimeValue::Image(image) = image_item.value else { return None };
    if image_item.input_role_key.is_some() {
        return None;
    }
    let prompt_key = NodeCapabilityInputKey::new("prompt").ok()?;
    let prompt = match inputs.get(&prompt_key) {
        None => None,
        Some(WorkflowNodeInputValue::Single(item)) if item.input_role_key.is_none() => {
            match &item.value {
                WorkflowRuntimeValue::Text(value) => Some(value.clone()),
                _ => return None,
            }
        }
        _ => return None,
    };
    if inputs.len() != usize::from(prompt.is_some()) + 1 {
        return None;
    }
    Some(ImageToVideoInputs { image, prompt })
}

fn image_to_video_contract(
    image_input_key: NodeCapabilityInputKey,
    output_key: NodeCapabilityOutputKey,
) -> Result<NodeCapabilityContract, NodeCapabilityContractError> {
    NodeCapabilityContract::try_new(
        NodeCapabilityContractRef::new(
            NodeCapabilityContractId::new("video.generate_from_image")?,
            NodeCapabilityContractVersion::new(1, 0)?,
        ),
        vec![
            NodeCapabilityParameterContract::required(
                NodeCapabilityParameterKey::new("generation_profile_ref")?,
                NodeCapabilityParameterConstraint::GenerationProfileRef,
            ),
            NodeCapabilityParameterContract::optional_with_default(
                NodeCapabilityParameterKey::new("duration_seconds")?,
                NodeCapabilityParameterConstraint::unsigned_integer_allowed_values([5, 10])?,
                NodeCapabilityParameterValue::UnsignedInteger(5),
            )?,
        ],
        vec![
            NodeCapabilityInputContract::new(
                image_input_key,
                NodeCapabilityInputBindingContract::RequiredSingleValue {
                    data_type: WorkflowDataType::Image,
                },
            )?,
            NodeCapabilityInputContract::new(
                NodeCapabilityInputKey::new("prompt")?,
                NodeCapabilityInputBindingContract::OptionalSingleValue {
                    data_type: WorkflowDataType::Text,
                },
            )?,
        ],
        vec![NodeCapabilityOutputContract::new(output_key, WorkflowDataType::Video, true)],
        NodeCapabilityExecutionKind::MediaTransformation,
    )
}
