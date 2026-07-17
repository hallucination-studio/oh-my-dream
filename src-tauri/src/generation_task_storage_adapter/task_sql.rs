use projects::project::domain::ProjectId;
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use tasks::generation_task::{application::*, domain::*};

use super::{outbox::encode_effect_kind, repository::storage, translator};

pub(super) const TASK_COLUMNS: &str = "id, project_id, workflow_id, workflow_run_id,
    workflow_node_id, workflow_node_execution_id, idempotency_key, request_hash,
    request_schema_version, request_kind, request_json, generation_profile_ref, provider_id,
    route_id, status, progress_percent, remote_task_id, result_kind, result_text,
    result_asset_id, result_media_kind, failure_kind, failure_code, failure_message,
    provider_deadline_at, completed_at, created_at, updated_at, revision";

pub(super) fn load_by_idempotency(
    connection: &Connection,
    project_id: ProjectId,
    key: &str,
) -> Result<Option<GenerationTaskAggregate>, GenerationTaskRepositoryError> {
    let sql = format!(
        "SELECT {TASK_COLUMNS} FROM generation_tasks
         WHERE project_id = ?1 AND idempotency_key = ?2"
    );
    let row = connection
        .query_row(
            &sql,
            params![project_id.as_uuid().as_bytes(), key.as_bytes()],
            translator::TaskRow::from_sql,
        )
        .optional()
        .map_err(storage)?;
    row.map(restore).transpose()
}

pub(super) fn load_by_origin(
    connection: &Connection,
    project_id: ProjectId,
    execution_id: &[u8],
) -> Result<Option<GenerationTaskAggregate>, GenerationTaskRepositoryError> {
    let sql = format!(
        "SELECT {TASK_COLUMNS} FROM generation_tasks
         WHERE project_id = ?1 AND workflow_node_execution_id = ?2"
    );
    let row = connection
        .query_row(
            &sql,
            params![project_id.as_uuid().as_bytes(), execution_id],
            translator::TaskRow::from_sql,
        )
        .optional()
        .map_err(storage)?;
    row.map(restore).transpose()
}

pub(super) fn load_by_sql<P: rusqlite::Params>(
    connection: &Connection,
    predicate: &str,
    params: P,
) -> Result<Option<GenerationTaskAggregate>, GenerationTaskRepositoryError> {
    let sql = format!("SELECT {TASK_COLUMNS} FROM generation_tasks WHERE {predicate}");
    let row = connection
        .query_row(&sql, params, translator::TaskRow::from_sql)
        .optional()
        .map_err(storage)?;
    row.map(restore).transpose()
}

pub(super) fn restore(
    row: translator::TaskRow,
) -> Result<GenerationTaskAggregate, GenerationTaskRepositoryError> {
    row.restore().map_err(|()| GenerationTaskRepositoryError::Corruption)
}

pub(super) fn matching_existing(
    existing: GenerationTaskAggregate,
    requested: &GenerationTaskAggregate,
    idempotency: bool,
) -> Result<GenerationTaskCreateResult, GenerationTaskRepositoryError> {
    if existing.request_hash() != requested.request_hash() {
        return Err(if idempotency {
            GenerationTaskRepositoryError::IdempotencyConflict
        } else {
            GenerationTaskRepositoryError::OriginConflict
        });
    }
    Ok(GenerationTaskCreateResult::Existing(existing))
}

pub(super) fn insert_task(
    transaction: &Transaction<'_>,
    task: &GenerationTaskAggregate,
) -> Result<(), GenerationTaskRepositoryError> {
    let encoded =
        translator::encode_task(task).map_err(|()| GenerationTaskRepositoryError::Corruption)?;
    transaction
        .execute(
            "INSERT INTO generation_tasks VALUES (
             ?1,?2,?3,?4,?5,?6,?7,?8,1,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,
             ?20,?21,?22,?23,?24,?25,?26,?27,?28)",
            params![
                task.id().as_uuid().as_bytes(),
                task.origin().project_id().as_uuid().as_bytes(),
                task.origin().workflow_id().as_uuid().as_bytes(),
                task.origin().workflow_run_id().as_uuid().as_bytes(),
                task.origin().workflow_node_id().as_uuid().as_bytes(),
                task.origin().workflow_node_execution_id().as_uuid().as_bytes(),
                task.idempotency_key().as_str().as_bytes(),
                task.request_hash().as_bytes().as_slice(),
                encoded.request_kind,
                encoded.request_json,
                task.target().generation_profile_ref().to_string(),
                task.target().provider_id().as_str(),
                task.target().route_id().as_str(),
                encoded.status,
                encoded.progress_percent,
                encoded.remote_task_id,
                encoded.result_kind,
                encoded.result_text,
                encoded.result_asset_id,
                encoded.result_media_kind,
                encoded.failure_kind,
                encoded.failure_code,
                encoded.failure_message,
                task.provider_deadline_at().as_utc_milliseconds(),
                encoded.completed_at,
                task.created_at().as_utc_milliseconds(),
                task.updated_at().as_utc_milliseconds(),
                i64::try_from(task.revision().get())
                    .map_err(|_| GenerationTaskRepositoryError::StorageFailure)?
            ],
        )
        .map_err(storage)?;
    Ok(())
}

pub(super) fn update_task(
    transaction: &Transaction<'_>,
    task: &GenerationTaskAggregate,
    expected_revision: u64,
) -> Result<(), GenerationTaskRepositoryError> {
    let encoded =
        translator::encode_task(task).map_err(|()| GenerationTaskRepositoryError::Corruption)?;
    let count = transaction
        .execute(
            "UPDATE generation_tasks SET status=?1, progress_percent=?2, remote_task_id=?3,
             result_kind=?4, result_text=?5, result_asset_id=?6, result_media_kind=?7,
             failure_kind=?8, failure_code=?9, failure_message=?10, completed_at=?11,
             updated_at=?12, revision=?13 WHERE id=?14 AND revision=?15",
            params![
                encoded.status,
                encoded.progress_percent,
                encoded.remote_task_id,
                encoded.result_kind,
                encoded.result_text,
                encoded.result_asset_id,
                encoded.result_media_kind,
                encoded.failure_kind,
                encoded.failure_code,
                encoded.failure_message,
                encoded.completed_at,
                task.updated_at().as_utc_milliseconds(),
                i64::try_from(task.revision().get())
                    .map_err(|_| GenerationTaskRepositoryError::StorageFailure)?,
                task.id().as_uuid().as_bytes(),
                i64::try_from(expected_revision)
                    .map_err(|_| GenerationTaskRepositoryError::OptimisticConflict)?
            ],
        )
        .map_err(storage)?;
    if count == 1 { Ok(()) } else { Err(GenerationTaskRepositoryError::OptimisticConflict) }
}

pub(super) fn insert_effect(
    transaction: &Transaction<'_>,
    effect: &GenerationTaskEffect,
    created_at: GenerationTaskTimestamp,
) -> Result<(), GenerationTaskRepositoryError> {
    let kind = encode_effect_kind(effect.kind());
    let dedup = format!(
        "{}:{kind}:{}:{}",
        effect.task_id(),
        effect.available_at().as_utc_milliseconds(),
        effect.delivery_attempts()
    );
    transaction
        .execute(
            "INSERT INTO generation_task_outbox(
             task_id,kind,payload_json,deduplication_key,available_at,state,delivery_attempts,
             processed_at,last_error,created_at)
             VALUES (?1,?2,'{}',?3,?4,'Ready',?5,NULL,NULL,?6)",
            params![
                effect.task_id().as_uuid().as_bytes(),
                kind,
                dedup,
                effect.available_at().as_utc_milliseconds(),
                i64::from(effect.delivery_attempts()),
                created_at.as_utc_milliseconds()
            ],
        )
        .map_err(storage)?;
    Ok(())
}

pub(super) fn consume_effect(
    transaction: &Transaction<'_>,
    task: &GenerationTaskAggregate,
    claim: GenerationTaskEffectClaim,
) -> Result<(), GenerationTaskRepositoryError> {
    let count = transaction
        .execute(
            "UPDATE generation_task_outbox SET state='Completed', processed_at=?1
             WHERE id=?2 AND task_id=?3 AND state='Claimed'",
            params![
                task.updated_at().as_utc_milliseconds(),
                i64::try_from(claim.effect_id().get())
                    .map_err(|_| GenerationTaskRepositoryError::EffectClaimConflict)?,
                task.id().as_uuid().as_bytes()
            ],
        )
        .map_err(storage)?;
    if count == 1 { Ok(()) } else { Err(GenerationTaskRepositoryError::EffectClaimConflict) }
}

pub(super) fn list_tasks(
    connection: &Connection,
    query: GenerationTaskListQuery,
) -> Result<GenerationTaskCursorPage<GenerationTaskSummaryView>, GenerationTaskRepositoryError> {
    use rusqlite::types::Value;
    let mut sql = format!("SELECT {TASK_COLUMNS} FROM generation_tasks WHERE project_id = ?");
    let mut values = vec![Value::Blob(query.project_id().as_uuid().as_bytes().to_vec())];
    if let Some(status) = query.status() {
        if status == GenerationTaskStatus::Running {
            sql.push_str(" AND status IN ('Submitting','Running')");
        } else {
            sql.push_str(" AND status = ?");
            values.push(Value::Text(translator::status(status).into()));
        }
    }
    if let Some(kind) = query.request_kind() {
        sql.push_str(" AND request_kind = ?");
        values.push(Value::Text(translator::kind(kind).into()));
    }
    if let Some(cursor) = query.cursor() {
        sql.push_str(" AND (created_at < ? OR (created_at = ? AND id < ?))");
        values.push(Value::Integer(cursor.created_at.as_utc_milliseconds()));
        values.push(Value::Integer(cursor.created_at.as_utc_milliseconds()));
        values.push(Value::Blob(cursor.task_id.as_uuid().as_bytes().to_vec()));
    }
    sql.push_str(" ORDER BY created_at DESC, id DESC LIMIT ?");
    values.push(Value::Integer(i64::from(query.limit()) + 1));
    let mut statement = connection.prepare(&sql).map_err(storage)?;
    let rows = statement
        .query_map(rusqlite::params_from_iter(values), translator::TaskRow::from_sql)
        .map_err(storage)?;
    let mut tasks =
        rows.map(|row| row.map_err(storage).and_then(restore)).collect::<Result<Vec<_>, _>>()?;
    let has_more = tasks.len() > usize::from(query.limit());
    tasks.truncate(usize::from(query.limit()));
    let next_cursor = has_more.then(|| {
        let task = &tasks[tasks.len() - 1];
        GenerationTaskListCursor { created_at: task.created_at(), task_id: task.id() }
    });
    Ok(GenerationTaskCursorPage {
        items: tasks.iter().map(GenerationTaskSummaryView::from_task).collect(),
        next_cursor,
    })
}
