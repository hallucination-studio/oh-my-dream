use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use projects::project::domain::ProjectId;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use tasks::generation_task::{application::*, domain::*, interfaces::*};

use super::{schema::SCHEMA, task_sql};

/// SQLite implementation of the Task aggregate repository and outbox reader.
#[derive(Clone)]
pub struct SqliteGenerationTaskRepositoryAdapterImpl {
    pub(super) connection: Arc<Mutex<Connection>>,
}

impl SqliteGenerationTaskRepositoryAdapterImpl {
    /// Creates the two frozen Task tables on an existing metadata connection.
    pub fn try_new(
        connection: Arc<Mutex<Connection>>,
    ) -> Result<Self, GenerationTaskRepositoryError> {
        let locked =
            connection.lock().map_err(|_| GenerationTaskRepositoryError::StorageFailure)?;
        locked.pragma_update(None, "foreign_keys", true).map_err(storage)?;
        locked.execute_batch(SCHEMA).map_err(storage)?;
        drop(locked);
        Ok(Self { connection })
    }

    pub(super) fn with_connection<T>(
        &self,
        operation: impl FnOnce(&mut Connection) -> Result<T, GenerationTaskRepositoryError>,
    ) -> Result<T, GenerationTaskRepositoryError> {
        let mut connection =
            self.connection.lock().map_err(|_| GenerationTaskRepositoryError::StorageFailure)?;
        operation(&mut connection)
    }
}

#[async_trait]
impl GenerationTaskRepositoryInterface for SqliteGenerationTaskRepositoryAdapterImpl {
    async fn create_generation_task(
        &self,
        task: &GenerationTaskAggregate,
        message: GenerationTaskEffect,
    ) -> Result<GenerationTaskCreateResult, GenerationTaskRepositoryError> {
        if message.task_id() != task.id() {
            return Err(GenerationTaskRepositoryError::Corruption);
        }
        self.with_connection(|connection| {
            let transaction = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(storage)?;
            if let Some(existing) = task_sql::load_by_idempotency(
                &transaction,
                task.origin().project_id(),
                task.idempotency_key().as_str(),
            )? {
                return task_sql::matching_existing(existing, task, true);
            }
            if let Some(existing) = task_sql::load_by_origin(
                &transaction,
                task.origin().project_id(),
                task.origin().workflow_node_execution_id().as_uuid().as_bytes(),
            )? {
                return task_sql::matching_existing(existing, task, false);
            }
            task_sql::insert_task(&transaction, task)?;
            task_sql::insert_effect(&transaction, &message, task.created_at())?;
            transaction.commit().map_err(storage)?;
            Ok(GenerationTaskCreateResult::Created(task.clone()))
        })
    }

    async fn load_generation_task(
        &self,
        id: GenerationTaskId,
    ) -> Result<Option<GenerationTaskAggregate>, GenerationTaskRepositoryError> {
        self.with_connection(|connection| {
            task_sql::load_by_sql(connection, "id = ?1", [id.as_uuid().as_bytes()])
        })
    }

    async fn load_generation_task_for_project(
        &self,
        project_id: ProjectId,
        id: GenerationTaskId,
    ) -> Result<Option<GenerationTaskAggregate>, GenerationTaskRepositoryError> {
        self.with_connection(|connection| {
            let sql = format!(
                "SELECT {} FROM generation_tasks WHERE project_id = ?1 AND id = ?2",
                task_sql::TASK_COLUMNS
            );
            let row = connection
                .query_row(
                    &sql,
                    params![project_id.as_uuid().as_bytes(), id.as_uuid().as_bytes()],
                    super::translator::TaskRow::from_sql,
                )
                .optional()
                .map_err(storage)?;
            row.map(task_sql::restore).transpose()
        })
    }

    async fn save_generation_task(
        &self,
        task: &GenerationTaskAggregate,
        expected_revision: u64,
        outbox: GenerationTaskOutboxChanges,
    ) -> Result<(), GenerationTaskRepositoryError> {
        self.with_connection(|connection| {
            let transaction = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(storage)?;
            let current: Option<i64> = transaction
                .query_row(
                    "SELECT revision FROM generation_tasks WHERE id = ?1",
                    [task.id().as_uuid().as_bytes()],
                    |row| row.get(0),
                )
                .optional()
                .map_err(storage)?;
            let expected = i64::try_from(expected_revision)
                .map_err(|_| GenerationTaskRepositoryError::OptimisticConflict)?;
            if current != Some(expected) || task.revision().get() < expected_revision {
                return Err(GenerationTaskRepositoryError::OptimisticConflict);
            }
            let stored =
                task_sql::load_by_sql(&transaction, "id = ?1", [task.id().as_uuid().as_bytes()])?
                    .ok_or(GenerationTaskRepositoryError::Corruption)?;
            if !stored.has_same_immutable_facts(task)
                || outbox.enqueue.iter().any(|effect| effect.task_id() != task.id())
            {
                return Err(GenerationTaskRepositoryError::Corruption);
            }
            if let Some(claim) = outbox.consume {
                task_sql::consume_effect(&transaction, task, claim)?;
            }
            task_sql::update_task(&transaction, task, expected_revision)?;
            for effect in outbox.enqueue {
                task_sql::insert_effect(&transaction, &effect, task.updated_at())?;
            }
            transaction.commit().map_err(storage)
        })
    }

    async fn list_generation_tasks(
        &self,
        query: GenerationTaskListQuery,
    ) -> Result<GenerationTaskCursorPage<GenerationTaskSummaryView>, GenerationTaskRepositoryError>
    {
        self.with_connection(|connection| task_sql::list_tasks(connection, query))
    }
}

pub(super) fn storage(_: rusqlite::Error) -> GenerationTaskRepositoryError {
    GenerationTaskRepositoryError::StorageFailure
}
