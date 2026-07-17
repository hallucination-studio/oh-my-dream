use std::collections::BTreeMap;
use std::time::Instant;

use engine::node_capability::*;

use crate::{
    GenerationProfileAvailabilityReaderInterface, GenerationProfileAvailabilityRequest,
    GenerationProfileAvailabilityState, GenerationProfileCatalog, GenerationProfileLifecycleState,
    GenerationProfileRef, NodeCapabilityMediaBoundaryError, NodeCapabilityMediaKind,
    NodeCapabilityProducedMediaDisplayName, NodeCapabilityProducedMediaOutputKey,
    NodeCapabilityProducedMediaPayload, NodeCapabilityProducedMediaProvenance,
    NodeCapabilityProducedMediaReference, NodeCapabilityProducedMediaWriteRequest,
    NodeCapabilityProducedMediaWriterInterface,
};

pub(crate) struct SelectedGenerationProfile {
    pub(crate) boundary_ref: NodeCapabilityGenerationProfileRefParameterValue,
    pub(crate) profile_ref: GenerationProfileRef,
}

pub(crate) fn selected_generation_profile(
    parameters: &NodeCapabilityNormalizedParameters,
    expected_parameter_count: usize,
) -> Option<SelectedGenerationProfile> {
    if parameters.len() != expected_parameter_count {
        return None;
    }
    let key = NodeCapabilityParameterKey::new("generation_profile_ref").ok()?;
    let NodeCapabilityParameterValue::GenerationProfile(boundary_ref) = parameters.get(&key)?
    else {
        return None;
    };
    let profile_ref =
        GenerationProfileRef::try_from_node_capability_parameter_value(boundary_ref).ok()?;
    Some(SelectedGenerationProfile { boundary_ref: boundary_ref.clone(), profile_ref })
}

pub(crate) async fn generation_profile_readiness<A>(
    catalog: &GenerationProfileCatalog,
    availability_reader: &A,
    contract: &NodeCapabilityContract,
    selected: Option<SelectedGenerationProfile>,
    deadline: NodeCapabilityReadinessDeadline,
) -> Vec<NodeCapabilityReadinessIssue>
where
    A: GenerationProfileAvailabilityReaderInterface,
{
    let Some(selected) = selected else {
        return vec![NodeCapabilityReadinessIssue::invalid_capability_invocation()];
    };
    let parameter_key = match NodeCapabilityParameterKey::new("generation_profile_ref") {
        Ok(value) => value,
        Err(_) => return vec![NodeCapabilityReadinessIssue::invalid_capability_invocation()],
    };
    let compatible =
        catalog.find_generation_profile(&selected.profile_ref).is_ok_and(|definition| {
            definition.lifecycle_state() == GenerationProfileLifecycleState::Active
                && definition.compatible_capabilities().contains(contract.contract_ref())
        });
    if !compatible {
        return vec![NodeCapabilityReadinessIssue::generation_profile_incompatible(
            parameter_key,
            selected.boundary_ref,
        )];
    }
    let state = read_selected_profile_availability(
        availability_reader,
        contract.contract_ref(),
        &selected.profile_ref,
        deadline,
    )
    .await;
    match state {
        Err(()) => {
            vec![NodeCapabilityReadinessIssue::generation_profile_availability_indeterminate(
                parameter_key,
                selected.boundary_ref,
            )]
        }
        Ok(GenerationProfileAvailabilityState::Available) => Vec::new(),
        Ok(GenerationProfileAvailabilityState::Unavailable { .. }) => {
            vec![NodeCapabilityReadinessIssue::generation_profile_unavailable(
                parameter_key,
                selected.boundary_ref,
            )]
        }
        Ok(GenerationProfileAvailabilityState::Indeterminate { .. }) => {
            vec![NodeCapabilityReadinessIssue::generation_profile_availability_indeterminate(
                parameter_key,
                selected.boundary_ref,
            )]
        }
    }
}

async fn read_selected_profile_availability<A>(
    reader: &A,
    contract_ref: &NodeCapabilityContractRef,
    profile_ref: &GenerationProfileRef,
    deadline: NodeCapabilityReadinessDeadline,
) -> Result<GenerationProfileAvailabilityState, ()>
where
    A: GenerationProfileAvailabilityReaderInterface,
{
    let request = GenerationProfileAvailabilityRequest::try_new(
        contract_ref.clone(),
        vec![profile_ref.clone()],
        deadline.monotonic_instant(),
    )
    .map_err(|_| ())?;
    let observations =
        reader.read_generation_profile_availability(request).await.map_err(|_| ())?;
    match observations.as_slice() {
        [observation] if observation.profile_ref() == profile_ref => {
            Ok(observation.state().clone())
        }
        _ => Err(()),
    }
}

pub(crate) fn invalid_invocation(
    contract: &NodeCapabilityContract,
    request: &NodeCapabilityExecutionRequest,
) -> NodeCapabilityExecutionError {
    NodeCapabilityExecutionError::invalid_capability_invocation(
        contract.contract_ref().clone(),
        request.context.node_execution_id,
    )
}

pub(crate) fn origin_matches_contract(
    contract: &NodeCapabilityContract,
    request: &NodeCapabilityExecutionRequest,
) -> bool {
    request.origin.capability_contract_ref() == contract.contract_ref()
}

pub(crate) fn required_text_input(
    inputs: &WorkflowNodeInputSet,
    key: &str,
) -> Option<WorkflowTextValue> {
    let key = NodeCapabilityInputKey::new(key).ok()?;
    let WorkflowNodeInputValue::Single(item) = inputs.get(&key)? else { return None };
    match &item.value {
        WorkflowRuntimeValue::Text(value) if item.input_role_key.is_none() => Some(value.clone()),
        _ => None,
    }
}

pub(crate) fn provider_call_interruption(
    contract: &NodeCapabilityContract,
    request: &NodeCapabilityExecutionRequest,
) -> Option<NodeCapabilityExecutionError> {
    if request.context.cancellation.is_cancelled() {
        return Some(NodeCapabilityExecutionError::cancelled_while_calling_provider(
            contract.contract_ref().clone(),
            request.context.node_execution_id,
        ));
    }
    if request.context.deadline.is_reached_at(Instant::now()) {
        return Some(NodeCapabilityExecutionError::deadline_exceeded_while_calling_provider(
            contract.contract_ref().clone(),
            request.context.node_execution_id,
        ));
    }
    None
}

pub(crate) async fn write_generated_media<W>(
    writer: &W,
    contract: &NodeCapabilityContract,
    request: &NodeCapabilityExecutionRequest,
    output_key: &NodeCapabilityOutputKey,
    display_name: &str,
    provenance: NodeCapabilityProducedMediaProvenance,
    payload: NodeCapabilityProducedMediaPayload,
) -> Result<WorkflowRuntimeValue, NodeCapabilityExecutionError>
where
    W: NodeCapabilityProducedMediaWriterInterface,
{
    let expected_kind = payload.media_kind();
    let expected_digest = payload.digest();
    let write_request = NodeCapabilityProducedMediaWriteRequest::try_new(
        request.context.clone(),
        request.origin.clone(),
        NodeCapabilityProducedMediaOutputKey::new(
            request.context.workflow_run_id,
            request.context.node_execution_id,
            output_key.clone(),
            0,
        ),
        NodeCapabilityProducedMediaDisplayName::try_new(display_name).map_err(|_| {
            invalid_result_while_constructing_media_write(contract, request, output_key)
        })?,
        provenance,
        payload,
    )
    .map_err(|_| invalid_result_while_constructing_media_write(contract, request, output_key))?;
    if let Some(error) = media_write_interruption(contract, request, output_key) {
        return Err(error);
    }
    let result = writer.write_node_output_media(write_request).await;
    if let Some(error) = media_write_interruption(contract, request, output_key) {
        return Err(error);
    }
    let reference =
        result.map_err(|error| media_write_boundary_error(contract, request, output_key, error))?;
    produced_runtime_value(contract, request, output_key, expected_kind, expected_digest, reference)
}

pub(crate) fn complete_single_output(
    contract: &NodeCapabilityContract,
    request: &NodeCapabilityExecutionRequest,
    output_key: &NodeCapabilityOutputKey,
    value: WorkflowRuntimeValue,
) -> Result<WorkflowNodeOutputSet, NodeCapabilityExecutionError> {
    WorkflowNodeOutputSet::try_new(contract, BTreeMap::from([(output_key.clone(), value)])).map_err(
        |_| {
            NodeCapabilityExecutionError::invalid_result_while_assembling_outputs(
                contract.contract_ref().clone(),
                request.context.node_execution_id,
                output_key.clone(),
            )
        },
    )
}

fn invalid_result_while_constructing_media_write(
    contract: &NodeCapabilityContract,
    request: &NodeCapabilityExecutionRequest,
    output_key: &NodeCapabilityOutputKey,
) -> NodeCapabilityExecutionError {
    NodeCapabilityExecutionError::invalid_result_while_constructing_media_write(
        contract.contract_ref().clone(),
        request.context.node_execution_id,
        output_key.clone(),
    )
}

fn media_write_interruption(
    contract: &NodeCapabilityContract,
    request: &NodeCapabilityExecutionRequest,
    output_key: &NodeCapabilityOutputKey,
) -> Option<NodeCapabilityExecutionError> {
    if request.context.cancellation.is_cancelled() {
        return Some(NodeCapabilityExecutionError::cancelled_while_writing_output(
            contract.contract_ref().clone(),
            request.context.node_execution_id,
            output_key.clone(),
        ));
    }
    if request.context.deadline.is_reached_at(Instant::now()) {
        return Some(NodeCapabilityExecutionError::deadline_exceeded_while_writing_output(
            contract.contract_ref().clone(),
            request.context.node_execution_id,
            output_key.clone(),
        ));
    }
    None
}

fn media_write_boundary_error(
    contract: &NodeCapabilityContract,
    request: &NodeCapabilityExecutionRequest,
    output_key: &NodeCapabilityOutputKey,
    error: NodeCapabilityMediaBoundaryError,
) -> NodeCapabilityExecutionError {
    match error {
        NodeCapabilityMediaBoundaryError::Cancelled => {
            NodeCapabilityExecutionError::cancelled_while_writing_output(
                contract.contract_ref().clone(),
                request.context.node_execution_id,
                output_key.clone(),
            )
        }
        NodeCapabilityMediaBoundaryError::DeadlineExceeded => {
            NodeCapabilityExecutionError::deadline_exceeded_while_writing_output(
                contract.contract_ref().clone(),
                request.context.node_execution_id,
                output_key.clone(),
            )
        }
        NodeCapabilityMediaBoundaryError::Media(failure) => {
            NodeCapabilityExecutionError::managed_media_output_write_failed(
                contract.contract_ref().clone(),
                request.context.node_execution_id,
                output_key.clone(),
                failure,
            )
        }
    }
}

fn produced_runtime_value(
    contract: &NodeCapabilityContract,
    request: &NodeCapabilityExecutionRequest,
    output_key: &NodeCapabilityOutputKey,
    expected_kind: NodeCapabilityMediaKind,
    expected_digest: crate::NodeCapabilityMediaContentDigest,
    reference: NodeCapabilityProducedMediaReference,
) -> Result<WorkflowRuntimeValue, NodeCapabilityExecutionError> {
    let observed_kind = reference.media_kind();
    let (fingerprint, value) = match reference {
        NodeCapabilityProducedMediaReference::Image(value) => {
            (value.content_fingerprint(), WorkflowRuntimeValue::Image(value))
        }
        NodeCapabilityProducedMediaReference::Video(value) => {
            (value.content_fingerprint(), WorkflowRuntimeValue::Video(value))
        }
        NodeCapabilityProducedMediaReference::Audio(value) => {
            (value.content_fingerprint(), WorkflowRuntimeValue::Audio(value))
        }
    };
    let failure = if observed_kind != expected_kind {
        Some(NodeCapabilityMediaFailure::KindMismatch {
            expected: expected_kind.to_workflow_data_type(),
            observed: observed_kind.to_workflow_data_type(),
        })
    } else if fingerprint.as_bytes() != expected_digest.as_bytes() {
        Some(NodeCapabilityMediaFailure::DigestMismatch)
    } else {
        None
    };
    match failure {
        Some(failure) => Err(NodeCapabilityExecutionError::managed_media_output_write_failed(
            contract.contract_ref().clone(),
            request.context.node_execution_id,
            output_key.clone(),
            failure,
        )),
        None => Ok(value),
    }
}
