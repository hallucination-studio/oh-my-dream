use std::time::Instant;

use engine::node_capability::*;

use crate::{
    GenerationProfileAvailabilityReaderInterface, GenerationProfileAvailabilityRequest,
    GenerationProfileAvailabilityState, GenerationProfileCatalog, GenerationProfileLifecycleState,
    GenerationProfileRef, NodeCapabilityGenerationTaskStartRequest,
    NodeCapabilityGenerationTaskStarterInterface,
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

pub(crate) fn resolve_inputs_interruption(
    contract: &NodeCapabilityContract,
    request: &NodeCapabilityExecutionRequest,
) -> Option<NodeCapabilityExecutionError> {
    if request.context.cancellation.is_cancelled() {
        return Some(NodeCapabilityExecutionError::cancelled_while_resolving_inputs(
            contract.contract_ref().clone(),
            request.context.node_execution_id,
        ));
    }
    if request.context.deadline.is_reached_at(Instant::now()) {
        return Some(NodeCapabilityExecutionError::deadline_exceeded_while_resolving_inputs(
            contract.contract_ref().clone(),
            request.context.node_execution_id,
        ));
    }
    None
}

pub(crate) async fn start_generation_task<S>(
    starter: &S,
    contract: &NodeCapabilityContract,
    execution_request: &NodeCapabilityExecutionRequest,
    start_request: NodeCapabilityGenerationTaskStartRequest,
) -> Result<WorkflowNodeCapabilityExecutionOutcome, NodeCapabilityExecutionError>
where
    S: NodeCapabilityGenerationTaskStarterInterface,
{
    if execution_request.context.cancellation.is_cancelled() {
        return Err(NodeCapabilityExecutionError::cancelled_while_starting_generation_task(
            contract.contract_ref().clone(),
            execution_request.context.node_execution_id,
        ));
    }
    if execution_request.context.deadline.is_reached_at(Instant::now()) {
        return Err(
            NodeCapabilityExecutionError::deadline_exceeded_while_starting_generation_task(
                contract.contract_ref().clone(),
                execution_request.context.node_execution_id,
            ),
        );
    }
    starter.start_generation_task(start_request).await.map_err(|failure| {
        NodeCapabilityExecutionError::generation_task_start_failed(
            contract.contract_ref().clone(),
            execution_request.context.node_execution_id,
            failure,
        )
    })?;
    Ok(WorkflowNodeCapabilityExecutionOutcome::WaitingForGenerationTask)
}
