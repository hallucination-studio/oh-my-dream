use super::{ReviewReceipt, ReviewedChangeError, ReviewedChangeRepository, WorkflowCandidate};
use rusqlite::{Connection, OptionalExtension, params};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

const DATABASE_FILE: &str = "reviewed_change.sqlite";

pub struct ReviewedChangeSqliteRepository {
    connection: Mutex<Connection>,
}

impl ReviewedChangeSqliteRepository {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, ReviewedChangeError> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(storage)?;
        }
        let connection = Connection::open(path).map_err(storage)?;
        connection.busy_timeout(Duration::from_secs(5)).map_err(storage)?;
        connection
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS workflow_candidates (
                    candidate_id TEXT PRIMARY KEY NOT NULL,
                    candidate_json TEXT NOT NULL,
                    created_at INTEGER NOT NULL
                );
                CREATE TABLE IF NOT EXISTS review_receipts (
                    receipt_id TEXT PRIMARY KEY NOT NULL,
                    receipt_json TEXT NOT NULL,
                    created_at INTEGER NOT NULL
                );",
            )
            .map_err(storage)?;
        Ok(Self { connection: Mutex::new(connection) })
    }

    #[must_use]
    pub fn path(config_root: impl AsRef<Path>) -> PathBuf {
        config_root.as_ref().join(DATABASE_FILE)
    }
}

impl ReviewedChangeRepository for ReviewedChangeSqliteRepository {
    fn insert(&self, candidate: &WorkflowCandidate) -> Result<(), ReviewedChangeError> {
        let json = serde_json::to_string(candidate).map_err(storage)?;
        self.connection
            .lock()
            .map_err(|_| {
                ReviewedChangeError::Storage("candidate database lock poisoned".to_owned())
            })?
            .execute(
                "INSERT INTO workflow_candidates (candidate_id, candidate_json, created_at)
                 VALUES (?1, ?2, unixepoch())",
                params![candidate.id(), json],
            )
            .map(|_| ())
            .map_err(storage)
    }

    fn get(&self, candidate_id: &str) -> Result<Option<WorkflowCandidate>, ReviewedChangeError> {
        let connection = self.connection.lock().map_err(|_| {
            ReviewedChangeError::Storage("candidate database lock poisoned".to_owned())
        })?;
        let json = connection
            .query_row(
                "SELECT candidate_json FROM workflow_candidates WHERE candidate_id = ?1",
                [candidate_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(storage)?;
        json.map(|value| serde_json::from_str(&value).map_err(storage)).transpose()
    }

    fn insert_receipt(&self, receipt: &ReviewReceipt) -> Result<(), ReviewedChangeError> {
        let json = serde_json::to_string(receipt).map_err(storage)?;
        self.connection
            .lock()
            .map_err(|_| {
                ReviewedChangeError::Storage("candidate database lock poisoned".to_owned())
            })?
            .execute(
                "INSERT INTO review_receipts (receipt_id, receipt_json, created_at)
                 VALUES (?1, ?2, unixepoch())",
                params![receipt.id(), json],
            )
            .map(|_| ())
            .map_err(storage)
    }

    fn get_receipt(&self, receipt_id: &str) -> Result<Option<ReviewReceipt>, ReviewedChangeError> {
        let connection = self.connection.lock().map_err(|_| {
            ReviewedChangeError::Storage("candidate database lock poisoned".to_owned())
        })?;
        let json = connection
            .query_row(
                "SELECT receipt_json FROM review_receipts WHERE receipt_id = ?1",
                [receipt_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(storage)?;
        json.map(|value| serde_json::from_str(&value).map_err(storage)).transpose()
    }
}

fn storage(error: impl std::fmt::Display) -> ReviewedChangeError {
    ReviewedChangeError::Storage(error.to_string())
}
