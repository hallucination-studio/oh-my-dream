use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use engine::node_capability::*;

use crate::generation_capability_execution::{
    SelectedGenerationProfile, generation_profile_readiness, invalid_invocation,
    origin_matches_contract, resolve_inputs_interruption, selected_generation_profile,
    start_generation_task,
};
use crate::{
    GenerationProfileAvailabilityReaderInterface, GenerationProfileCatalog,
    ImageToVideoDurationSeconds, NodeCapabilityGenerationTaskAssetSnapshot,
    NodeCapabilityGenerationTaskRequest, NodeCapabilityGenerationTaskStartRequest,
    NodeCapabilityGenerationTaskStarterInterface, NodeCapabilityManagedMediaReadRequest,
    NodeCapabilityManagedMediaReadSelection, NodeCapabilityManagedMediaReaderInterface,
    NodeCapabilityManagedMediaReference, NodeCapabilityMediaBoundaryError,
    NodeCapabilityReadableImageInput, NodeCapabilityReadableMediaInput,
};

/// Generates one managed Video from an exact managed Image and optional Text.
pub struct ImageToVideoCapabilityImpl<R, A, S> {
    generation_profile_catalog: Arc<GenerationProfileCatalog>,
    generation_profile_availability_reader: A,
    managed_media_reader: R,
    generation_task_starter: S,
    contract: NodeCapabilityContract,
    image_input_key: NodeCapabilityInputKey,
    output_key: NodeCapabilityOutputKey,
}

impl<R, A, S> ImageToVideoCapabilityImpl<R, A, S> {
    /// Builds the frozen `video.generate_from_image@1.0` capability.
    pub fn try_new(
        generation_profile_catalog: Arc<GenerationProfileCatalog>,
        generation_profile_availability_reader: A,
        managed_media_reader: R,
        generation_task_starter: S,
    ) -> Result<Self, NodeCapabilityContractError> {
        let image_input_key = NodeCapabilityInputKey::new("image")?;
        let output_key = NodeCapabilityOutputKey::new("video")?;
        Ok(Self {
            generation_profile_catalog,
            generation_profile_availability_reader,
            managed_media_reader,
            generation_task_starter,
            contract: image_to_video_contract(image_input_key.clone(), output_key.clone())?,
            image_input_key,
            output_key,
        })
    }
}

#[async_trait]
impl<R, A, S> WorkflowNodeCapabilityInterface for ImageToVideoCapabilityImpl<R, A, S>
where
    R: NodeCapabilityManagedMediaReaderInterface,
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
    ) -> Result<WorkflowNodeCapabilityExecutionOutcome, NodeCapabilityExecutionError> {
        if let Some(error) = resolve_inputs_interruption(&self.contract, &request) {
            return Err(error);
        }
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
        let input_image =
            NodeCapabilityGenerationTaskAssetSnapshot::image(readable_image.media_reference());
        let start_request = NodeCapabilityGenerationTaskStartRequest::try_new(
            request.context.clone(),
            request.origin.clone(),
            parameters.selected_profile.profile_ref,
            NodeCapabilityGenerationTaskRequest::Video {
                input_image,
                prompt: inputs.prompt,
                duration_seconds: parameters.duration_seconds,
            },
            self.output_key.clone(),
            vec![input_image],
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

impl<R, A, S> ImageToVideoCapabilityImpl<R, A, S>
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
