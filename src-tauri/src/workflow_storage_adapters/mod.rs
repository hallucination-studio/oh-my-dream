//! SQLite adapters for Workflow snapshots, Runs, receipts, events, and effects.

mod graph;
mod receipt;
mod run;
mod run_repository;

#[cfg(test)]
mod tests;

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use engine::{
    node_capability::{WorkflowNodeCapabilityRegistry, WorkflowRunId},
    workflow::{
        WorkflowAggregateRepositoryInterface, WorkflowApplicationError, WorkflowCreateReceipt,
        WorkflowCreationCommit, WorkflowLoadKey, WorkflowMutationCommit,
        WorkflowRunAdmissionCommit, WorkflowRunAdmissionReceipt, WorkflowRunEvent,
        WorkflowRunEventSequence, WorkflowRunLoadKey, WorkflowRunRepositoryInterface,
        WorkflowRunRequestId,
    },
    workflow_graph::{WorkflowAggregate, WorkflowMutationReceipt, WorkflowMutationRequestId},
};
use rusqlite::{Connection, OptionalExtension, params};

use graph::{decode_workflow, encode_workflow};
use receipt::{
    insert_creation_receipt, insert_mutation_receipt, load_creation_receipt, load_mutation_receipt,
};
use run_repository::*;

const CREATE_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS workflows (
    workflow_id BLOB PRIMARY KEY NOT NULL CHECK(length(workflow_id) = 16),
    project_id BLOB UNIQUE NOT NULL CHECK(length(project_id) = 16),
    schema_version INTEGER NOT NULL CHECK(schema_version > 0),
    revision INTEGER NOT NULL CHECK(revision > 0),
    graph_payload BLOB NOT NULL CHECK(length(graph_payload) <= 1048576),
    created_at INTEGER NOT NULL CHECK(created_at >= 0),
    updated_at INTEGER NOT NULL CHECK(updated_at >= created_at)
);
CREATE TABLE IF NOT EXISTS workflow_create_receipts (
    request_id BLOB PRIMARY KEY NOT NULL CHECK(length(request_id) = 16),
    command_hash BLOB NOT NULL CHECK(length(command_hash) = 32),
    workflow_snapshot BLOB NOT NULL CHECK(length(workflow_snapshot) <= 1048576),
    result_fingerprint BLOB NOT NULL CHECK(length(result_fingerprint) = 32)
);
CREATE TABLE IF NOT EXISTS workflow_mutation_receipts (
    request_id BLOB PRIMARY KEY NOT NULL CHECK(length(request_id) = 16),
    command_hash BLOB NOT NULL CHECK(length(command_hash) = 32),
    workflow_snapshot BLOB NOT NULL CHECK(length(workflow_snapshot) <= 1048576),
    result_fingerprint BLOB NOT NULL CHECK(length(result_fingerprint) = 32)
);
CREATE TABLE IF NOT EXISTS workflow_runs (
    workflow_run_id BLOB PRIMARY KEY NOT NULL CHECK(length(workflow_run_id) = 16),
    project_id BLOB NOT NULL CHECK(length(project_id) = 16),
    workflow_id BLOB NOT NULL CHECK(length(workflow_id) = 16),
    workflow_revision INTEGER NOT NULL CHECK(workflow_revision > 0),
    run_payload BLOB NOT NULL CHECK(length(run_payload) <= 4194304),
    event_count INTEGER NOT NULL CHECK(event_count > 0),
    created_at INTEGER NOT NULL CHECK(created_at >= 0),
    updated_at INTEGER NOT NULL CHECK(updated_at >= created_at)
);
CREATE TABLE IF NOT EXISTS workflow_node_executions (
    workflow_run_id BLOB NOT NULL CHECK(length(workflow_run_id) = 16),
    workflow_node_id BLOB NOT NULL CHECK(length(workflow_node_id) = 16),
    node_execution_id BLOB NOT NULL CHECK(length(node_execution_id) = 16),
    state INTEGER NOT NULL CHECK(state BETWEEN 0 AND 5),
    progress_basis_points INTEGER CHECK(
        progress_basis_points IS NULL OR progress_basis_points BETWEEN 0 AND 10000
    ),
    started_at INTEGER,
    finished_at INTEGER,
    node_payload BLOB NOT NULL CHECK(length(node_payload) <= 2097152),
    PRIMARY KEY(workflow_run_id, node_execution_id),
    FOREIGN KEY(workflow_run_id) REFERENCES workflow_runs(workflow_run_id) ON DELETE CASCADE
);
CREATE TABLE IF NOT EXISTS workflow_node_execution_outputs (
    workflow_run_id BLOB NOT NULL CHECK(length(workflow_run_id) = 16),
    node_execution_id BLOB NOT NULL CHECK(length(node_execution_id) = 16),
    output_key TEXT NOT NULL,
    value_payload BLOB NOT NULL CHECK(length(value_payload) <= 1048576),
    PRIMARY KEY(workflow_run_id, node_execution_id, output_key),
    FOREIGN KEY(workflow_run_id, node_execution_id)
        REFERENCES workflow_node_executions(workflow_run_id, node_execution_id) ON DELETE CASCADE
);
CREATE TABLE IF NOT EXISTS workflow_run_events (
    workflow_run_id BLOB NOT NULL CHECK(length(workflow_run_id) = 16),
    sequence INTEGER NOT NULL CHECK(sequence > 0),
    event_payload BLOB NOT NULL CHECK(length(event_payload) <= 1048576),
    occurred_at INTEGER NOT NULL CHECK(occurred_at >= 0),
    delivery_attempt_count INTEGER NOT NULL DEFAULT 0 CHECK(delivery_attempt_count >= 0),
    delivered INTEGER NOT NULL DEFAULT 0 CHECK(delivered IN (0, 1)),
    PRIMARY KEY(workflow_run_id, sequence),
    FOREIGN KEY(workflow_run_id) REFERENCES workflow_runs(workflow_run_id) ON DELETE CASCADE
);
CREATE TABLE IF NOT EXISTS workflow_run_request_receipts (
    request_id BLOB PRIMARY KEY NOT NULL CHECK(length(request_id) = 16),
    command_hash BLOB NOT NULL CHECK(length(command_hash) = 32),
    workflow_run_id BLOB NOT NULL CHECK(length(workflow_run_id) = 16)
);
CREATE INDEX IF NOT EXISTS workflow_runs_latest_node_order
    ON workflow_runs(project_id, workflow_id, created_at DESC, workflow_run_id DESC);
CREATE INDEX IF NOT EXISTS workflow_run_events_delivery
    ON workflow_run_events(delivered, workflow_run_id, sequence);
";

/// SQLite implementation of both Workflow aggregate and Run persistence boundaries.
#[derive(Clone)]
pub struct SqliteWorkflowRunRepositoryAdapterImpl {
    connection: Arc<Mutex<Connection>>,
    capabilities: Arc<WorkflowNodeCapabilityRegistry>,
}

impl SqliteWorkflowRunRepositoryAdapterImpl {
    /// Initializes Workflow-private tables on an already validated metadata connection.
    pub fn try_new(
        connection: Arc<Mutex<Connection>>,
        capabilities: Arc<WorkflowNodeCapabilityRegistry>,
    ) -> Result<Self, WorkflowApplicationError> {
        connection
            .lock()
            .map_err(|_| persistence())?
            .execute_batch(CREATE_SCHEMA)
            .map_err(|_| persistence())?;
        Ok(Self { connection, capabilities })
    }

    async fn blocking<T: Send + 'static>(
        &self,
        operation: impl FnOnce(
            &mut Connection,
            &WorkflowNodeCapabilityRegistry,
        ) -> Result<T, WorkflowApplicationError>
        + Send
        + 'static,
    ) -> Result<T, WorkflowApplicationError> {
        let connection = Arc::clone(&self.connection);
        let capabilities = Arc::clone(&self.capabilities);
        tokio::task::spawn_blocking(move || {
            let mut connection = connection.lock().map_err(|_| persistence())?;
            operation(&mut connection, &capabilities)
        })
        .await
        .map_err(|_| persistence())?
    }

    /// Loads a bounded page of committed events not yet delivered to Desktop.
    pub async fn list_undelivered_workflow_run_events(
        &self,
        limit: usize,
    ) -> Result<Vec<WorkflowRunEvent>, WorkflowApplicationError> {
        self.blocking(move |connection, _| list_undelivered_events(connection, limit)).await
    }

    /// Records one Desktop delivery attempt and whether it completed successfully.
    pub async fn record_workflow_run_event_delivery_attempt(
        &self,
        workflow_run_id: WorkflowRunId,
        sequence: WorkflowRunEventSequence,
        delivered: bool,
    ) -> Result<(), WorkflowApplicationError> {
        self.blocking(move |connection, _| {
            record_event_delivery_attempt(connection, workflow_run_id, sequence, delivered)
        })
        .await
    }
}

#[async_trait]
impl WorkflowAggregateRepositoryInterface for SqliteWorkflowRunRepositoryAdapterImpl {
    async fn load_workflow(
        &self,
        key: WorkflowLoadKey,
    ) -> Result<Option<WorkflowAggregate>, WorkflowApplicationError> {
        self.blocking(move |connection, capabilities| {
            let (column, value) = match key {
                WorkflowLoadKey::Project(id) => ("project_id", id.as_uuid()),
                WorkflowLoadKey::Workflow(id) => ("workflow_id", id.as_uuid()),
            };
            let sql = format!("SELECT graph_payload FROM workflows WHERE {column} = ?1");
            connection
                .query_row(&sql, [value.as_bytes().as_slice()], |row| row.get::<_, Vec<u8>>(0))
                .optional()
                .map_err(|_| persistence())?
                .map(|bytes| decode_workflow(&bytes, capabilities))
                .transpose()
        })
        .await
    }

    async fn load_workflow_creation_receipt(
        &self,
        request_id: engine::workflow::WorkflowCreateRequestId,
    ) -> Result<Option<WorkflowCreateReceipt>, WorkflowApplicationError> {
        self.blocking(move |connection, capabilities| {
            load_creation_receipt(connection, capabilities, request_id)
        })
        .await
    }

    async fn load_workflow_mutation_receipt(
        &self,
        request_id: WorkflowMutationRequestId,
    ) -> Result<Option<WorkflowMutationReceipt>, WorkflowApplicationError> {
        self.blocking(move |connection, capabilities| {
            load_mutation_receipt(connection, capabilities, request_id)
        })
        .await
    }

    async fn commit_workflow_creation(
        &self,
        commit: WorkflowCreationCommit,
    ) -> Result<WorkflowAggregate, WorkflowApplicationError> {
        self.blocking(move |connection, capabilities| {
            commit_creation(connection, capabilities, commit)
        })
        .await
    }

    async fn commit_workflow_mutation(
        &self,
        commit: WorkflowMutationCommit,
    ) -> Result<WorkflowMutationReceipt, WorkflowApplicationError> {
        self.blocking(move |connection, capabilities| {
            commit_mutation(connection, capabilities, commit)
        })
        .await
    }
}

#[async_trait]
impl WorkflowRunRepositoryInterface for SqliteWorkflowRunRepositoryAdapterImpl {
    async fn load_workflow_run(
        &self,
        key: WorkflowRunLoadKey,
    ) -> Result<Option<engine::workflow::WorkflowRunAggregate>, WorkflowApplicationError> {
        self.blocking(move |connection, _| load_run(connection, key)).await
    }

    async fn load_workflow_run_admission_receipt(
        &self,
        request_id: WorkflowRunRequestId,
    ) -> Result<Option<WorkflowRunAdmissionReceipt>, WorkflowApplicationError> {
        self.blocking(move |connection, _| load_run_receipt(connection, request_id)).await
    }

    async fn admit_workflow_run(
        &self,
        commit: WorkflowRunAdmissionCommit,
    ) -> Result<WorkflowRunAdmissionReceipt, WorkflowApplicationError> {
        self.blocking(move |connection, _| admit_run(connection, commit)).await
    }

    async fn commit_workflow_run_transition(
        &self,
        run: engine::workflow::WorkflowRunAggregate,
        expected_last_event_count: usize,
    ) -> Result<engine::workflow::WorkflowRunAggregate, WorkflowApplicationError> {
        self.blocking(move |connection, _| {
            commit_run_transition(connection, run, expected_last_event_count)
        })
        .await
    }

    async fn list_workflow_run_events_after(
        &self,
        workflow_run_id: WorkflowRunId,
        after_sequence: Option<WorkflowRunEventSequence>,
        limit: usize,
    ) -> Result<Vec<WorkflowRunEvent>, WorkflowApplicationError> {
        self.blocking(move |connection, _| {
            list_run_events(connection, workflow_run_id, after_sequence, limit)
        })
        .await
    }

    async fn load_latest_workflow_run_for_node(
        &self,
        project_id: projects::project::domain::ProjectId,
        workflow_id: engine::workflow_graph::WorkflowId,
        node_id: engine::workflow_graph::WorkflowNodeId,
    ) -> Result<Option<engine::workflow::WorkflowRunAggregate>, WorkflowApplicationError> {
        self.blocking(move |connection, _| {
            load_latest_run_for_node(connection, project_id, workflow_id, node_id)
        })
        .await
    }
}

fn commit_creation(
    connection: &mut Connection,
    capabilities: &WorkflowNodeCapabilityRegistry,
    commit: WorkflowCreationCommit,
) -> Result<WorkflowAggregate, WorkflowApplicationError> {
    let (workflow, receipt) = commit.into_parts();
    let transaction = connection.transaction().map_err(|_| persistence())?;
    if let Some(existing) = load_creation_receipt(&transaction, capabilities, receipt.request_id())?
    {
        if existing.command_hash() == receipt.command_hash() {
            return Ok(existing.created_workflow().clone());
        }
        return Err(WorkflowApplicationError::WorkflowCreationIdempotencyConflict);
    }
    let payload = encode_workflow(&workflow)?;
    transaction
        .execute(
            "INSERT INTO workflows(
                workflow_id, project_id, schema_version, revision, graph_payload, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                workflow.id.as_uuid().as_bytes().as_slice(),
                workflow.project_id.as_uuid().as_bytes().as_slice(),
                workflow.schema_version.get(),
                workflow.revision.get(),
                payload,
                workflow.created_at.as_utc_milliseconds(),
                workflow.updated_at.as_utc_milliseconds(),
            ],
        )
        .map_err(|error| match error {
            rusqlite::Error::SqliteFailure(_, _) => {
                WorkflowApplicationError::WorkflowAlreadyExistsForProject
            }
            _ => persistence(),
        })?;
    insert_creation_receipt(&transaction, &receipt)?;
    transaction.commit().map_err(|_| persistence())?;
    Ok(workflow)
}

fn commit_mutation(
    connection: &mut Connection,
    capabilities: &WorkflowNodeCapabilityRegistry,
    commit: WorkflowMutationCommit,
) -> Result<WorkflowMutationReceipt, WorkflowApplicationError> {
    let (workflow, expected_revision, receipt) = commit.into_parts();
    let transaction = connection.transaction().map_err(|_| persistence())?;
    if let Some(existing) = load_mutation_receipt(&transaction, capabilities, receipt.request_id())?
    {
        if existing.command_hash() == receipt.command_hash() {
            return Ok(existing);
        }
        return Err(WorkflowApplicationError::WorkflowMutationIdempotencyConflict);
    }
    let changed = transaction
        .execute(
            "UPDATE workflows SET revision = ?2, graph_payload = ?3, updated_at = ?4
             WHERE workflow_id = ?1 AND revision = ?5",
            params![
                workflow.id.as_uuid().as_bytes().as_slice(),
                workflow.revision.get(),
                encode_workflow(&workflow)?,
                workflow.updated_at.as_utc_milliseconds(),
                expected_revision.get(),
            ],
        )
        .map_err(|_| persistence())?;
    if changed != 1 {
        return Err(WorkflowApplicationError::WorkflowRevisionConflict);
    }
    insert_mutation_receipt(&transaction, &receipt)?;
    transaction.commit().map_err(|_| persistence())?;
    Ok(receipt)
}

fn persistence() -> WorkflowApplicationError {
    WorkflowApplicationError::WorkflowPersistenceFailure
}
