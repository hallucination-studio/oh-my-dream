mod delivery;
mod write;

use std::collections::BTreeMap;

use engine::{
    node_capability::WorkflowRunId,
    workflow::{
        WorkflowApplicationError, WorkflowRunAdmissionCommit, WorkflowRunAdmissionReceipt,
        WorkflowRunAggregate, WorkflowRunEvent, WorkflowRunEventSequence, WorkflowRunLoadKey,
        WorkflowRunRequestId, WorkflowRunRestoreData, WorkflowRunTime,
    },
    workflow_graph::{WorkflowId, WorkflowNodeId},
};
use projects::project::domain::ProjectId;
use rusqlite::{Connection, OptionalExtension, params};

use super::{
    persistence,
    run::{
        NodeExecutionRowScalars, decode_event_row, decode_node_execution, decode_node_state_row,
        decode_run_core,
    },
};
use crate::post_commit_effect::{
    DesktopPostCommitEffect, DesktopPostCommitEffectId, DesktopPostCommitTimestamp,
    insert_ready_post_commit_effect,
};
pub(super) use delivery::*;
use write::*;

pub(super) fn load_run(
    connection: &Connection,
    key: WorkflowRunLoadKey,
) -> Result<Option<WorkflowRunAggregate>, WorkflowApplicationError> {
    let (sql, run_id, project_id) = match key {
        WorkflowRunLoadKey::Run(run_id) => (
            "SELECT workflow_run_id FROM workflow_runs
             WHERE workflow_run_id = ?1 AND ?2 IS NULL",
            run_id,
            None,
        ),
        WorkflowRunLoadKey::ProjectScoped { project_id, workflow_run_id } => (
            "SELECT workflow_run_id FROM workflow_runs
             WHERE workflow_run_id = ?1 AND project_id = ?2",
            workflow_run_id,
            Some(project_id),
        ),
    };
    connection
        .query_row(
            sql,
            params![
                run_id.as_uuid().as_bytes().as_slice(),
                project_id.map(|id| id.as_uuid().as_bytes().to_vec())
            ],
            |row| row.get::<_, Vec<u8>>(0),
        )
        .optional()
        .map_err(|_| persistence())?
        .map(|bytes| workflow_run_id(&bytes).and_then(|id| load_run_by_id(connection, id)))
        .transpose()
}

pub(super) fn list_active_runs(
    connection: &Connection,
    project_id: ProjectId,
    limit: usize,
) -> Result<Vec<WorkflowRunAggregate>, WorkflowApplicationError> {
    let mut statement = connection
        .prepare(
            "SELECT workflow_run_id, run_payload FROM workflow_runs
             WHERE project_id = ?1
             ORDER BY created_at DESC, workflow_run_id DESC",
        )
        .map_err(|_| persistence())?;
    let rows = statement
        .query_map([project_id.as_uuid().as_bytes().as_slice()], |row| {
            Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, Vec<u8>>(1)?))
        })
        .map_err(|_| persistence())?;
    let mut runs = Vec::with_capacity(limit);
    for row in rows {
        let (id, payload) = row.map_err(|_| persistence())?;
        let core = decode_run_core(&payload)?;
        if matches!(
            core.state,
            engine::workflow::WorkflowRunState::Queued
                | engine::workflow::WorkflowRunState::Running
        ) {
            runs.push(load_run_by_id(connection, workflow_run_id(&id)?)?);
            if runs.len() == limit {
                break;
            }
        }
    }
    Ok(runs)
}

fn load_run_by_id(
    connection: &Connection,
    run_id: WorkflowRunId,
) -> Result<WorkflowRunAggregate, WorkflowApplicationError> {
    let (project_id, core_payload, created_at, updated_at) = connection
        .query_row(
            "SELECT project_id, run_payload, created_at, updated_at
             FROM workflow_runs WHERE workflow_run_id = ?1",
            [run_id.as_uuid().as_bytes().as_slice()],
            |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, Vec<u8>>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            },
        )
        .map_err(|_| persistence())?;
    let core = decode_run_core(&core_payload)?;
    let mut nodes = load_node_executions(connection, run_id)?;
    let node_executions = core
        .plan
        .nodes()
        .iter()
        .map(|planned| nodes.remove(&planned.node_execution_id).ok_or_else(persistence))
        .collect::<Result<Vec<_>, _>>()?;
    if !nodes.is_empty() {
        return Err(persistence());
    }
    WorkflowRunAggregate::try_restore(WorkflowRunRestoreData {
        run_id,
        project_id: ProjectId::from_uuid(uuid(&project_id)?).ok_or_else(persistence)?,
        plan: core.plan,
        state: core.state,
        node_executions,
        events: load_all_events(connection, run_id)?,
        created_at: WorkflowRunTime::from_utc_milliseconds(created_at)?,
        updated_at: WorkflowRunTime::from_utc_milliseconds(updated_at)?,
        failure: core.failure,
    })
    .map_err(Into::into)
}

fn load_node_executions(
    connection: &Connection,
    run_id: WorkflowRunId,
) -> Result<
    BTreeMap<
        engine::node_capability::WorkflowNodeExecutionId,
        engine::workflow::WorkflowNodeExecutionRestoreData,
    >,
    WorkflowApplicationError,
> {
    let mut statement = connection
        .prepare(
            "SELECT workflow_node_id, node_execution_id, state, progress_basis_points,
                    started_at, finished_at, node_payload
             FROM workflow_node_executions WHERE workflow_run_id = ?1",
        )
        .map_err(|_| persistence())?;
    let rows = statement
        .query_map([run_id.as_uuid().as_bytes().as_slice()], |row| {
            Ok((
                row.get::<_, Vec<u8>>(0)?,
                row.get::<_, Vec<u8>>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, Option<u16>>(3)?,
                row.get::<_, Option<i64>>(4)?,
                row.get::<_, Option<i64>>(5)?,
                row.get::<_, Vec<u8>>(6)?,
            ))
        })
        .map_err(|_| persistence())?;
    let mut nodes = BTreeMap::new();
    for row in rows {
        let (node_id, execution_id, state, progress, started_at, finished_at, payload) =
            row.map_err(|_| persistence())?;
        let node_id = WorkflowNodeId::from_uuid(uuid(&node_id)?)?;
        let execution_id =
            engine::node_capability::WorkflowNodeExecutionId::from_uuid(uuid(&execution_id)?)
                .ok_or_else(persistence)?;
        let outputs = load_node_outputs(connection, run_id, execution_id)?;
        let node = decode_node_execution(
            NodeExecutionRowScalars {
                node_id,
                execution_id,
                state: decode_node_state_row(state)?,
                progress_basis_points: progress,
                started_at,
                finished_at,
            },
            outputs,
            &payload,
        )?;
        if nodes.insert(execution_id, node).is_some() {
            return Err(persistence());
        }
    }
    Ok(nodes)
}

fn load_node_outputs(
    connection: &Connection,
    run_id: WorkflowRunId,
    execution_id: engine::node_capability::WorkflowNodeExecutionId,
) -> Result<Option<engine::node_capability::WorkflowNodeOutputSet>, WorkflowApplicationError> {
    let mut statement = connection
        .prepare(
            "SELECT output_key, value_payload FROM workflow_node_execution_outputs
             WHERE workflow_run_id = ?1 AND node_execution_id = ?2 ORDER BY output_key",
        )
        .map_err(|_| persistence())?;
    let rows = statement
        .query_map(
            params![
                run_id.as_uuid().as_bytes().as_slice(),
                execution_id.as_uuid().as_bytes().as_slice()
            ],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?)),
        )
        .map_err(|_| persistence())?;
    let mut values = BTreeMap::new();
    for row in rows {
        let (key, payload) = row.map_err(|_| persistence())?;
        let key = engine::node_capability::NodeCapabilityOutputKey::new(key)
            .map_err(|_| persistence())?;
        let value = super::run::runtime_value::decode_value_bytes(&payload)?;
        if values.insert(key, value).is_some() {
            return Err(persistence());
        }
    }
    if values.is_empty() {
        Ok(None)
    } else {
        super::run::runtime_value::restore_output_values(values).map(Some)
    }
}

fn load_all_events(
    connection: &Connection,
    run_id: WorkflowRunId,
) -> Result<Vec<WorkflowRunEvent>, WorkflowApplicationError> {
    let mut statement = connection
        .prepare(
            "SELECT sequence, occurred_at, event_payload FROM workflow_run_events
             WHERE workflow_run_id = ?1 ORDER BY sequence",
        )
        .map_err(|_| persistence())?;
    statement
        .query_map([run_id.as_uuid().as_bytes().as_slice()], |row| {
            Ok((row.get::<_, u64>(0)?, row.get::<_, i64>(1)?, row.get::<_, Vec<u8>>(2)?))
        })
        .map_err(|_| persistence())?
        .map(|row| {
            let (sequence, occurred_at, bytes) = row.map_err(|_| persistence())?;
            decode_event_row(run_id, sequence, occurred_at, &bytes)
        })
        .collect()
}

pub(super) fn load_run_receipt(
    connection: &Connection,
    request_id: WorkflowRunRequestId,
) -> Result<Option<WorkflowRunAdmissionReceipt>, WorkflowApplicationError> {
    connection
        .query_row(
            "SELECT request_id, command_hash, workflow_run_id
             FROM workflow_run_request_receipts WHERE request_id = ?1",
            [request_id.as_uuid().as_bytes().as_slice()],
            |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, Vec<u8>>(1)?,
                    row.get::<_, Vec<u8>>(2)?,
                ))
            },
        )
        .optional()
        .map_err(|_| persistence())?
        .map(decode_receipt)
        .transpose()
}

pub(super) fn admit_run(
    connection: &mut Connection,
    commit: WorkflowRunAdmissionCommit,
) -> Result<WorkflowRunAdmissionReceipt, WorkflowApplicationError> {
    let (run, receipt, effect) = commit.into_parts();
    let transaction = connection.transaction().map_err(|_| persistence())?;
    if let Some(existing) = load_run_receipt(&transaction, receipt.request_id())? {
        if existing.command_hash() == receipt.command_hash() {
            return Ok(existing);
        }
        return Err(WorkflowApplicationError::WorkflowRunIdempotencyConflict);
    }
    insert_run(&transaction, &run)?;
    replace_node_executions(&transaction, &run)?;
    insert_events(&transaction, run.events())?;
    transaction
        .execute(
            "INSERT INTO workflow_run_request_receipts(request_id, command_hash, workflow_run_id)
             VALUES (?1, ?2, ?3)",
            params![
                receipt.request_id().as_uuid().as_bytes().as_slice(),
                receipt.command_hash().as_bytes().as_slice(),
                receipt.workflow_run_id().as_uuid().as_bytes().as_slice(),
            ],
        )
        .map_err(|_| persistence())?;
    let effect_id =
        DesktopPostCommitEffectId::from_uuid(run.run_id().as_uuid()).map_err(|_| persistence())?;
    let created_at =
        DesktopPostCommitTimestamp::from_epoch_millis(run.created_at().as_utc_milliseconds())
            .map_err(|_| persistence())?;
    insert_ready_post_commit_effect(
        &transaction,
        effect_id,
        DesktopPostCommitEffect::Workflow(effect),
        created_at,
    )
    .map_err(|_| persistence())?;
    transaction.commit().map_err(|_| persistence())?;
    Ok(receipt)
}

pub(super) fn commit_run_transition(
    connection: &mut Connection,
    run: WorkflowRunAggregate,
    expected_event_count: usize,
) -> Result<WorkflowRunAggregate, WorkflowApplicationError> {
    let transaction = connection.transaction().map_err(|_| persistence())?;
    let stored_count: i64 = transaction
        .query_row(
            "SELECT event_count FROM workflow_runs WHERE workflow_run_id = ?1",
            [run.run_id().as_uuid().as_bytes().as_slice()],
            |row| row.get(0),
        )
        .optional()
        .map_err(|_| persistence())?
        .ok_or(WorkflowApplicationError::WorkflowRunNotFound)?;
    if usize::try_from(stored_count).ok() != Some(expected_event_count)
        || run.events().len() < expected_event_count
    {
        return Err(WorkflowApplicationError::WorkflowPersistenceFailure);
    }
    update_run(&transaction, &run, expected_event_count)?;
    replace_node_executions(&transaction, &run)?;
    insert_events(&transaction, &run.events()[expected_event_count..])?;
    transaction.commit().map_err(|_| persistence())?;
    Ok(run)
}

pub(super) fn list_run_events(
    connection: &Connection,
    run_id: WorkflowRunId,
    after: Option<WorkflowRunEventSequence>,
    limit: usize,
) -> Result<Vec<WorkflowRunEvent>, WorkflowApplicationError> {
    if !(1..=500).contains(&limit) {
        return Err(WorkflowApplicationError::WorkflowRunEventLimitOutOfBounds {
            requested_limit: u16::try_from(limit).unwrap_or(u16::MAX),
        });
    }
    let mut statement = connection
        .prepare(
            "SELECT sequence, occurred_at, event_payload FROM workflow_run_events
             WHERE workflow_run_id = ?1 AND sequence > ?2 ORDER BY sequence LIMIT ?3",
        )
        .map_err(|_| persistence())?;
    statement
        .query_map(
            params![
                run_id.as_uuid().as_bytes().as_slice(),
                after.map(WorkflowRunEventSequence::get).unwrap_or(0),
                limit
            ],
            |row| Ok((row.get::<_, u64>(0)?, row.get::<_, i64>(1)?, row.get::<_, Vec<u8>>(2)?)),
        )
        .map_err(|_| persistence())?
        .map(|row| {
            let (sequence, occurred_at, bytes) = row.map_err(|_| persistence())?;
            decode_event_row(run_id, sequence, occurred_at, &bytes)
        })
        .collect()
}

pub(super) fn load_latest_run_for_node(
    connection: &Connection,
    project_id: ProjectId,
    workflow_id: WorkflowId,
    node_id: WorkflowNodeId,
) -> Result<Option<WorkflowRunAggregate>, WorkflowApplicationError> {
    connection
        .query_row(
            "SELECT runs.workflow_run_id
             FROM workflow_runs AS runs
             JOIN workflow_node_executions AS nodes
               ON nodes.workflow_run_id = runs.workflow_run_id
             WHERE runs.project_id = ?1 AND runs.workflow_id = ?2
               AND nodes.workflow_node_id = ?3
             ORDER BY runs.created_at DESC, runs.workflow_run_id DESC
             LIMIT 1",
            params![
                project_id.as_uuid().as_bytes().as_slice(),
                workflow_id.as_uuid().as_bytes().as_slice(),
                node_id.as_uuid().as_bytes().as_slice(),
            ],
            |row| row.get::<_, Vec<u8>>(0),
        )
        .optional()
        .map_err(|_| persistence())?
        .map(|bytes| workflow_run_id(&bytes).and_then(|id| load_run_by_id(connection, id)))
        .transpose()
}
