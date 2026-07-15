use crate::workflow_authority::{
    WorkflowAuthorityError, WorkflowCommitRequest, WorkflowCommitResult, WorkflowHead,
    WorkflowRepository,
};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

const WORKFLOW_DATABASE_FILE: &str = "workflow.sqlite";

/// SQLite adapter for the Workflow authority storage boundary.
pub(crate) struct WorkflowSqliteRepository {
    connection: Mutex<Connection>,
}

impl WorkflowSqliteRepository {
    /// Opens and migrates the dedicated Workflow authority database.
    pub(crate) fn open(path: impl AsRef<Path>) -> Result<Self, WorkflowAuthorityError> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| storage(error.to_string()))?;
        }
        let connection = Connection::open(path).map_err(|error| storage(error.to_string()))?;
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(|error| storage(error.to_string()))?;
        let repository = Self { connection: Mutex::new(connection) };
        repository.migrate()?;
        Ok(repository)
    }

    /// Returns the default authority database path under an application config root.
    pub(crate) fn path(config_root: impl AsRef<Path>) -> std::path::PathBuf {
        config_root.as_ref().join(WORKFLOW_DATABASE_FILE)
    }

    fn migrate(&self) -> Result<(), WorkflowAuthorityError> {
        let connection = self.connection.lock().map_err(|_| lock_error())?;
        connection
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS workflow_heads (
                    project_id TEXT PRIMARY KEY NOT NULL,
                    revision INTEGER NOT NULL,
                    workflow_json TEXT NOT NULL,
                    updated_at INTEGER NOT NULL
                );
                CREATE TABLE IF NOT EXISTS workflow_receipts (
                    project_id TEXT NOT NULL,
                    request_id TEXT NOT NULL,
                    request_hash TEXT NOT NULL,
                    outcome_json TEXT NOT NULL,
                    created_at INTEGER NOT NULL,
                    PRIMARY KEY (project_id, request_id)
                );
                CREATE TABLE IF NOT EXISTS workflow_undo (
                    undo_id TEXT PRIMARY KEY NOT NULL,
                    project_id TEXT NOT NULL,
                    revision INTEGER NOT NULL,
                    request_id TEXT NOT NULL,
                    previous_workflow_json TEXT,
                    created_at INTEGER NOT NULL,
                    UNIQUE (project_id, revision)
                );",
            )
            .map_err(|error| storage(format!("migrate Workflow authority database: {error}")))
    }
}

impl WorkflowRepository for WorkflowSqliteRepository {
    fn load_head(&self, project_id: &str) -> Result<Option<WorkflowHead>, WorkflowAuthorityError> {
        let connection = self.connection.lock().map_err(|_| lock_error())?;
        let row = connection
            .query_row(
                "SELECT project_id, revision, workflow_json
                 FROM workflow_heads WHERE project_id = ?1",
                [project_id],
                |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?, row.get::<_, String>(2)?))
                },
            )
            .optional()
            .map_err(|error| storage(format!("load Workflow head: {error}")))?;
        row.map(decode_head).transpose()
    }

    fn load_receipt(
        &self,
        project_id: &str,
        request_id: &str,
        request_hash: &str,
    ) -> Result<Option<WorkflowCommitResult>, WorkflowAuthorityError> {
        let connection = self.connection.lock().map_err(|_| lock_error())?;
        existing_receipt_values(&connection, project_id, request_id, request_hash)
    }

    fn commit(
        &self,
        request: &WorkflowCommitRequest,
    ) -> Result<WorkflowCommitResult, WorkflowAuthorityError> {
        let workflow_json = serialize_workflow(&request.workflow)?;
        let mut connection = self.connection.lock().map_err(|_| lock_error())?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| storage(format!("begin Workflow transaction: {error}")))?;
        if let Some(result) = existing_receipt(&transaction, request)? {
            transaction
                .commit()
                .map_err(|error| storage(format!("commit Workflow receipt read: {error}")))?;
            return Ok(result.mark_deduplicated());
        }

        let current = load_head_in_transaction(&transaction, &request.project_id)?;
        check_revision(request.expected_revision, current.as_ref())?;
        let result = mutation_result(current.as_ref(), request, &workflow_json)?;
        if result.changed {
            persist_change(&transaction, request, &result, &workflow_json, current.as_ref())?;
        }
        insert_receipt(&transaction, request, &result)?;
        transaction
            .commit()
            .map_err(|error| storage(format!("commit Workflow mutation: {error}")))?;
        Ok(result)
    }
}

fn existing_receipt(
    transaction: &Transaction<'_>,
    request: &WorkflowCommitRequest,
) -> Result<Option<WorkflowCommitResult>, WorkflowAuthorityError> {
    existing_receipt_values(
        transaction,
        &request.project_id,
        &request.request_id,
        &request.request_hash,
    )
}

fn existing_receipt_values(
    connection: &Connection,
    project_id: &str,
    request_id: &str,
    expected_hash: &str,
) -> Result<Option<WorkflowCommitResult>, WorkflowAuthorityError> {
    let row = connection
        .query_row(
            "SELECT request_hash, outcome_json FROM workflow_receipts
             WHERE project_id = ?1 AND request_id = ?2",
            params![project_id, request_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(|error| storage(format!("read Workflow receipt: {error}")))?;
    let Some((request_hash, outcome_json)) = row else {
        return Ok(None);
    };
    if request_hash != expected_hash {
        return Err(WorkflowAuthorityError::RequestHashMismatch {
            request_id: request_id.to_owned(),
        });
    }
    let result = serde_json::from_str(&outcome_json).map_err(|error| corrupt(error.to_string()))?;
    Ok(Some(result))
}

fn load_head_in_transaction(
    transaction: &Transaction<'_>,
    project_id: &str,
) -> Result<Option<WorkflowHead>, WorkflowAuthorityError> {
    let row = transaction
        .query_row(
            "SELECT project_id, revision, workflow_json
             FROM workflow_heads WHERE project_id = ?1",
            [project_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?, row.get::<_, String>(2)?)),
        )
        .optional()
        .map_err(|error| storage(format!("load Workflow head in transaction: {error}")))?;
    row.map(decode_head).transpose()
}

fn check_revision(
    expected: Option<u64>,
    current: Option<&WorkflowHead>,
) -> Result<(), WorkflowAuthorityError> {
    let actual = current.map(|head| head.revision);
    if expected == actual {
        return Ok(());
    }
    Err(WorkflowAuthorityError::RevisionConflict { expected, actual })
}

fn mutation_result(
    current: Option<&WorkflowHead>,
    request: &WorkflowCommitRequest,
    workflow_json: &str,
) -> Result<WorkflowCommitResult, WorkflowAuthorityError> {
    if current.is_none() && request.workflow.nodes.is_empty() {
        return Ok(WorkflowCommitResult {
            head: None,
            changed: false,
            deduplicated: false,
            undo_id: None,
        });
    }
    if let Some(current) = current
        && serialize_workflow(&current.workflow)? == workflow_json
    {
        return Ok(WorkflowCommitResult {
            head: Some(current.clone()),
            changed: false,
            deduplicated: false,
            undo_id: None,
        });
    }
    let revision = current
        .map(|head| head.revision)
        .unwrap_or_default()
        .checked_add(1)
        .ok_or(WorkflowAuthorityError::RevisionOverflow)?;
    let undo_id = format!("workflow:{}:{revision}", request.project_id);
    Ok(WorkflowCommitResult {
        head: Some(WorkflowHead {
            project_id: request.project_id.clone(),
            revision,
            workflow: request.workflow.clone(),
        }),
        changed: true,
        deduplicated: false,
        undo_id: Some(undo_id),
    })
}

fn persist_change(
    transaction: &Transaction<'_>,
    request: &WorkflowCommitRequest,
    result: &WorkflowCommitResult,
    workflow_json: &str,
    current: Option<&WorkflowHead>,
) -> Result<(), WorkflowAuthorityError> {
    let head = result.head.as_ref().ok_or_else(|| corrupt("changed result has no head"))?;
    let revision = sqlite_revision(head.revision)?;
    transaction
        .execute(
            "INSERT INTO workflow_heads (project_id, revision, workflow_json, updated_at)
             VALUES (?1, ?2, ?3, unixepoch())
             ON CONFLICT(project_id) DO UPDATE SET
                revision = excluded.revision,
                workflow_json = excluded.workflow_json,
                updated_at = excluded.updated_at",
            params![request.project_id, revision, workflow_json],
        )
        .map_err(|error| storage(format!("persist Workflow head: {error}")))?;
    let previous_json = current.map(|head| serialize_workflow(&head.workflow)).transpose()?;
    transaction
        .execute(
            "INSERT INTO workflow_undo
             (undo_id, project_id, revision, request_id, previous_workflow_json, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, unixepoch())",
            params![
                head_undo_id(result)?,
                request.project_id,
                revision,
                request.request_id,
                previous_json,
            ],
        )
        .map_err(|error| storage(format!("persist Workflow undo journal: {error}")))?;
    Ok(())
}

fn insert_receipt(
    transaction: &Transaction<'_>,
    request: &WorkflowCommitRequest,
    result: &WorkflowCommitResult,
) -> Result<(), WorkflowAuthorityError> {
    let outcome_json = serde_json::to_string(result).map_err(|error| corrupt(error.to_string()))?;
    transaction
        .execute(
            "INSERT INTO workflow_receipts
             (project_id, request_id, request_hash, outcome_json, created_at)
             VALUES (?1, ?2, ?3, ?4, unixepoch())",
            params![request.project_id, request.request_id, request.request_hash, outcome_json],
        )
        .map_err(|error| storage(format!("persist Workflow receipt: {error}")))?;
    Ok(())
}

fn head_undo_id(result: &WorkflowCommitResult) -> Result<&str, WorkflowAuthorityError> {
    result.undo_id.as_deref().ok_or_else(|| corrupt("changed result has no undo id"))
}

fn serialize_workflow(workflow: &engine::Workflow) -> Result<String, WorkflowAuthorityError> {
    serde_json::to_string(workflow).map_err(|error| corrupt(error.to_string()))
}

fn decode_head(
    (project_id, revision, workflow_json): (String, i64, String),
) -> Result<WorkflowHead, WorkflowAuthorityError> {
    let revision = u64::try_from(revision).map_err(|error| corrupt(error.to_string()))?;
    let workflow: engine::Workflow =
        serde_json::from_str(&workflow_json).map_err(|error| corrupt(error.to_string()))?;
    if workflow.project_id != project_id {
        return Err(corrupt("head project does not match its Workflow"));
    }
    Ok(WorkflowHead { project_id, revision, workflow })
}

fn sqlite_revision(revision: u64) -> Result<i64, WorkflowAuthorityError> {
    i64::try_from(revision).map_err(|error| corrupt(error.to_string()))
}

fn storage(message: impl Into<String>) -> WorkflowAuthorityError {
    WorkflowAuthorityError::Storage { message: message.into() }
}

fn corrupt(message: impl Into<String>) -> WorkflowAuthorityError {
    WorkflowAuthorityError::CorruptData { message: message.into() }
}

fn lock_error() -> WorkflowAuthorityError {
    storage("Workflow repository lock was poisoned")
}
