//! Private SQLite foundation for the single Desktop metadata database.

use std::path::Path;
use std::time::Duration;

use rusqlite::{Connection, OpenFlags};
use thiserror::Error;

const APPLICATION_ID: i32 = 0x4f4d4432;
const CURRENT_USER_VERSION: i32 = 1;
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

pub(crate) const BACKEND_SETTINGS_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS desktop_backend_config (
    singleton_id INTEGER PRIMARY KEY NOT NULL CHECK(singleton_id = 1),
    schema_version INTEGER NOT NULL CHECK(schema_version > 0),
    revision INTEGER NOT NULL DEFAULT 1 CHECK(revision > 0),
    config_json BLOB NOT NULL CHECK(length(config_json) BETWEEN 1 AND 262144)
);
CREATE TABLE IF NOT EXISTS generation_provider_credentials (
    credential_id TEXT PRIMARY KEY NOT NULL,
    secret BLOB NOT NULL CHECK(length(secret) BETWEEN 1 AND 16384)
);
CREATE TABLE IF NOT EXISTS assistant_model_credentials (
    credential_id TEXT PRIMARY KEY NOT NULL,
    secret BLOB NOT NULL CHECK(length(secret) BETWEEN 1 AND 16384)
);
";

/// Derives the one database beside the private configuration directory.
pub(crate) fn metadata_sqlite_path(config_root: &Path) -> std::path::PathBuf {
    config_root.with_file_name("metadata.sqlite")
}

/// Safe failure categories while opening the private metadata database.
#[derive(Debug, Error)]
pub(crate) enum MetadataSqliteError {
    /// A non-empty database is not owned by the hard-cut storage epoch.
    #[error("unsupported legacy storage epoch")]
    UnsupportedLegacyStorageEpoch,
    /// The database was created by a newer unsupported application version.
    #[error("unsupported metadata schema version {found}")]
    UnsupportedVersion { found: i32 },
    /// SQLite rejected an open, configuration, integrity, or schema operation.
    #[error("metadata storage unavailable during {operation}")]
    Sqlite {
        operation: &'static str,
        #[source]
        source: rusqlite::Error,
    },
    /// The database failed SQLite's bounded integrity check.
    #[error("metadata storage is corrupt")]
    Corruption,
    /// Private file permissions could not be established.
    #[error("metadata storage permission denied")]
    PermissionDenied,
}

/// Opens or creates the metadata database for the current hard-cut epoch.
pub(crate) fn open_metadata_sqlite(path: &Path) -> Result<Connection, MetadataSqliteError> {
    let existed_non_empty = path.metadata().map(|metadata| metadata.len() > 0).unwrap_or(false);
    prepare_private_file(path)?;
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
    )
    .map_err(|source| sqlite("open", source))?;
    configure_connection(&connection)?;
    acquire_process_lock(&connection)?;
    validate_or_initialize_epoch(&connection, existed_non_empty)?;
    verify_integrity(&connection)?;
    create_current_schema(&connection)?;
    restrict_permissions(path)?;
    Ok(connection)
}

fn acquire_process_lock(connection: &Connection) -> Result<(), MetadataSqliteError> {
    connection
        .pragma_update(None, "locking_mode", "EXCLUSIVE")
        .map_err(|source| sqlite("configure exclusive process lock", source))?;
    connection
        .execute_batch("BEGIN EXCLUSIVE; COMMIT;")
        .map_err(|source| sqlite("acquire exclusive process lock", source))
}

#[cfg(unix)]
fn prepare_private_file(path: &Path) -> Result<(), MetadataSqliteError> {
    use std::os::unix::fs::OpenOptionsExt;
    if path.exists() {
        return Ok(());
    }
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map(|_| ())
        .map_err(|_| MetadataSqliteError::PermissionDenied)
}

#[cfg(not(unix))]
fn prepare_private_file(path: &Path) -> Result<(), MetadataSqliteError> {
    if path.exists() {
        return Ok(());
    }
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map(|_| ())
        .map_err(|_| MetadataSqliteError::PermissionDenied)
}

fn configure_connection(connection: &Connection) -> Result<(), MetadataSqliteError> {
    connection
        .busy_timeout(BUSY_TIMEOUT)
        .map_err(|source| sqlite("configure busy timeout", source))?;
    connection
        .pragma_update(None, "foreign_keys", true)
        .map_err(|source| sqlite("enable foreign keys", source))
}

fn validate_or_initialize_epoch(
    connection: &Connection,
    existed_non_empty: bool,
) -> Result<(), MetadataSqliteError> {
    let application_id = pragma_i32(connection, "application_id")?;
    let user_version = pragma_i32(connection, "user_version")?;
    if existed_non_empty && application_id != APPLICATION_ID {
        return Err(MetadataSqliteError::UnsupportedLegacyStorageEpoch);
    }
    if user_version > CURRENT_USER_VERSION {
        return Err(MetadataSqliteError::UnsupportedVersion { found: user_version });
    }
    if !existed_non_empty {
        let transaction =
            connection.unchecked_transaction().map_err(|source| sqlite("begin epoch", source))?;
        transaction
            .pragma_update(None, "application_id", APPLICATION_ID)
            .map_err(|source| sqlite("write application ID", source))?;
        transaction
            .pragma_update(None, "user_version", CURRENT_USER_VERSION)
            .map_err(|source| sqlite("write schema version", source))?;
        transaction.commit().map_err(|source| sqlite("commit epoch", source))?;
    }
    Ok(())
}

fn verify_integrity(connection: &Connection) -> Result<(), MetadataSqliteError> {
    let result: String = connection
        .query_row("PRAGMA quick_check(1)", [], |row| row.get(0))
        .map_err(|source| sqlite("check integrity", source))?;
    if result == "ok" { Ok(()) } else { Err(MetadataSqliteError::Corruption) }
}

fn create_current_schema(connection: &Connection) -> Result<(), MetadataSqliteError> {
    let version = pragma_i32(connection, "user_version")?;
    if version != CURRENT_USER_VERSION {
        return Err(MetadataSqliteError::UnsupportedLegacyStorageEpoch);
    }
    let transaction =
        connection.unchecked_transaction().map_err(|source| sqlite("begin schema", source))?;
    transaction
        .execute_batch(BACKEND_SETTINGS_SCHEMA)
        .map_err(|source| sqlite("create backend settings schema", source))?;
    transaction.commit().map_err(|source| sqlite("commit schema", source))
}

fn pragma_i32(connection: &Connection, name: &'static str) -> Result<i32, MetadataSqliteError> {
    connection
        .pragma_query_value(None, name, |row| row.get(0))
        .map_err(|source| sqlite("read schema metadata", source))
}

fn sqlite(operation: &'static str, source: rusqlite::Error) -> MetadataSqliteError {
    MetadataSqliteError::Sqlite { operation, source }
}

#[cfg(unix)]
fn restrict_permissions(path: &Path) -> Result<(), MetadataSqliteError> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
        .map_err(|_| MetadataSqliteError::PermissionDenied)
}

#[cfg(not(unix))]
fn restrict_permissions(_path: &Path) -> Result<(), MetadataSqliteError> {
    Ok(())
}

#[cfg(test)]
#[path = "metadata_sqlite/tests.rs"]
mod tests;
