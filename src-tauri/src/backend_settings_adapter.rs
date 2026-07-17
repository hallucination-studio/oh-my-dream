use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use rusqlite::{Connection, ErrorCode, OptionalExtension, TransactionBehavior, params};

use crate::{
    credential_repository::{
        AssistantModelCredentialId, AssistantModelCredentialRepositoryError,
        AssistantModelCredentialRepositoryInterface, AssistantModelCredentialSecret,
        GenerationProviderCredentialId, GenerationProviderCredentialRepositoryError,
        GenerationProviderCredentialRepositoryInterface, GenerationProviderCredentialSecret,
    },
    desktop_backend_config::{
        DesktopBackendConfig, DesktopBackendConfigRepositoryError,
        DesktopBackendConfigRepositoryInterface,
    },
};

const CONFIG_SCHEMA_VERSION: i64 = 1;

#[derive(Clone)]
pub struct SqliteDesktopBackendSettingsAdapterImpl {
    connection: Arc<Mutex<Connection>>,
}

impl SqliteDesktopBackendSettingsAdapterImpl {
    pub fn try_new(
        connection: Arc<Mutex<Connection>>,
    ) -> Result<Self, DesktopBackendConfigRepositoryError> {
        connection
            .lock()
            .map_err(|_| DesktopBackendConfigRepositoryError::Unavailable)?
            .execute_batch(crate::metadata_sqlite::BACKEND_SETTINGS_SCHEMA)
            .map_err(config_sqlite_error)?;
        Ok(Self { connection })
    }

    async fn blocking<T: Send + 'static>(
        &self,
        operation: impl FnOnce(&mut Connection) -> T + Send + 'static,
    ) -> Result<T, DesktopBackendConfigRepositoryError> {
        let connection = Arc::clone(&self.connection);
        tokio::task::spawn_blocking(move || {
            let mut connection =
                connection.lock().map_err(|_| DesktopBackendConfigRepositoryError::Unavailable)?;
            Ok(operation(&mut connection))
        })
        .await
        .map_err(|_| DesktopBackendConfigRepositoryError::Unavailable)?
    }
}

#[async_trait]
impl DesktopBackendConfigRepositoryInterface for SqliteDesktopBackendSettingsAdapterImpl {
    async fn load_or_initialize_desktop_backend_config(
        &self,
    ) -> Result<DesktopBackendConfig, DesktopBackendConfigRepositoryError> {
        self.blocking(load_or_initialize_config).await?
    }

    async fn save_desktop_backend_config(
        &self,
        config: DesktopBackendConfig,
    ) -> Result<(), DesktopBackendConfigRepositoryError> {
        let encoded = config
            .canonical_json()
            .map_err(|_| DesktopBackendConfigRepositoryError::InvalidConfig)?;
        self.blocking(move |connection| {
            connection
                .execute(
                    "INSERT INTO desktop_backend_config(singleton_id, schema_version, config_json)
                     VALUES (1, ?1, ?2)
                     ON CONFLICT(singleton_id) DO UPDATE SET
                     schema_version = excluded.schema_version,
                     config_json = excluded.config_json",
                    params![CONFIG_SCHEMA_VERSION, encoded],
                )
                .map(|_| ())
                .map_err(config_sqlite_error)
        })
        .await?
    }
}

#[async_trait]
impl GenerationProviderCredentialRepositoryInterface for SqliteDesktopBackendSettingsAdapterImpl {
    async fn save_generation_provider_credential(
        &self,
        id: GenerationProviderCredentialId,
        secret: GenerationProviderCredentialSecret,
    ) -> Result<(), GenerationProviderCredentialRepositoryError> {
        save_credential(
            &self.connection,
            CredentialTable::GenerationProvider,
            id.as_str().to_owned(),
            secret.as_bytes().to_vec(),
        )
        .await
        .map_err(generation_error)
    }

    async fn load_generation_provider_credential(
        &self,
        id: &GenerationProviderCredentialId,
    ) -> Result<GenerationProviderCredentialSecret, GenerationProviderCredentialRepositoryError>
    {
        let bytes = load_credential(
            &self.connection,
            CredentialTable::GenerationProvider,
            id.as_str().to_owned(),
        )
        .await
        .map_err(generation_error)?;
        GenerationProviderCredentialSecret::new(bytes)
    }

    async fn delete_generation_provider_credential(
        &self,
        id: &GenerationProviderCredentialId,
    ) -> Result<(), GenerationProviderCredentialRepositoryError> {
        delete_credential(
            &self.connection,
            CredentialTable::GenerationProvider,
            id.as_str().to_owned(),
        )
        .await
        .map_err(generation_error)
    }
}

#[async_trait]
impl AssistantModelCredentialRepositoryInterface for SqliteDesktopBackendSettingsAdapterImpl {
    async fn save_assistant_model_credential(
        &self,
        id: AssistantModelCredentialId,
        secret: AssistantModelCredentialSecret,
    ) -> Result<(), AssistantModelCredentialRepositoryError> {
        save_credential(
            &self.connection,
            CredentialTable::AssistantModel,
            id.as_str().to_owned(),
            secret.as_bytes().to_vec(),
        )
        .await
        .map_err(assistant_error)
    }

    async fn load_assistant_model_credential(
        &self,
        id: &AssistantModelCredentialId,
    ) -> Result<AssistantModelCredentialSecret, AssistantModelCredentialRepositoryError> {
        let bytes = load_credential(
            &self.connection,
            CredentialTable::AssistantModel,
            id.as_str().to_owned(),
        )
        .await
        .map_err(assistant_error)?;
        AssistantModelCredentialSecret::new(bytes)
    }

    async fn delete_assistant_model_credential(
        &self,
        id: &AssistantModelCredentialId,
    ) -> Result<(), AssistantModelCredentialRepositoryError> {
        delete_credential(&self.connection, CredentialTable::AssistantModel, id.as_str().to_owned())
            .await
            .map_err(assistant_error)
    }
}

fn load_or_initialize_config(
    connection: &mut Connection,
) -> Result<DesktopBackendConfig, DesktopBackendConfigRepositoryError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(config_sqlite_error)?;
    let row = transaction
        .query_row(
            "SELECT schema_version, config_json FROM desktop_backend_config WHERE singleton_id = 1",
            [],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Vec<u8>>(1)?)),
        )
        .optional()
        .map_err(config_sqlite_error)?;
    let config = match row {
        Some((CONFIG_SCHEMA_VERSION, encoded)) => {
            DesktopBackendConfig::from_canonical_json(&encoded)
                .map_err(|_| DesktopBackendConfigRepositoryError::InvalidConfig)?
        }
        Some(_) => return Err(DesktopBackendConfigRepositoryError::InvalidConfig),
        None => {
            let config = DesktopBackendConfig::default();
            let encoded = config
                .canonical_json()
                .map_err(|_| DesktopBackendConfigRepositoryError::InvalidConfig)?;
            transaction
                .execute(
                    "INSERT INTO desktop_backend_config(singleton_id, schema_version, config_json)
                     VALUES (1, ?1, ?2)",
                    params![CONFIG_SCHEMA_VERSION, encoded],
                )
                .map_err(config_sqlite_error)?;
            config
        }
    };
    transaction.commit().map_err(config_sqlite_error)?;
    Ok(config)
}

#[derive(Clone, Copy)]
enum CredentialTable {
    GenerationProvider,
    AssistantModel,
}

impl CredentialTable {
    const fn table(self) -> &'static str {
        match self {
            Self::GenerationProvider => "generation_provider_credentials",
            Self::AssistantModel => "assistant_model_credentials",
        }
    }
}

async fn save_credential(
    connection: &Arc<Mutex<Connection>>,
    table: CredentialTable,
    id: String,
    mut secret: Vec<u8>,
) -> Result<(), CredentialStorageError> {
    let connection = Arc::clone(connection);
    tokio::task::spawn_blocking(move || {
        let result = connection
            .lock()
            .map_err(|_| CredentialStorageError::Unavailable)?
            .execute(
                &format!(
                    "INSERT INTO {}(credential_id, secret) VALUES (?1, ?2)
                     ON CONFLICT(credential_id) DO UPDATE SET secret = excluded.secret",
                    table.table()
                ),
                params![id, secret],
            )
            .map(|_| ())
            .map_err(credential_sqlite_error);
        secret.fill(0);
        result
    })
    .await
    .map_err(|_| CredentialStorageError::Unavailable)?
}

async fn load_credential(
    connection: &Arc<Mutex<Connection>>,
    table: CredentialTable,
    id: String,
) -> Result<Vec<u8>, CredentialStorageError> {
    let connection = Arc::clone(connection);
    tokio::task::spawn_blocking(move || {
        connection
            .lock()
            .map_err(|_| CredentialStorageError::Unavailable)?
            .query_row(
                &format!("SELECT secret FROM {} WHERE credential_id = ?1", table.table()),
                params![id],
                |row| row.get(0),
            )
            .optional()
            .map_err(credential_sqlite_error)?
            .ok_or(CredentialStorageError::NotFound)
    })
    .await
    .map_err(|_| CredentialStorageError::Unavailable)?
}

async fn delete_credential(
    connection: &Arc<Mutex<Connection>>,
    table: CredentialTable,
    id: String,
) -> Result<(), CredentialStorageError> {
    let connection = Arc::clone(connection);
    tokio::task::spawn_blocking(move || {
        connection
            .lock()
            .map_err(|_| CredentialStorageError::Unavailable)?
            .execute(
                &format!("DELETE FROM {} WHERE credential_id = ?1", table.table()),
                params![id],
            )
            .map(|_| ())
            .map_err(credential_sqlite_error)
    })
    .await
    .map_err(|_| CredentialStorageError::Unavailable)?
}

#[derive(Clone, Copy)]
enum CredentialStorageError {
    NotFound,
    PermissionDenied,
    Unavailable,
}

fn generation_error(error: CredentialStorageError) -> GenerationProviderCredentialRepositoryError {
    match error {
        CredentialStorageError::NotFound => GenerationProviderCredentialRepositoryError::NotFound,
        CredentialStorageError::PermissionDenied => {
            GenerationProviderCredentialRepositoryError::PermissionDenied
        }
        CredentialStorageError::Unavailable => {
            GenerationProviderCredentialRepositoryError::Unavailable
        }
    }
}

fn assistant_error(error: CredentialStorageError) -> AssistantModelCredentialRepositoryError {
    match error {
        CredentialStorageError::NotFound => AssistantModelCredentialRepositoryError::NotFound,
        CredentialStorageError::PermissionDenied => {
            AssistantModelCredentialRepositoryError::PermissionDenied
        }
        CredentialStorageError::Unavailable => AssistantModelCredentialRepositoryError::Unavailable,
    }
}

fn config_sqlite_error(error: rusqlite::Error) -> DesktopBackendConfigRepositoryError {
    if permission_denied(&error) {
        DesktopBackendConfigRepositoryError::PermissionDenied
    } else {
        DesktopBackendConfigRepositoryError::Unavailable
    }
}

fn credential_sqlite_error(error: rusqlite::Error) -> CredentialStorageError {
    if permission_denied(&error) {
        CredentialStorageError::PermissionDenied
    } else {
        CredentialStorageError::Unavailable
    }
}

fn permission_denied(error: &rusqlite::Error) -> bool {
    matches!(
        error,
        rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error {
                code: ErrorCode::PermissionDenied
                    | ErrorCode::ReadOnly
                    | ErrorCode::AuthorizationForStatementDenied,
                ..
            },
            _
        )
    )
}

#[cfg(test)]
mod tests;
