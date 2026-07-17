use std::collections::BTreeMap;
use std::time::Instant;

use async_trait::async_trait;
use engine::node_capability::*;

use crate::{
    NodeCapabilityAssetIdMediaReadSelection, NodeCapabilityManagedMediaReadRequest,
    NodeCapabilityManagedMediaReadSelection, NodeCapabilityManagedMediaReaderInterface,
    NodeCapabilityMediaBoundaryError, NodeCapabilityMediaKind, NodeCapabilityReadableMediaInput,
};

const ASSET_ID_PARAMETER: &str = "asset_id";

macro_rules! asset_read_capability {
    ($name:ident, $contract_id:literal, $output_key:literal, $media_variant:ident) => {
        #[doc = concat!("Reads one Available managed ", stringify!($media_variant), " Asset.")]
        pub struct $name<R> {
            managed_media_reader: R,
            contract: NodeCapabilityContract,
            output_key: NodeCapabilityOutputKey,
        }

        impl<R> $name<R> {
            #[doc = concat!("Builds the frozen `", $contract_id, "@1.0` capability.")]
            pub fn try_new(managed_media_reader: R) -> Result<Self, NodeCapabilityContractError> {
                let output_key = NodeCapabilityOutputKey::new($output_key)?;
                Ok(Self {
                    managed_media_reader,
                    contract: asset_read_contract(
                        $contract_id,
                        output_key.clone(),
                        NodeCapabilityMediaKind::$media_variant,
                    )?,
                    output_key,
                })
            }
        }

        #[async_trait]
        impl<R: NodeCapabilityManagedMediaReaderInterface> WorkflowNodeCapabilityInterface
            for $name<R>
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
                let Some(asset_id) = normalized_asset_id(&request.normalized_parameters) else {
                    return vec![NodeCapabilityReadinessIssue::invalid_capability_invocation()];
                };
                let result = self
                    .managed_media_reader
                    .read_managed_media(NodeCapabilityManagedMediaReadRequest::new(
                        request.project_id,
                        NodeCapabilityManagedMediaReadSelection::AssetId(
                            NodeCapabilityAssetIdMediaReadSelection::new(
                                asset_id,
                                NodeCapabilityMediaKind::$media_variant,
                            ),
                        ),
                        request.deadline.monotonic_instant(),
                    ))
                    .await;
                readiness_issues_for_asset_read(result, asset_id)
            }

            async fn execute_node_capability(
                &self,
                request: NodeCapabilityExecutionRequest,
            ) -> Result<WorkflowNodeCapabilityExecutionOutcome, NodeCapabilityExecutionError> {
                execute_asset_read(
                    &self.managed_media_reader,
                    &self.contract,
                    request,
                    NodeCapabilityMediaKind::$media_variant,
                    &self.output_key,
                    |value| match value {
                        NodeCapabilityReadableMediaInput::$media_variant(value) => {
                            Some(WorkflowRuntimeValue::$media_variant(value.media_reference()))
                        }
                        _ => None,
                    },
                )
                .await
                .map(WorkflowNodeCapabilityExecutionOutcome::Completed)
            }
        }
    };
}

asset_read_capability!(ReadImageAssetCapabilityImpl, "image.read_asset", "image", Image);
asset_read_capability!(ReadVideoAssetCapabilityImpl, "video.read_asset", "video", Video);
asset_read_capability!(ReadAudioAssetCapabilityImpl, "audio.read_asset", "audio", Audio);

async fn execute_asset_read<R: NodeCapabilityManagedMediaReaderInterface>(
    reader: &R,
    contract: &NodeCapabilityContract,
    request: NodeCapabilityExecutionRequest,
    expected_kind: NodeCapabilityMediaKind,
    output_key: &NodeCapabilityOutputKey,
    into_runtime_value: fn(NodeCapabilityReadableMediaInput) -> Option<WorkflowRuntimeValue>,
) -> Result<WorkflowNodeOutputSet, NodeCapabilityExecutionError> {
    let Some(asset_id) =
        normalized_asset_id(&request.normalized_parameters).filter(|_| request.inputs.is_empty())
    else {
        return Err(invalid_invocation(contract, &request));
    };
    if request.origin.capability_contract_ref() != contract.contract_ref() {
        return Err(invalid_invocation(contract, &request));
    }
    if let Some(error) = cancelled_or_elapsed_error(contract, &request) {
        return Err(error);
    }
    let result = reader
        .read_managed_media(NodeCapabilityManagedMediaReadRequest::new(
            request.context.project_id,
            NodeCapabilityManagedMediaReadSelection::AssetId(
                NodeCapabilityAssetIdMediaReadSelection::new(asset_id, expected_kind),
            ),
            request.context.deadline.monotonic_instant(),
        ))
        .await;
    if let Some(error) = cancelled_or_elapsed_error(contract, &request) {
        return Err(error);
    }
    let runtime_value = match result {
        Ok(readable) => {
            let observed_kind = readable.media_kind();
            match into_runtime_value(readable) {
                Some(value) => value,
                None => {
                    return Err(kind_mismatch_execution_error(
                        contract,
                        &request,
                        expected_kind,
                        observed_kind,
                    ));
                }
            }
        }
        Err(error) => return Err(media_boundary_execution_error(contract, &request, error)),
    };
    single_output(contract, output_key, runtime_value).map_err(|_| {
        NodeCapabilityExecutionError::invalid_result_while_assembling_outputs(
            contract.contract_ref().clone(),
            request.context.node_execution_id,
            output_key.clone(),
        )
    })
}
fn asset_read_contract(
    id: &str,
    output_key: NodeCapabilityOutputKey,
    media_kind: NodeCapabilityMediaKind,
) -> Result<NodeCapabilityContract, NodeCapabilityContractError> {
    NodeCapabilityContract::try_new(
        contract_ref(id)?,
        vec![NodeCapabilityParameterContract::required(
            NodeCapabilityParameterKey::new(ASSET_ID_PARAMETER)?,
            NodeCapabilityParameterConstraint::managed_asset_id(
                media_kind.to_workflow_data_type(),
            )?,
        )],
        Vec::new(),
        vec![NodeCapabilityOutputContract::new(
            output_key,
            media_kind.to_workflow_data_type(),
            true,
        )],
        NodeCapabilityExecutionKind::ManagedAssetRead,
    )
}

fn contract_ref(id: &str) -> Result<NodeCapabilityContractRef, NodeCapabilityContractError> {
    Ok(NodeCapabilityContractRef::new(
        NodeCapabilityContractId::new(id)?,
        NodeCapabilityContractVersion::new(1, 0)?,
    ))
}

fn normalized_asset_id(
    parameters: &NodeCapabilityNormalizedParameters,
) -> Option<WorkflowManagedAssetIdBoundaryValue> {
    match parameters.get(&NodeCapabilityParameterKey::new(ASSET_ID_PARAMETER).ok()?)? {
        NodeCapabilityParameterValue::ManagedAsset(value) if parameters.len() == 1 => {
            Some(value.asset_id())
        }
        _ => None,
    }
}

fn single_output(
    contract: &NodeCapabilityContract,
    key: &NodeCapabilityOutputKey,
    value: WorkflowRuntimeValue,
) -> Result<WorkflowNodeOutputSet, WorkflowRuntimeValueError> {
    let mut values = BTreeMap::new();
    values.insert(key.clone(), value);
    WorkflowNodeOutputSet::try_new(contract, values)
}

fn readiness_issues_for_asset_read(
    result: Result<NodeCapabilityReadableMediaInput, NodeCapabilityMediaBoundaryError>,
    asset_id: WorkflowManagedAssetIdBoundaryValue,
) -> Vec<NodeCapabilityReadinessIssue> {
    let parameter_key = match NodeCapabilityParameterKey::new(ASSET_ID_PARAMETER) {
        Ok(value) => value,
        Err(_) => return vec![NodeCapabilityReadinessIssue::invalid_capability_invocation()],
    };
    match result {
        Ok(_) => Vec::new(),
        Err(NodeCapabilityMediaBoundaryError::Media(NodeCapabilityMediaFailure::Unavailable)) => {
            vec![NodeCapabilityReadinessIssue::managed_asset_unavailable(parameter_key, asset_id)]
        }
        Err(NodeCapabilityMediaBoundaryError::Media(
            NodeCapabilityMediaFailure::KindMismatch { expected, observed },
        )) => {
            match NodeCapabilityReadinessIssue::managed_asset_kind_mismatch(
                parameter_key.clone(),
                asset_id,
                expected,
                observed,
            ) {
                Ok(issue) => vec![issue],
                Err(_) => {
                    vec![NodeCapabilityReadinessIssue::managed_asset_readiness_indeterminate(
                        parameter_key,
                        asset_id,
                    )]
                }
            }
        }
        Err(_) => vec![NodeCapabilityReadinessIssue::managed_asset_readiness_indeterminate(
            parameter_key,
            asset_id,
        )],
    }
}

fn cancelled_or_elapsed_error(
    contract: &NodeCapabilityContract,
    request: &NodeCapabilityExecutionRequest,
) -> Option<NodeCapabilityExecutionError> {
    let parameter_key = contract.parameters().first()?.key().clone();
    if request.context.cancellation.is_cancelled() {
        return Some(NodeCapabilityExecutionError::cancelled_while_resolving_parameter(
            contract.contract_ref().clone(),
            request.context.node_execution_id,
            parameter_key,
        ));
    }
    if request.context.deadline.is_reached_at(Instant::now()) {
        return Some(NodeCapabilityExecutionError::deadline_exceeded_while_resolving_parameter(
            contract.contract_ref().clone(),
            request.context.node_execution_id,
            parameter_key,
        ));
    }
    None
}

fn invalid_invocation(
    contract: &NodeCapabilityContract,
    request: &NodeCapabilityExecutionRequest,
) -> NodeCapabilityExecutionError {
    NodeCapabilityExecutionError::invalid_capability_invocation(
        contract.contract_ref().clone(),
        request.context.node_execution_id,
    )
}

fn media_boundary_execution_error(
    contract: &NodeCapabilityContract,
    request: &NodeCapabilityExecutionRequest,
    error: NodeCapabilityMediaBoundaryError,
) -> NodeCapabilityExecutionError {
    match error {
        NodeCapabilityMediaBoundaryError::Cancelled => match contract.parameters().first() {
            Some(parameter) => NodeCapabilityExecutionError::cancelled_while_resolving_parameter(
                contract.contract_ref().clone(),
                request.context.node_execution_id,
                parameter.key().clone(),
            ),
            None => invalid_invocation(contract, request),
        },
        NodeCapabilityMediaBoundaryError::DeadlineExceeded => match contract.parameters().first() {
            Some(parameter) => {
                NodeCapabilityExecutionError::deadline_exceeded_while_resolving_parameter(
                    contract.contract_ref().clone(),
                    request.context.node_execution_id,
                    parameter.key().clone(),
                )
            }
            None => invalid_invocation(contract, request),
        },
        NodeCapabilityMediaBoundaryError::Media(failure) => {
            let Some(parameter_key) =
                contract.parameters().first().map(|value| value.key().clone())
            else {
                return invalid_invocation(contract, request);
            };
            NodeCapabilityExecutionError::managed_media_parameter_resolution_failed(
                contract.contract_ref().clone(),
                request.context.node_execution_id,
                parameter_key,
                failure,
            )
        }
    }
}

fn kind_mismatch_execution_error(
    contract: &NodeCapabilityContract,
    request: &NodeCapabilityExecutionRequest,
    expected: NodeCapabilityMediaKind,
    observed: NodeCapabilityMediaKind,
) -> NodeCapabilityExecutionError {
    let Some(parameter_key) = contract.parameters().first().map(|value| value.key().clone()) else {
        return invalid_invocation(contract, request);
    };
    NodeCapabilityExecutionError::managed_media_parameter_resolution_failed(
        contract.contract_ref().clone(),
        request.context.node_execution_id,
        parameter_key,
        NodeCapabilityMediaFailure::KindMismatch {
            expected: expected.to_workflow_data_type(),
            observed: observed.to_workflow_data_type(),
        },
    )
}
