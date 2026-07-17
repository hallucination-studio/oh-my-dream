use std::sync::{Arc, Mutex};

use assistant::{
    domain::{AssistantRepairActivationId, AssistantSessionId},
    interfaces::{
        AssistantApplicationError, AssistantFailedWorkflowRunId, AssistantRepairActivation,
        AssistantRepairActivationRecordResult, AssistantRepairActivationRepositoryInterface,
    },
};
use async_trait::async_trait;
use projects::project::domain::ProjectId;
use rusqlite::{Connection, OptionalExtension, params};
use uuid::Uuid;

const CREATE_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS assistant_repair_activations (
    activation_id BLOB PRIMARY KEY NOT NULL CHECK(length(activation_id) = 16),
    project_id BLOB NOT NULL CHECK(length(project_id) = 16),
    session_id BLOB NOT NULL CHECK(length(session_id) = 16),
    failed_run_id BLOB NOT NULL CHECK(length(failed_run_id) = 16),
    failed_run_facts BLOB NOT NULL CHECK(length(failed_run_facts) > 0),
    created_at INTEGER NOT NULL CHECK(created_at >= 0),
    UNIQUE(project_id, failed_run_id)
);
";

type RepairActivationRawRow = (Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>, i64);

#[derive(Clone)]
pub struct SqliteAssistantRepairActivationRepositoryAdapterImpl {
    connection: Arc<Mutex<Connection>>,
}

impl SqliteAssistantRepairActivationRepositoryAdapterImpl {
    pub fn try_new(connection: Arc<Mutex<Connection>>) -> Result<Self, AssistantApplicationError> {
        connection
            .lock()
            .map_err(|_| storage())?
            .execute_batch(CREATE_SCHEMA)
            .map_err(|_| storage())?;
        Ok(Self { connection })
    }

    async fn blocking<T: Send + 'static>(
        &self,
        operation: impl FnOnce(&mut Connection) -> Result<T, AssistantApplicationError> + Send + 'static,
    ) -> Result<T, AssistantApplicationError> {
        let connection = Arc::clone(&self.connection);
        tokio::task::spawn_blocking(move || {
            let mut connection = connection.lock().map_err(|_| storage())?;
            operation(&mut connection)
        })
        .await
        .map_err(|_| storage())?
    }
}

#[async_trait]
impl AssistantRepairActivationRepositoryInterface
    for SqliteAssistantRepairActivationRepositoryAdapterImpl
{
    async fn record_or_get_repair_activation(
        &self,
        activation: AssistantRepairActivation,
    ) -> Result<AssistantRepairActivationRecordResult, AssistantApplicationError> {
        self.blocking(move |connection| record_or_get(connection, activation)).await
    }

    async fn load_repair_activation(
        &self,
        project_id: ProjectId,
        activation_id: AssistantRepairActivationId,
    ) -> Result<Option<AssistantRepairActivation>, AssistantApplicationError> {
        self.blocking(move |connection| load_by_id(connection, project_id, activation_id)).await
    }

    async fn load_repair_activation_for_failed_run(
        &self,
        project_id: ProjectId,
        failed_workflow_run_id: AssistantFailedWorkflowRunId,
    ) -> Result<Option<AssistantRepairActivation>, AssistantApplicationError> {
        self.blocking(move |connection| load_by_run(connection, project_id, failed_workflow_run_id))
            .await
    }
}

fn record_or_get(
    connection: &mut Connection,
    activation: AssistantRepairActivation,
) -> Result<AssistantRepairActivationRecordResult, AssistantApplicationError> {
    let transaction = connection.transaction().map_err(|_| storage())?;
    if let Some(existing) =
        load_by_run(&transaction, activation.project_id(), activation.failed_workflow_run_id())?
    {
        transaction.commit().map_err(|_| storage())?;
        return Ok(AssistantRepairActivationRecordResult::Existing(existing));
    }
    let changed = transaction
        .execute(
            "INSERT OR IGNORE INTO assistant_repair_activations
             (activation_id, project_id, session_id, failed_run_id, failed_run_facts, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                activation.id().as_uuid().as_bytes(),
                activation.project_id().as_uuid().as_bytes(),
                activation.session_id().as_uuid().as_bytes(),
                activation.failed_workflow_run_id().0,
                activation.exact_failed_run_facts(),
                activation.created_at_epoch_ms(),
            ],
        )
        .map_err(|_| storage())?;
    if changed == 0 {
        let existing = load_by_run(
            &transaction,
            activation.project_id(),
            activation.failed_workflow_run_id(),
        )?
        .ok_or_else(storage)?;
        transaction.commit().map_err(|_| storage())?;
        return Ok(AssistantRepairActivationRecordResult::Existing(existing));
    }
    transaction.commit().map_err(|_| storage())?;
    Ok(AssistantRepairActivationRecordResult::Created(activation))
}

fn load_by_id(
    connection: &Connection,
    project_id: ProjectId,
    activation_id: AssistantRepairActivationId,
) -> Result<Option<AssistantRepairActivation>, AssistantApplicationError> {
    read(
        connection,
        "SELECT activation_id, project_id, session_id, failed_run_id, failed_run_facts, created_at
         FROM assistant_repair_activations WHERE project_id = ?1 AND activation_id = ?2",
        params![project_id.as_uuid().as_bytes(), activation_id.as_uuid().as_bytes()],
    )
}

fn load_by_run(
    connection: &Connection,
    project_id: ProjectId,
    failed_run_id: AssistantFailedWorkflowRunId,
) -> Result<Option<AssistantRepairActivation>, AssistantApplicationError> {
    read(
        connection,
        "SELECT activation_id, project_id, session_id, failed_run_id, failed_run_facts, created_at
         FROM assistant_repair_activations WHERE project_id = ?1 AND failed_run_id = ?2",
        params![project_id.as_uuid().as_bytes(), failed_run_id.0],
    )
}

fn read(
    connection: &Connection,
    sql: &str,
    parameters: impl rusqlite::Params,
) -> Result<Option<AssistantRepairActivation>, AssistantApplicationError> {
    let row = connection
        .query_row(sql, parameters, |row| {
            Ok((
                row.get::<_, Vec<u8>>(0)?,
                row.get::<_, Vec<u8>>(1)?,
                row.get::<_, Vec<u8>>(2)?,
                row.get::<_, Vec<u8>>(3)?,
                row.get::<_, Vec<u8>>(4)?,
                row.get::<_, i64>(5)?,
            ))
        })
        .optional()
        .map_err(|_| storage())?;
    row.map(decode).transpose()
}

fn decode(
    row: RepairActivationRawRow,
) -> Result<AssistantRepairActivation, AssistantApplicationError> {
    let (activation_id, project_id, session_id, failed_run_id, facts, created_at) = row;
    AssistantRepairActivation::new(
        AssistantRepairActivationId::from_uuid(uuid(activation_id)?).map_err(|_| storage())?,
        ProjectId::from_uuid(uuid(project_id)?).ok_or_else(storage)?,
        AssistantSessionId::from_uuid(uuid(session_id)?).map_err(|_| storage())?,
        AssistantFailedWorkflowRunId(failed_run_id.try_into().map_err(|_| storage())?),
        facts,
        created_at,
    )
    .map_err(|_| storage())
}

fn uuid(bytes: Vec<u8>) -> Result<Uuid, AssistantApplicationError> {
    Uuid::from_slice(&bytes).map_err(|_| storage())
}

fn storage() -> AssistantApplicationError {
    AssistantApplicationError::ExternalBoundaryFailed
}
