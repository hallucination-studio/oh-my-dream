use super::*;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[test]
fn creates_and_reopens_the_current_hard_cut_epoch() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("metadata.sqlite");

    let connection = open_metadata_sqlite(&path).unwrap();
    assert_eq!(pragma_i32(&connection, "application_id").unwrap(), APPLICATION_ID);
    assert_eq!(pragma_i32(&connection, "user_version").unwrap(), CURRENT_USER_VERSION);
    assert_eq!(pragma_i32(&connection, "foreign_keys").unwrap(), 1);
    #[cfg(unix)]
    assert_eq!(path.metadata().unwrap().permissions().mode() & 0o777, 0o600);
    drop(connection);

    let reopened = open_metadata_sqlite(&path).unwrap();
    for table in
        ["desktop_backend_config", "generation_provider_credentials", "assistant_model_credentials"]
    {
        let count: i64 = reopened
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                [table],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }
}

#[test]
fn holds_exclusive_process_lock_for_connection_lifetime() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("metadata.sqlite");
    let owner = open_metadata_sqlite(&path).unwrap();
    let contender = Connection::open(&path).unwrap();
    contender.busy_timeout(Duration::ZERO).unwrap();

    assert!(contender.execute_batch("BEGIN IMMEDIATE; ROLLBACK;").is_err());
    drop(owner);
    contender.execute_batch("BEGIN IMMEDIATE; ROLLBACK;").unwrap();
}

#[test]
fn refuses_a_prior_epoch_without_mutating_it() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("metadata.sqlite");
    let connection = Connection::open(&path).unwrap();
    connection.pragma_update(None, "application_id", 0x4f4d4431).unwrap();
    connection.pragma_update(None, "user_version", 2).unwrap();
    connection.execute("CREATE TABLE legacy(value TEXT)", []).unwrap();
    connection.execute("INSERT INTO legacy VALUES ('preserve-me')", []).unwrap();
    drop(connection);

    assert!(matches!(
        open_metadata_sqlite(&path),
        Err(MetadataSqliteError::UnsupportedLegacyStorageEpoch)
    ));

    let connection = Connection::open(&path).unwrap();
    let value: String =
        connection.query_row("SELECT value FROM legacy", [], |row| row.get(0)).unwrap();
    assert_eq!(value, "preserve-me");
    assert_eq!(pragma_i32(&connection, "application_id").unwrap(), 0x4f4d4431);
    assert_eq!(pragma_i32(&connection, "user_version").unwrap(), 2);
}

#[test]
fn refuses_a_newer_version_in_the_current_epoch_without_mutating_it() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("metadata.sqlite");
    let connection = Connection::open(&path).unwrap();
    connection.pragma_update(None, "application_id", APPLICATION_ID).unwrap();
    connection.pragma_update(None, "user_version", CURRENT_USER_VERSION + 1).unwrap();
    connection.execute("CREATE TABLE future(value TEXT)", []).unwrap();
    drop(connection);

    assert!(matches!(
        open_metadata_sqlite(&path),
        Err(MetadataSqliteError::UnsupportedVersion { found: 2 })
    ));

    let connection = Connection::open(&path).unwrap();
    let count: i64 = connection
        .query_row("SELECT count(*) FROM sqlite_master WHERE name = 'future'", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 1);
}
