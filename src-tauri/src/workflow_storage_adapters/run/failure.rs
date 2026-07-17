use engine::{
    node_capability::{
        NodeCapabilityContractId, NodeCapabilityContractRef, NodeCapabilityContractVersion,
        NodeCapabilityExecutionError, NodeCapabilityExecutionFailure, NodeCapabilityExecutionStage,
        NodeCapabilityExecutionTarget, NodeCapabilityInputKey, NodeCapabilityOutputKey,
        NodeCapabilityParameterKey, NodeCapabilityProviderFailure, WorkflowNodeExecutionId,
    },
    workflow::{
        WorkflowApplicationError, WorkflowNodeExecutionBlockReason, WorkflowNodeExecutionFailure,
        WorkflowRunEventPayload, WorkflowRunFailure,
    },
    workflow_graph::WorkflowNodeId,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

mod detail;
#[cfg(test)]
mod tests;

use super::{
    super::persistence,
    runtime_value::{OutputSetPayload, decode_output_set, encode_output_set},
};
use detail::*;

#[derive(Serialize, Deserialize)]
pub(super) enum FailurePayload {
    NodeExecutionFailed { sorted_failed_node_ids: Vec<Uuid> },
    InterruptedByRestart,
}

#[derive(Serialize, Deserialize)]
pub(super) enum BlockReasonPayload {
    UpstreamNodeFailed { sorted_upstream_node_ids: Vec<Uuid> },
}

#[derive(Serialize, Deserialize)]
pub(super) struct ExecutionFailurePayload {
    contract_id: String,
    contract_major: u16,
    contract_minor: u16,
    execution_id: Uuid,
    stage: StagePayload,
    failure: ExecutionSourcePayload,
    target: TargetPayload,
}

#[derive(Serialize, Deserialize)]
pub(super) enum EventKindPayload {
    RunQueued,
    RunStarted,
    NodeStarted { execution_id: Uuid },
    NodeProgressed { execution_id: Uuid, progress_basis_points: u16 },
    NodeWaitingForExternalCompletion { execution_id: Uuid },
    NodeSucceeded { execution_id: Uuid, outputs: OutputSetPayload },
    NodeFailed { execution_id: Uuid, failure: ExecutionFailurePayload },
    NodeBlocked { execution_id: Uuid, reason: BlockReasonPayload },
    NodeCancelled { execution_id: Uuid },
    RunSucceeded,
    RunFailed { failure: FailurePayload },
    RunCancelled,
}

pub(super) fn encode_run_failure(value: &WorkflowRunFailure) -> FailurePayload {
    match value {
        WorkflowRunFailure::NodeExecutionFailed { sorted_failed_node_ids } => {
            FailurePayload::NodeExecutionFailed {
                sorted_failed_node_ids: sorted_failed_node_ids
                    .iter()
                    .map(|id| id.as_uuid())
                    .collect(),
            }
        }
        WorkflowRunFailure::InterruptedByRestart => FailurePayload::InterruptedByRestart,
    }
}

pub(super) fn decode_run_failure(
    value: FailurePayload,
) -> Result<WorkflowRunFailure, WorkflowApplicationError> {
    match value {
        FailurePayload::NodeExecutionFailed { sorted_failed_node_ids } => {
            Ok(WorkflowRunFailure::NodeExecutionFailed {
                sorted_failed_node_ids: sorted_failed_node_ids
                    .into_iter()
                    .map(WorkflowNodeId::from_uuid)
                    .collect::<Result<Vec<_>, _>>()?,
            })
        }
        FailurePayload::InterruptedByRestart => Ok(WorkflowRunFailure::InterruptedByRestart),
    }
}

pub(super) fn encode_block_reason(value: &WorkflowNodeExecutionBlockReason) -> BlockReasonPayload {
    match value {
        WorkflowNodeExecutionBlockReason::UpstreamNodeFailed { sorted_upstream_node_ids } => {
            BlockReasonPayload::UpstreamNodeFailed {
                sorted_upstream_node_ids: sorted_upstream_node_ids
                    .iter()
                    .map(|id| id.as_uuid())
                    .collect(),
            }
        }
    }
}

pub(super) fn decode_block_reason(
    value: BlockReasonPayload,
) -> Result<WorkflowNodeExecutionBlockReason, WorkflowApplicationError> {
    match value {
        BlockReasonPayload::UpstreamNodeFailed { sorted_upstream_node_ids } => {
            Ok(WorkflowNodeExecutionBlockReason::UpstreamNodeFailed {
                sorted_upstream_node_ids: sorted_upstream_node_ids
                    .into_iter()
                    .map(WorkflowNodeId::from_uuid)
                    .collect::<Result<Vec<_>, _>>()?,
            })
        }
    }
}

pub(super) fn encode_execution_failure(
    value: &WorkflowNodeExecutionFailure,
) -> ExecutionFailurePayload {
    let error = &value.capability_error;
    ExecutionFailurePayload {
        contract_id: error.contract_ref().id().as_str().to_owned(),
        contract_major: error.contract_ref().version().major(),
        contract_minor: error.contract_ref().version().minor(),
        execution_id: error.node_execution_id().as_uuid(),
        stage: encode_stage(error.stage()),
        failure: encode_source(error.failure()),
        target: encode_target(error.target()),
    }
}

pub(super) fn decode_execution_failure(
    value: ExecutionFailurePayload,
) -> Result<WorkflowNodeExecutionFailure, WorkflowApplicationError> {
    let error = NodeCapabilityExecutionError::try_new(
        NodeCapabilityContractRef::new(
            NodeCapabilityContractId::new(value.contract_id).map_err(|_| persistence())?,
            NodeCapabilityContractVersion::new(value.contract_major, value.contract_minor)
                .map_err(|_| persistence())?,
        ),
        WorkflowNodeExecutionId::from_uuid(value.execution_id).ok_or_else(persistence)?,
        decode_stage(value.stage),
        decode_source(value.failure)?,
        decode_target(value.target)?,
    )
    .map_err(|_| persistence())?;
    Ok(WorkflowNodeExecutionFailure { capability_error: error })
}

pub(super) fn encode_event(value: &WorkflowRunEventPayload) -> EventKindPayload {
    match value {
        WorkflowRunEventPayload::WorkflowRunQueuedEvent => EventKindPayload::RunQueued,
        WorkflowRunEventPayload::WorkflowRunStartedEvent => EventKindPayload::RunStarted,
        WorkflowRunEventPayload::WorkflowNodeStartedEvent { node_execution_id } => {
            EventKindPayload::NodeStarted { execution_id: node_execution_id.as_uuid() }
        }
        WorkflowRunEventPayload::WorkflowNodeProgressedEvent {
            node_execution_id,
            progress_basis_points,
        } => EventKindPayload::NodeProgressed {
            execution_id: node_execution_id.as_uuid(),
            progress_basis_points: *progress_basis_points,
        },
        WorkflowRunEventPayload::WorkflowNodeWaitingForExternalCompletionEvent {
            node_execution_id,
        } => EventKindPayload::NodeWaitingForExternalCompletion {
            execution_id: node_execution_id.as_uuid(),
        },
        WorkflowRunEventPayload::WorkflowNodeSucceededEvent { node_execution_id, outputs } => {
            EventKindPayload::NodeSucceeded {
                execution_id: node_execution_id.as_uuid(),
                outputs: encode_output_set(outputs),
            }
        }
        WorkflowRunEventPayload::WorkflowNodeFailedEvent { node_execution_id, failure } => {
            EventKindPayload::NodeFailed {
                execution_id: node_execution_id.as_uuid(),
                failure: encode_execution_failure(failure),
            }
        }
        WorkflowRunEventPayload::WorkflowNodeBlockedEvent { node_execution_id, reason } => {
            EventKindPayload::NodeBlocked {
                execution_id: node_execution_id.as_uuid(),
                reason: encode_block_reason(reason),
            }
        }
        WorkflowRunEventPayload::WorkflowNodeCancelledEvent { node_execution_id } => {
            EventKindPayload::NodeCancelled { execution_id: node_execution_id.as_uuid() }
        }
        WorkflowRunEventPayload::WorkflowRunSucceededEvent => EventKindPayload::RunSucceeded,
        WorkflowRunEventPayload::WorkflowRunFailedEvent { failure } => {
            EventKindPayload::RunFailed { failure: encode_run_failure(failure) }
        }
        WorkflowRunEventPayload::WorkflowRunCancelledEvent => EventKindPayload::RunCancelled,
    }
}

pub(super) fn decode_event(
    value: EventKindPayload,
) -> Result<WorkflowRunEventPayload, WorkflowApplicationError> {
    match value {
        EventKindPayload::RunQueued => Ok(WorkflowRunEventPayload::WorkflowRunQueuedEvent),
        EventKindPayload::RunStarted => Ok(WorkflowRunEventPayload::WorkflowRunStartedEvent),
        EventKindPayload::NodeStarted { execution_id } => {
            Ok(WorkflowRunEventPayload::WorkflowNodeStartedEvent {
                node_execution_id: execution_id_from(execution_id)?,
            })
        }
        EventKindPayload::NodeProgressed { execution_id, progress_basis_points } => {
            Ok(WorkflowRunEventPayload::WorkflowNodeProgressedEvent {
                node_execution_id: execution_id_from(execution_id)?,
                progress_basis_points,
            })
        }
        EventKindPayload::NodeWaitingForExternalCompletion { execution_id } => {
            Ok(WorkflowRunEventPayload::WorkflowNodeWaitingForExternalCompletionEvent {
                node_execution_id: execution_id_from(execution_id)?,
            })
        }
        EventKindPayload::NodeSucceeded { execution_id, outputs } => {
            Ok(WorkflowRunEventPayload::WorkflowNodeSucceededEvent {
                node_execution_id: execution_id_from(execution_id)?,
                outputs: decode_output_set(outputs)?,
            })
        }
        EventKindPayload::NodeFailed { execution_id, failure } => {
            Ok(WorkflowRunEventPayload::WorkflowNodeFailedEvent {
                node_execution_id: execution_id_from(execution_id)?,
                failure: decode_execution_failure(failure)?,
            })
        }
        EventKindPayload::NodeBlocked { execution_id, reason } => {
            Ok(WorkflowRunEventPayload::WorkflowNodeBlockedEvent {
                node_execution_id: execution_id_from(execution_id)?,
                reason: decode_block_reason(reason)?,
            })
        }
        EventKindPayload::NodeCancelled { execution_id } => {
            Ok(WorkflowRunEventPayload::WorkflowNodeCancelledEvent {
                node_execution_id: execution_id_from(execution_id)?,
            })
        }
        EventKindPayload::RunSucceeded => Ok(WorkflowRunEventPayload::WorkflowRunSucceededEvent),
        EventKindPayload::RunFailed { failure } => {
            Ok(WorkflowRunEventPayload::WorkflowRunFailedEvent {
                failure: decode_run_failure(failure)?,
            })
        }
        EventKindPayload::RunCancelled => Ok(WorkflowRunEventPayload::WorkflowRunCancelledEvent),
    }
}

fn encode_source(value: &NodeCapabilityExecutionFailure) -> ExecutionSourcePayload {
    match value {
        NodeCapabilityExecutionFailure::InvalidCapabilityInvocation => {
            ExecutionSourcePayload::InvalidCapabilityInvocation
        }
        NodeCapabilityExecutionFailure::InvalidCapabilityResult => {
            ExecutionSourcePayload::InvalidCapabilityResult
        }
        NodeCapabilityExecutionFailure::Readiness(value) => {
            ExecutionSourcePayload::Readiness(encode_readiness(value))
        }
        NodeCapabilityExecutionFailure::Provider(value) => ExecutionSourcePayload::Provider {
            category: encode_provider_category(value.category()),
            retryable: value.is_retryable(),
            safe_retry_after_nanos: value.safe_retry_at().and_then(|retry_at| {
                u64::try_from(
                    retry_at.saturating_duration_since(std::time::Instant::now()).as_nanos(),
                )
                .ok()
            }),
        },
        NodeCapabilityExecutionFailure::Media(value) => {
            ExecutionSourcePayload::Media(encode_media_failure(*value))
        }
        NodeCapabilityExecutionFailure::Cancelled => ExecutionSourcePayload::Cancelled,
        NodeCapabilityExecutionFailure::DeadlineExceeded => {
            ExecutionSourcePayload::DeadlineExceeded
        }
    }
}

fn decode_source(
    value: ExecutionSourcePayload,
) -> Result<NodeCapabilityExecutionFailure, WorkflowApplicationError> {
    match value {
        ExecutionSourcePayload::InvalidCapabilityInvocation => {
            Ok(NodeCapabilityExecutionFailure::InvalidCapabilityInvocation)
        }
        ExecutionSourcePayload::InvalidCapabilityResult => {
            Ok(NodeCapabilityExecutionFailure::InvalidCapabilityResult)
        }
        ExecutionSourcePayload::Readiness(value) => {
            decode_readiness(value).map(NodeCapabilityExecutionFailure::Readiness)
        }
        ExecutionSourcePayload::Provider { category, retryable, safe_retry_after_nanos } => {
            NodeCapabilityProviderFailure::try_restore_with_retry_after(
                decode_provider_category(category),
                retryable,
                safe_retry_after_nanos.map(std::time::Duration::from_nanos),
            )
            .map(NodeCapabilityExecutionFailure::Provider)
            .map_err(|_| persistence())
        }
        ExecutionSourcePayload::Media(value) => {
            Ok(NodeCapabilityExecutionFailure::Media(decode_media_failure(value)))
        }
        ExecutionSourcePayload::Cancelled => Ok(NodeCapabilityExecutionFailure::Cancelled),
        ExecutionSourcePayload::DeadlineExceeded => {
            Ok(NodeCapabilityExecutionFailure::DeadlineExceeded)
        }
    }
}

fn encode_stage(value: NodeCapabilityExecutionStage) -> StagePayload {
    match value {
        NodeCapabilityExecutionStage::ResolveInputs => StagePayload::ResolveInputs,
        NodeCapabilityExecutionStage::CallProvider => StagePayload::CallProvider,
        NodeCapabilityExecutionStage::ValidateProviderResult => {
            StagePayload::ValidateProviderResult
        }
        NodeCapabilityExecutionStage::WriteManagedMedia => StagePayload::WriteManagedMedia,
        NodeCapabilityExecutionStage::AssembleOutputs => StagePayload::AssembleOutputs,
    }
}

fn decode_stage(value: StagePayload) -> NodeCapabilityExecutionStage {
    match value {
        StagePayload::ResolveInputs => NodeCapabilityExecutionStage::ResolveInputs,
        StagePayload::CallProvider => NodeCapabilityExecutionStage::CallProvider,
        StagePayload::ValidateProviderResult => {
            NodeCapabilityExecutionStage::ValidateProviderResult
        }
        StagePayload::WriteManagedMedia => NodeCapabilityExecutionStage::WriteManagedMedia,
        StagePayload::AssembleOutputs => NodeCapabilityExecutionStage::AssembleOutputs,
    }
}

fn encode_target(value: &NodeCapabilityExecutionTarget) -> TargetPayload {
    match value {
        NodeCapabilityExecutionTarget::Capability => TargetPayload::Capability,
        NodeCapabilityExecutionTarget::Parameter(key) => {
            TargetPayload::Parameter(key.as_str().to_owned())
        }
        NodeCapabilityExecutionTarget::Input(key) => TargetPayload::Input(key.as_str().to_owned()),
        NodeCapabilityExecutionTarget::Output(key) => {
            TargetPayload::Output(key.as_str().to_owned())
        }
    }
}

fn decode_target(
    value: TargetPayload,
) -> Result<NodeCapabilityExecutionTarget, WorkflowApplicationError> {
    match value {
        TargetPayload::Capability => Ok(NodeCapabilityExecutionTarget::Capability),
        TargetPayload::Parameter(key) => NodeCapabilityParameterKey::new(key)
            .map(NodeCapabilityExecutionTarget::Parameter)
            .map_err(|_| persistence()),
        TargetPayload::Input(key) => NodeCapabilityInputKey::new(key)
            .map(NodeCapabilityExecutionTarget::Input)
            .map_err(|_| persistence()),
        TargetPayload::Output(key) => NodeCapabilityOutputKey::new(key)
            .map(NodeCapabilityExecutionTarget::Output)
            .map_err(|_| persistence()),
    }
}

fn execution_id_from(value: Uuid) -> Result<WorkflowNodeExecutionId, WorkflowApplicationError> {
    WorkflowNodeExecutionId::from_uuid(value).ok_or_else(persistence)
}
