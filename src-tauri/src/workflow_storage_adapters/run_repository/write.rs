use engine::{
    node_capability::WorkflowRunId,
    workflow::{
        WorkflowApplicationError, WorkflowRunAdmissionReceipt, WorkflowRunAggregate,
        WorkflowRunCommandHash, WorkflowRunEvent, WorkflowRunRequestId,
    },
};
use rusqlite::{Transaction, params};
use uuid::Uuid;

use super::super::{
    persistence,
    run::{encode_event_row, encode_node_execution, encode_run_core},
};

pub(super) fn insert_run(
    transaction: &Transaction<'_>,
    run: &WorkflowRunAggregate,
) -> Result<(), WorkflowApplicationError> {
    transaction
        .execute(
            "INSERT INTO workflow_runs(
                workflow_run_id, project_id, workflow_id, workflow_revision, run_payload,
                event_count, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                run.run_id().as_uuid().as_bytes().as_slice(),
                run.project_id().as_uuid().as_bytes().as_slice(),
                run.workflow_id().as_uuid().as_bytes().as_slice(),
                run.workflow_revision().get(),
                encode_run_core(run)?,
                run.events().len(),
                run.created_at().as_utc_milliseconds(),
                run.updated_at().as_utc_milliseconds(),
            ],
        )
        .map(|_| ())
        .map_err(|_| persistence())
}

pub(super) fn update_run(
    transaction: &Transaction<'_>,
    run: &WorkflowRunAggregate,
    expected_event_count: usize,
) -> Result<(), WorkflowApplicationError> {
    let changed = transaction
        .execute(
            "UPDATE workflow_runs
             SET run_payload = ?2, event_count = ?3, updated_at = ?4
             WHERE workflow_run_id = ?1 AND event_count = ?5",
            params![
                run.run_id().as_uuid().as_bytes().as_slice(),
                encode_run_core(run)?,
                run.events().len(),
                run.updated_at().as_utc_milliseconds(),
                expected_event_count,
            ],
        )
        .map_err(|_| persistence())?;
    if changed == 1 { Ok(()) } else { Err(persistence()) }
}

pub(super) fn replace_node_executions(
    transaction: &Transaction<'_>,
    run: &WorkflowRunAggregate,
) -> Result<(), WorkflowApplicationError> {
    transaction
        .execute(
            "DELETE FROM workflow_node_executions WHERE workflow_run_id = ?1",
            [run.run_id().as_uuid().as_bytes().as_slice()],
        )
        .map_err(|_| persistence())?;
    for node in run.node_executions() {
        transaction
            .execute(
                "INSERT INTO workflow_node_executions(
                    workflow_run_id, workflow_node_id, node_execution_id, state,
                    progress_basis_points, started_at, finished_at, node_payload
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    run.run_id().as_uuid().as_bytes().as_slice(),
                    node.node_id().as_uuid().as_bytes().as_slice(),
                    node.execution_id().as_uuid().as_bytes().as_slice(),
                    node_state(node.state()),
                    node.progress_basis_points(),
                    node.started_at().map(|value| value.as_utc_milliseconds()),
                    node.finished_at().map(|value| value.as_utc_milliseconds()),
                    encode_node_execution(node)?,
                ],
            )
            .map_err(|_| persistence())?;
        insert_outputs(transaction, run, node)?;
    }
    Ok(())
}

fn insert_outputs(
    transaction: &Transaction<'_>,
    run: &WorkflowRunAggregate,
    node: &engine::workflow::WorkflowNodeExecutionEntity,
) -> Result<(), WorkflowApplicationError> {
    let Some(outputs) = node.outputs() else {
        return Ok(());
    };
    for (key, value) in outputs.iter() {
        transaction
            .execute(
                "INSERT INTO workflow_node_execution_outputs(
                    workflow_run_id, node_execution_id, output_key, value_payload
                 ) VALUES (?1, ?2, ?3, ?4)",
                params![
                    run.run_id().as_uuid().as_bytes().as_slice(),
                    node.execution_id().as_uuid().as_bytes().as_slice(),
                    key.as_str(),
                    super::super::run::runtime_value::encode_value_bytes(value)?,
                ],
            )
            .map_err(|_| persistence())?;
    }
    Ok(())
}

pub(super) fn insert_events(
    transaction: &Transaction<'_>,
    events: &[WorkflowRunEvent],
) -> Result<(), WorkflowApplicationError> {
    for event in events {
        transaction
            .execute(
                "INSERT INTO workflow_run_events(
                    workflow_run_id, sequence, event_payload, occurred_at
                 ) VALUES (?1, ?2, ?3, ?4)",
                params![
                    event.run_id().as_uuid().as_bytes().as_slice(),
                    event.sequence().get(),
                    encode_event_row(event)?,
                    event.occurred_at().as_utc_milliseconds(),
                ],
            )
            .map_err(|_| persistence())?;
    }
    Ok(())
}

pub(super) fn decode_receipt(
    row: (Vec<u8>, Vec<u8>, Vec<u8>),
) -> Result<WorkflowRunAdmissionReceipt, WorkflowApplicationError> {
    let request_id = WorkflowRunRequestId::from_uuid(uuid(&row.0)?).ok_or_else(persistence)?;
    let command_hash =
        WorkflowRunCommandHash::from_bytes(row.1.try_into().map_err(|_| persistence())?);
    let workflow_run_id = workflow_run_id(&row.2)?;
    Ok(WorkflowRunAdmissionReceipt::restore(request_id, command_hash, workflow_run_id))
}

pub(super) fn uuid(bytes: &[u8]) -> Result<Uuid, WorkflowApplicationError> {
    Uuid::from_slice(bytes).map_err(|_| persistence())
}

pub(super) fn workflow_run_id(bytes: &[u8]) -> Result<WorkflowRunId, WorkflowApplicationError> {
    WorkflowRunId::from_uuid(uuid(bytes)?).ok_or_else(persistence)
}

fn node_state(value: engine::workflow::WorkflowNodeExecutionState) -> i64 {
    match value {
        engine::workflow::WorkflowNodeExecutionState::Pending => 0,
        engine::workflow::WorkflowNodeExecutionState::Running => 1,
        engine::workflow::WorkflowNodeExecutionState::Succeeded => 2,
        engine::workflow::WorkflowNodeExecutionState::Failed => 3,
        engine::workflow::WorkflowNodeExecutionState::Cancelled => 4,
        engine::workflow::WorkflowNodeExecutionState::Blocked => 5,
        engine::workflow::WorkflowNodeExecutionState::WaitingForExternalCompletion => 6,
    }
}
