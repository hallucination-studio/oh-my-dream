//! Concrete Desktop adapters for Project-owned consumer interfaces.

mod row;
#[cfg(test)]
mod tests;

use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use projects::project::application::{
    ProjectApplicationError, ProjectListCursor, ProjectListPage, ProjectListQuery,
    ProjectMutationOperation, ProjectMutationReceipt, ProjectMutationRequestId,
};
use projects::project::domain::{ProjectAggregate, ProjectId, ProjectRevision, ProjectUpdatedAt};
use projects::project::interfaces::{
    ProjectClockInterface, ProjectIdentityGeneratorInterface, ProjectRepositoryInterface,
};
use rusqlite::{Connection, OptionalExtension, params};

use row::{SqliteProjectMutationReceiptRow, SqliteProjectRow};

const CREATE_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS projects (
    project_id BLOB PRIMARY KEY NOT NULL CHECK(length(project_id) = 16),
    name TEXT NOT NULL,
    revision INTEGER NOT NULL CHECK(revision > 0),
    created_at INTEGER NOT NULL CHECK(created_at >= 0),
    updated_at INTEGER NOT NULL CHECK(updated_at >= created_at)
);
CREATE TABLE IF NOT EXISTS project_mutation_receipts (
    request_id BLOB PRIMARY KEY NOT NULL CHECK(length(request_id) = 16),
    command_hash BLOB NOT NULL CHECK(length(command_hash) = 32),
    operation INTEGER NOT NULL CHECK(operation IN (1, 2)),
    project_id BLOB NOT NULL CHECK(length(project_id) = 16),
    name TEXT NOT NULL,
    revision INTEGER NOT NULL CHECK(revision > 0),
    created_at INTEGER NOT NULL CHECK(created_at >= 0),
    updated_at INTEGER NOT NULL CHECK(updated_at >= created_at),
    result_fingerprint BLOB NOT NULL CHECK(length(result_fingerprint) = 32)
);
CREATE INDEX IF NOT EXISTS projects_list_order
    ON projects(updated_at DESC, project_id DESC);
";

/// SQLite implementation of the Project persistence boundary.
#[derive(Clone)]
pub struct SqliteProjectRepositoryAdapterImpl {
    connection: Arc<Mutex<Connection>>,
}

impl SqliteProjectRepositoryAdapterImpl {
    /// Initializes Project-private tables on an already validated metadata connection.
    pub fn try_new(connection: Arc<Mutex<Connection>>) -> Result<Self, ProjectApplicationError> {
        connection
            .lock()
            .map_err(|_| persistence())?
            .execute_batch(CREATE_SCHEMA)
            .map_err(|_| persistence())?;
        Ok(Self { connection })
    }

    async fn blocking<T: Send + 'static>(
        &self,
        operation: impl FnOnce(&mut Connection) -> Result<T, ProjectApplicationError> + Send + 'static,
    ) -> Result<T, ProjectApplicationError> {
        let connection = Arc::clone(&self.connection);
        tokio::task::spawn_blocking(move || {
            let mut connection = connection.lock().map_err(|_| persistence())?;
            operation(&mut connection)
        })
        .await
        .map_err(|_| persistence())?
    }
}

#[async_trait]
impl ProjectRepositoryInterface for SqliteProjectRepositoryAdapterImpl {
    async fn load_project(
        &self,
        project_id: ProjectId,
    ) -> Result<Option<ProjectAggregate>, ProjectApplicationError> {
        self.blocking(move |connection| load_project(connection, project_id)).await
    }

    async fn list_projects(
        &self,
        query: ProjectListQuery,
    ) -> Result<ProjectListPage, ProjectApplicationError> {
        self.blocking(move |connection| list_projects(connection, query)).await
    }

    async fn load_project_mutation_receipt(
        &self,
        request_id: ProjectMutationRequestId,
    ) -> Result<Option<ProjectMutationReceipt>, ProjectApplicationError> {
        self.blocking(move |connection| load_receipt(connection, request_id)).await
    }

    async fn commit_project_creation(
        &self,
        project: ProjectAggregate,
        receipt: ProjectMutationReceipt,
    ) -> Result<ProjectMutationReceipt, ProjectApplicationError> {
        self.blocking(move |connection| commit_creation(connection, project, receipt)).await
    }

    async fn commit_project_rename(
        &self,
        project: ProjectAggregate,
        expected_revision: ProjectRevision,
        receipt: ProjectMutationReceipt,
    ) -> Result<ProjectMutationReceipt, ProjectApplicationError> {
        self.blocking(move |connection| {
            commit_rename(connection, project, expected_revision, receipt)
        })
        .await
    }
}

/// UTC system-clock implementation of the Project time boundary.
pub struct SystemProjectClockAdapterImpl;

impl ProjectClockInterface for SystemProjectClockAdapterImpl {
    fn observe_project_time(&self) -> Result<ProjectUpdatedAt, ProjectApplicationError> {
        let milliseconds =
            SystemTime::now().duration_since(UNIX_EPOCH).map_err(|_| persistence())?.as_millis();
        let milliseconds = i64::try_from(milliseconds).map_err(|_| persistence())?;
        ProjectUpdatedAt::new(milliseconds).map_err(Into::into)
    }
}

/// Operating-system-random UUIDv4 implementation of the Project identity boundary.
pub struct UuidProjectIdentityGeneratorAdapterImpl;

impl ProjectIdentityGeneratorInterface for UuidProjectIdentityGeneratorAdapterImpl {
    fn generate_project_id(&self) -> ProjectId {
        loop {
            if let Some(project_id) = ProjectId::from_uuid(uuid::Uuid::new_v4()) {
                return project_id;
            }
        }
    }
}

fn load_project(
    connection: &Connection,
    project_id: ProjectId,
) -> Result<Option<ProjectAggregate>, ProjectApplicationError> {
    connection
        .query_row(
            "SELECT project_id, name, revision, created_at, updated_at FROM projects WHERE project_id = ?1",
            [project_id.as_uuid().as_bytes().as_slice()],
            SqliteProjectRow::read,
        )
        .optional()
        .map_err(|_| persistence())?
        .map(SqliteProjectRow::try_into_domain)
        .transpose()
}

fn list_projects(
    connection: &Connection,
    query: ProjectListQuery,
) -> Result<ProjectListPage, ProjectApplicationError> {
    let fetch_limit = i64::from(query.limit.get()) + 1;
    let cursor_time = query.cursor.map(|cursor| cursor.updated_at.get()).unwrap_or(i64::MAX);
    let cursor_id =
        query.cursor.map(|cursor| *cursor.project_id.as_uuid().as_bytes()).unwrap_or([0xff; 16]);
    let mut statement = connection
        .prepare(
            "SELECT project_id, name, revision, created_at, updated_at FROM projects
             WHERE updated_at < ?1 OR (updated_at = ?1 AND project_id < ?2)
             ORDER BY updated_at DESC, project_id DESC LIMIT ?3",
        )
        .map_err(|_| persistence())?;
    let rows = statement
        .query_map(params![cursor_time, cursor_id.as_slice(), fetch_limit], SqliteProjectRow::read)
        .map_err(|_| persistence())?;
    let mut projects = rows
        .map(|row| row.map_err(|_| persistence()).and_then(SqliteProjectRow::try_into_domain))
        .collect::<Result<Vec<_>, _>>()?;
    let has_more = projects.len() > usize::from(query.limit.get());
    projects.truncate(usize::from(query.limit.get()));
    let next_cursor = has_more.then(|| projects.last().map(cursor_from)).flatten();
    Ok(ProjectListPage { projects, next_cursor })
}

fn cursor_from(project: &ProjectAggregate) -> ProjectListCursor {
    ProjectListCursor { updated_at: project.updated_at(), project_id: project.id() }
}

fn load_receipt(
    connection: &Connection,
    request_id: ProjectMutationRequestId,
) -> Result<Option<ProjectMutationReceipt>, ProjectApplicationError> {
    connection
        .query_row(
            "SELECT request_id, command_hash, operation, project_id, name, revision,
                    created_at, updated_at, result_fingerprint
             FROM project_mutation_receipts WHERE request_id = ?1",
            [request_id.as_uuid().as_bytes().as_slice()],
            SqliteProjectMutationReceiptRow::read,
        )
        .optional()
        .map_err(|_| persistence())?
        .map(SqliteProjectMutationReceiptRow::try_into_domain)
        .transpose()
}

fn commit_creation(
    connection: &mut Connection,
    project: ProjectAggregate,
    receipt: ProjectMutationReceipt,
) -> Result<ProjectMutationReceipt, ProjectApplicationError> {
    let transaction = connection.transaction().map_err(|_| persistence())?;
    if let Some(replayed) = replay(&transaction, &receipt)? {
        return Ok(replayed);
    }
    validate_receipt(&project, &receipt, ProjectMutationOperation::Create)?;
    SqliteProjectRow::from_domain(&project)?.insert(&transaction)?;
    SqliteProjectMutationReceiptRow::from_domain(&receipt)?.insert(&transaction)?;
    transaction.commit().map_err(|_| persistence())?;
    Ok(receipt)
}

fn commit_rename(
    connection: &mut Connection,
    project: ProjectAggregate,
    expected_revision: ProjectRevision,
    receipt: ProjectMutationReceipt,
) -> Result<ProjectMutationReceipt, ProjectApplicationError> {
    let transaction = connection.transaction().map_err(|_| persistence())?;
    if let Some(replayed) = replay(&transaction, &receipt)? {
        return Ok(replayed);
    }
    validate_receipt(&project, &receipt, ProjectMutationOperation::Rename)?;
    let current = load_project(&transaction, project.id())?
        .ok_or(ProjectApplicationError::ProjectNotFound { project_id: project.id() })?;
    if current.revision() != expected_revision {
        return Err(ProjectApplicationError::ProjectRevisionConflict {
            project_id: project.id(),
            expected_revision,
            actual_revision: current.revision(),
        });
    }
    let changed =
        SqliteProjectRow::from_domain(&project)?.update(&transaction, expected_revision)?;
    if changed != 1 {
        return Err(persistence());
    }
    SqliteProjectMutationReceiptRow::from_domain(&receipt)?.insert(&transaction)?;
    transaction.commit().map_err(|_| persistence())?;
    Ok(receipt)
}

fn replay(
    connection: &Connection,
    proposed: &ProjectMutationReceipt,
) -> Result<Option<ProjectMutationReceipt>, ProjectApplicationError> {
    let Some(existing) = load_receipt(connection, proposed.request_id())? else {
        return Ok(None);
    };
    if existing.command_hash() != proposed.command_hash() {
        return Err(ProjectApplicationError::ProjectMutationIdempotencyConflict {
            request_id: proposed.request_id(),
        });
    }
    Ok(Some(existing))
}

fn validate_receipt(
    project: &ProjectAggregate,
    receipt: &ProjectMutationReceipt,
    operation: ProjectMutationOperation,
) -> Result<(), ProjectApplicationError> {
    if receipt.operation() == operation && receipt.outcome().project() == project {
        Ok(())
    } else {
        Err(persistence())
    }
}

fn persistence() -> ProjectApplicationError {
    ProjectApplicationError::ProjectPersistenceFailure
}
