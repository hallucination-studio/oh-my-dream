use engine::{
    node_capability::WorkflowRunId,
    workflow::{WorkflowApplicationError, WorkflowRunEvent, WorkflowRunEventSequence},
};
use rusqlite::{Connection, params};

use super::super::{persistence, run::decode_event_row};
use super::write::uuid;

pub(crate) fn list_undelivered_events(
    connection: &Connection,
    limit: usize,
) -> Result<Vec<WorkflowRunEvent>, WorkflowApplicationError> {
    if !(1..=500).contains(&limit) {
        return Err(WorkflowApplicationError::WorkflowRunEventLimitOutOfBounds {
            requested_limit: u16::try_from(limit).unwrap_or(u16::MAX),
        });
    }
    let mut statement = connection
        .prepare(
            "SELECT workflow_run_id, sequence, occurred_at, event_payload
             FROM workflow_run_events
             WHERE delivered = 0 ORDER BY workflow_run_id, sequence LIMIT ?1",
        )
        .map_err(|_| persistence())?;
    statement
        .query_map([limit], |row| {
            Ok((
                row.get::<_, Vec<u8>>(0)?,
                row.get::<_, u64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, Vec<u8>>(3)?,
            ))
        })
        .map_err(|_| persistence())?
        .map(|row| {
            let (run_id, sequence, occurred_at, payload) = row.map_err(|_| persistence())?;
            let run_id = WorkflowRunId::from_uuid(uuid(&run_id)?).ok_or_else(persistence)?;
            decode_event_row(run_id, sequence, occurred_at, &payload)
        })
        .collect()
}

pub(crate) fn record_event_delivery_attempt(
    connection: &Connection,
    run_id: WorkflowRunId,
    sequence: WorkflowRunEventSequence,
    delivered: bool,
) -> Result<(), WorkflowApplicationError> {
    let changed = connection
        .execute(
            "UPDATE workflow_run_events
             SET delivery_attempt_count = delivery_attempt_count + 1,
                 delivered = CASE WHEN ?3 THEN 1 ELSE delivered END
             WHERE workflow_run_id = ?1 AND sequence = ?2 AND delivered = 0",
            params![run_id.as_uuid().as_bytes().as_slice(), sequence.get(), delivered],
        )
        .map_err(|_| persistence())?;
    if changed == 1 { Ok(()) } else { Err(persistence()) }
}
