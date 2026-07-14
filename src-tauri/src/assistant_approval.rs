//! Durable SDK RunState waiting for one human approval decision.

use crate::assistant_runtime::AssistantWaitingApproval;
use rusqlite::{Connection, ErrorCode, OptionalExtension, params};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use thiserror::Error;

pub trait PendingApprovalRepository: Send + Sync {
    fn save(&self, waiting: &AssistantWaitingApproval) -> Result<(), PendingApprovalError>;
    fn load(
        &self,
        session_id: &str,
    ) -> Result<Option<AssistantWaitingApproval>, PendingApprovalError>;
    fn delete(&self, session_id: &str) -> Result<(), PendingApprovalError>;
}

pub struct PendingApprovalService {
    repository: Arc<dyn PendingApprovalRepository>,
}

impl PendingApprovalService {
    #[must_use]
    pub fn new(repository: Arc<dyn PendingApprovalRepository>) -> Self {
        Self { repository }
    }

    pub fn save(&self, waiting: &AssistantWaitingApproval) -> Result<(), PendingApprovalError> {
        self.repository.save(waiting)
    }

    pub fn load(
        &self,
        session_id: &str,
    ) -> Result<Option<AssistantWaitingApproval>, PendingApprovalError> {
        self.repository.load(session_id)
    }

    pub fn delete(&self, session_id: &str) -> Result<(), PendingApprovalError> {
        self.repository.delete(session_id)
    }
}

pub struct PendingApprovalSqliteRepository {
    connection: Mutex<Connection>,
}

impl PendingApprovalSqliteRepository {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, PendingApprovalError> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(storage)?;
        }
        let connection = Connection::open(path).map_err(storage)?;
        connection
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS pending_assistant_approvals (
                    session_id TEXT PRIMARY KEY NOT NULL,
                    waiting_json TEXT NOT NULL,
                    created_at INTEGER NOT NULL
                );",
            )
            .map_err(storage)?;
        Ok(Self { connection: Mutex::new(connection) })
    }

    #[must_use]
    pub fn path(config_root: impl AsRef<Path>) -> PathBuf {
        config_root.as_ref().join("assistant_approval.sqlite")
    }
}

impl PendingApprovalRepository for PendingApprovalSqliteRepository {
    fn save(&self, waiting: &AssistantWaitingApproval) -> Result<(), PendingApprovalError> {
        let json = serde_json::to_string(waiting).map_err(storage)?;
        self.connection
            .lock()
            .map_err(|_| lock_error())?
            .execute(
                "INSERT INTO pending_assistant_approvals
                 (session_id, waiting_json, created_at) VALUES (?1, ?2, unixepoch())",
                params![waiting.session_id(), json],
            )
            .map(|_| ())
            .map_err(|error| approval_insert_error(error, waiting.session_id()))
    }

    fn load(
        &self,
        session_id: &str,
    ) -> Result<Option<AssistantWaitingApproval>, PendingApprovalError> {
        let connection = self.connection.lock().map_err(|_| lock_error())?;
        let json = connection
            .query_row(
                "SELECT waiting_json FROM pending_assistant_approvals WHERE session_id = ?1",
                [session_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(storage)?;
        json.map(|value| serde_json::from_str(&value).map_err(storage)).transpose()
    }

    fn delete(&self, session_id: &str) -> Result<(), PendingApprovalError> {
        self.connection
            .lock()
            .map_err(|_| lock_error())?
            .execute("DELETE FROM pending_assistant_approvals WHERE session_id = ?1", [session_id])
            .map(|_| ())
            .map_err(storage)
    }
}

#[derive(Debug, Error)]
pub enum PendingApprovalError {
    #[error("pending approval already exists for session {0}")]
    AlreadyExists(String),
    #[error("pending approval storage failed: {0}")]
    Storage(String),
}

fn approval_insert_error(error: rusqlite::Error, session_id: &str) -> PendingApprovalError {
    if matches!(
        error,
        rusqlite::Error::SqliteFailure(ref details, _)
            if details.code == ErrorCode::ConstraintViolation
    ) {
        return PendingApprovalError::AlreadyExists(session_id.to_owned());
    }
    PendingApprovalError::Storage(error.to_string())
}

fn storage(error: impl std::fmt::Display) -> PendingApprovalError {
    PendingApprovalError::Storage(error.to_string())
}

fn lock_error() -> PendingApprovalError {
    PendingApprovalError::Storage("pending approval database lock poisoned".to_owned())
}
