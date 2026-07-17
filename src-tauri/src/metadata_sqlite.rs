//! Private SQLite foundation for the single Desktop metadata database.

use std::path::Path;
use std::time::Duration;

use rusqlite::{Connection, OpenFlags};
use thiserror::Error;

const APPLICATION_ID: i32 = 0x4f4d4431;
const CURRENT_USER_VERSION: i32 = 2;
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

pub(crate) const BACKEND_SETTINGS_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS desktop_backend_config (
    singleton_id INTEGER PRIMARY KEY NOT NULL CHECK(singleton_id = 1),
    schema_version INTEGER NOT NULL CHECK(schema_version > 0),
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
    /// SQLite rejected an open, configuration, integrity, or migration operation.
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

/// Opens or creates the one metadata database and applies known migrations.
pub(crate) fn open_metadata_sqlite(path: &Path) -> Result<Connection, MetadataSqliteError> {
    let existed_non_empty = path.metadata().map(|metadata| metadata.len() > 0).unwrap_or(false);
    prepare_private_file(path)?;
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
    )
    .map_err(|source| sqlite("open", source))?;
    configure_connection(&connection)?;
    validate_or_initialize_epoch(&connection, existed_non_empty)?;
    verify_integrity(&connection)?;
    apply_migrations(&connection)?;
    restrict_permissions(path)?;
    Ok(connection)
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
            .pragma_update(None, "user_version", 1)
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

fn apply_migrations(connection: &Connection) -> Result<(), MetadataSqliteError> {
    let version = pragma_i32(connection, "user_version")?;
    if version == CURRENT_USER_VERSION {
        return Ok(());
    }
    if version != 1 {
        return Err(MetadataSqliteError::UnsupportedLegacyStorageEpoch);
    }
    let transaction =
        connection.unchecked_transaction().map_err(|source| sqlite("begin migration", source))?;
    transaction
        .execute_batch(BACKEND_SETTINGS_SCHEMA)
        .map_err(|source| sqlite("create backend settings schema", source))?;
    transaction
        .pragma_update(None, "user_version", CURRENT_USER_VERSION)
        .map_err(|source| sqlite("advance schema version", source))?;
    transaction.commit().map_err(|source| sqlite("commit migration", source))
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
mod tests {
    use super::*;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn creates_epoch_one_with_foreign_keys_and_private_permissions() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("metadata.sqlite");
        let connection = open_metadata_sqlite(&path).expect("database opens");

        assert_eq!(pragma_i32(&connection, "application_id").unwrap(), APPLICATION_ID);
        assert_eq!(pragma_i32(&connection, "user_version").unwrap(), CURRENT_USER_VERSION);
        assert_eq!(pragma_i32(&connection, "foreign_keys").unwrap(), 1);
        #[cfg(unix)]
        assert_eq!(path.metadata().unwrap().permissions().mode() & 0o777, 0o600);
    }

    #[test]
    fn refuses_non_empty_database_from_an_unknown_epoch_without_rewriting_it() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("metadata.sqlite");
        let connection = Connection::open(&path).expect("legacy database opens");
        connection.execute("CREATE TABLE legacy(value TEXT)", []).unwrap();
        drop(connection);

        let error = open_metadata_sqlite(&path).unwrap_err();
        assert!(matches!(error, MetadataSqliteError::UnsupportedLegacyStorageEpoch));
        let connection = Connection::open(&path).unwrap();
        let count: i64 = connection
            .query_row("SELECT count(*) FROM sqlite_master WHERE name = 'legacy'", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn refuses_a_newer_schema_version() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("metadata.sqlite");
        let connection = Connection::open(&path).unwrap();
        connection.pragma_update(None, "application_id", APPLICATION_ID).unwrap();
        connection.pragma_update(None, "user_version", CURRENT_USER_VERSION + 1).unwrap();
        drop(connection);

        assert!(matches!(
            open_metadata_sqlite(&path),
            Err(MetadataSqliteError::UnsupportedVersion { found: 3 })
        ));
    }

    #[test]
    fn migrates_epoch_one_to_backend_settings_schema_two() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("metadata.sqlite");
        let connection = Connection::open(&path).unwrap();
        connection.pragma_update(None, "application_id", APPLICATION_ID).unwrap();
        connection.pragma_update(None, "user_version", 1).unwrap();
        drop(connection);

        let connection = open_metadata_sqlite(&path).unwrap();
        assert_eq!(pragma_i32(&connection, "user_version").unwrap(), 2);
        for table in [
            "desktop_backend_config",
            "generation_provider_credentials",
            "assistant_model_credentials",
        ] {
            let count: i64 = connection
                .query_row(
                    "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                    [table],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(count, 1);
        }
    }
}
