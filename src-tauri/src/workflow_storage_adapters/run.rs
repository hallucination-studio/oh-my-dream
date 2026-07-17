mod failure;
mod plan;
pub(super) mod runtime_value;

use engine::{
    node_capability::{WorkflowNodeExecutionId, WorkflowRunId},
    workflow::{
        WorkflowApplicationError, WorkflowNodeExecutionRestoreData, WorkflowNodeExecutionState,
        WorkflowRunEvent, WorkflowRunFailure, WorkflowRunState, WorkflowRunTime,
    },
    workflow_graph::WorkflowNodeId,
};
use serde::{Deserialize, Serialize};

use super::persistence;
use failure::{
    BlockReasonPayload, ExecutionFailurePayload, FailurePayload, decode_block_reason,
    decode_execution_failure, decode_run_failure, encode_block_reason, encode_execution_failure,
    encode_run_failure,
};
use plan::{PlanPayload, decode_plan, encode_plan};

pub(super) struct DecodedRunCore {
    pub plan: engine::workflow::WorkflowExecutionPlan,
    pub state: WorkflowRunState,
    pub failure: Option<WorkflowRunFailure>,
}

pub(super) struct NodeExecutionRowScalars {
    pub node_id: WorkflowNodeId,
    pub execution_id: WorkflowNodeExecutionId,
    pub state: WorkflowNodeExecutionState,
    pub progress_basis_points: Option<u16>,
    pub started_at: Option<i64>,
    pub finished_at: Option<i64>,
}

#[derive(Serialize, Deserialize)]
struct RunCorePayload {
    plan: PlanPayload,
    state: RunStatePayload,
    failure: Option<FailurePayload>,
}

#[derive(Serialize, Deserialize)]
struct NodeExecutionPayload {
    failure: Option<ExecutionFailurePayload>,
    block_reason: Option<BlockReasonPayload>,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
enum RunStatePayload {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Serialize, Deserialize)]
struct EventPayload {
    payload: failure::EventKindPayload,
}

pub(super) fn encode_run_core(
    run: &engine::workflow::WorkflowRunAggregate,
) -> Result<Vec<u8>, WorkflowApplicationError> {
    let payload = RunCorePayload {
        plan: encode_plan(run.plan()),
        state: encode_run_state(run.state()),
        failure: run.failure().map(encode_run_failure),
    };
    bounded_json(&payload, 4 * 1_048_576)
}

pub(super) fn decode_run_core(bytes: &[u8]) -> Result<DecodedRunCore, WorkflowApplicationError> {
    let payload: RunCorePayload = decode_bounded_json(bytes, 4 * 1_048_576)?;
    Ok(DecodedRunCore {
        plan: decode_plan(payload.plan)?,
        state: decode_run_state(payload.state),
        failure: payload.failure.map(decode_run_failure).transpose()?,
    })
}

pub(super) fn encode_node_execution(
    node: &engine::workflow::WorkflowNodeExecutionEntity,
) -> Result<Vec<u8>, WorkflowApplicationError> {
    bounded_json(
        &NodeExecutionPayload {
            failure: node.failure().map(encode_execution_failure),
            block_reason: node.block_reason().map(encode_block_reason),
        },
        2 * 1_048_576,
    )
}

pub(super) fn decode_node_execution(
    scalars: NodeExecutionRowScalars,
    outputs: Option<engine::node_capability::WorkflowNodeOutputSet>,
    bytes: &[u8],
) -> Result<WorkflowNodeExecutionRestoreData, WorkflowApplicationError> {
    let node: NodeExecutionPayload = decode_bounded_json(bytes, 2 * 1_048_576)?;
    Ok(WorkflowNodeExecutionRestoreData {
        node_id: scalars.node_id,
        execution_id: scalars.execution_id,
        state: scalars.state,
        progress_basis_points: scalars.progress_basis_points,
        started_at: scalars.started_at.map(time).transpose()?,
        finished_at: scalars.finished_at.map(time).transpose()?,
        outputs,
        failure: node.failure.map(decode_execution_failure).transpose()?,
        block_reason: node.block_reason.map(decode_block_reason).transpose()?,
    })
}

pub(super) fn encode_event_row(
    event: &WorkflowRunEvent,
) -> Result<Vec<u8>, WorkflowApplicationError> {
    bounded_json(&EventPayload { payload: failure::encode_event(event.payload()) }, 1_048_576)
}

pub(super) fn decode_event_row(
    run_id: WorkflowRunId,
    sequence: u64,
    occurred_at: i64,
    bytes: &[u8],
) -> Result<WorkflowRunEvent, WorkflowApplicationError> {
    let event: EventPayload = decode_bounded_json(bytes, 1_048_576)?;
    Ok(WorkflowRunEvent::restore(
        run_id,
        engine::workflow::WorkflowRunEventSequence::new(sequence)?,
        time(occurred_at)?,
        failure::decode_event(event.payload)?,
    ))
}

fn bounded_json(
    value: &impl Serialize,
    maximum: usize,
) -> Result<Vec<u8>, WorkflowApplicationError> {
    let bytes = serde_json::to_vec(value).map_err(|_| persistence())?;
    if bytes.len() <= maximum { Ok(bytes) } else { Err(persistence()) }
}

fn decode_bounded_json<T: for<'de> Deserialize<'de>>(
    bytes: &[u8],
    maximum: usize,
) -> Result<T, WorkflowApplicationError> {
    if bytes.len() > maximum {
        return Err(persistence());
    }
    serde_json::from_slice(bytes).map_err(|_| persistence())
}

fn time(value: i64) -> Result<WorkflowRunTime, WorkflowApplicationError> {
    WorkflowRunTime::from_utc_milliseconds(value).map_err(Into::into)
}

fn encode_run_state(value: WorkflowRunState) -> RunStatePayload {
    match value {
        WorkflowRunState::Queued => RunStatePayload::Queued,
        WorkflowRunState::Running => RunStatePayload::Running,
        WorkflowRunState::Succeeded => RunStatePayload::Succeeded,
        WorkflowRunState::Failed => RunStatePayload::Failed,
        WorkflowRunState::Cancelled => RunStatePayload::Cancelled,
    }
}

fn decode_run_state(value: RunStatePayload) -> WorkflowRunState {
    match value {
        RunStatePayload::Queued => WorkflowRunState::Queued,
        RunStatePayload::Running => WorkflowRunState::Running,
        RunStatePayload::Succeeded => WorkflowRunState::Succeeded,
        RunStatePayload::Failed => WorkflowRunState::Failed,
        RunStatePayload::Cancelled => WorkflowRunState::Cancelled,
    }
}

pub(super) fn decode_node_state_row(
    value: i64,
) -> Result<WorkflowNodeExecutionState, WorkflowApplicationError> {
    match value {
        0 => Ok(WorkflowNodeExecutionState::Pending),
        1 => Ok(WorkflowNodeExecutionState::Running),
        2 => Ok(WorkflowNodeExecutionState::Succeeded),
        3 => Ok(WorkflowNodeExecutionState::Failed),
        4 => Ok(WorkflowNodeExecutionState::Cancelled),
        5 => Ok(WorkflowNodeExecutionState::Blocked),
        _ => Err(persistence()),
    }
}
