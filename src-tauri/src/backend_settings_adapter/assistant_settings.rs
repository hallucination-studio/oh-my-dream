use std::sync::Arc;

use assistant::interfaces::AssistantApplicationError;
use async_trait::async_trait;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

use super::{CONFIG_SCHEMA_VERSION, SqliteDesktopBackendSettingsAdapterImpl, current_config};
use crate::{
    assistant_model_runner::{
        AssistantModelRuntimeConnection, AssistantModelRuntimeConnectionReaderInterface,
    },
    assistant_provider_settings::{
        AssistantProviderApiKey, AssistantProviderBaseUrl, AssistantProviderModelId,
        AssistantProviderSettingsMutation, AssistantProviderSettingsMutationResult,
        AssistantProviderSettingsRepositoryError, AssistantProviderSettingsRepositoryInterface,
        AssistantProviderSettingsRevision, AssistantProviderSettingsSnapshot,
    },
    desktop_backend_config::DesktopBackendConfig,
};

const ASSISTANT_CREDENTIAL_ID: &str = "assistant.openai.default";

#[async_trait]
impl AssistantProviderSettingsRepositoryInterface for SqliteDesktopBackendSettingsAdapterImpl {
    async fn load_assistant_provider_settings_snapshot(
        &self,
    ) -> Result<AssistantProviderSettingsSnapshot, AssistantProviderSettingsRepositoryError> {
        let connection = Arc::clone(&self.connection);
        tokio::task::spawn_blocking(move || {
            let mut connection = connection
                .lock()
                .map_err(|_| AssistantProviderSettingsRepositoryError::Unavailable)?;
            load_or_initialize_snapshot(&mut connection)
        })
        .await
        .map_err(|_| AssistantProviderSettingsRepositoryError::Unavailable)?
    }

    async fn load_assistant_provider_api_key(
        &self,
    ) -> Result<AssistantProviderApiKey, AssistantProviderSettingsRepositoryError> {
        let connection = Arc::clone(&self.connection);
        tokio::task::spawn_blocking(move || {
            let value = connection
                .lock()
                .map_err(|_| AssistantProviderSettingsRepositoryError::Unavailable)?
                .query_row(
                    "SELECT secret FROM assistant_model_credentials WHERE credential_id = ?1",
                    [ASSISTANT_CREDENTIAL_ID],
                    |row| row.get::<_, Vec<u8>>(0),
                )
                .optional()
                .map_err(|_| AssistantProviderSettingsRepositoryError::Unavailable)?
                .ok_or(AssistantProviderSettingsRepositoryError::MissingCredential)?;
            AssistantProviderApiKey::try_new(value)
                .map_err(|_| AssistantProviderSettingsRepositoryError::InvalidSnapshot)
        })
        .await
        .map_err(|_| AssistantProviderSettingsRepositoryError::Unavailable)?
    }

    async fn apply_assistant_provider_settings_mutation(
        &self,
        expected_revision: AssistantProviderSettingsRevision,
        mutation: AssistantProviderSettingsMutation,
    ) -> Result<AssistantProviderSettingsMutationResult, AssistantProviderSettingsRepositoryError>
    {
        let connection = Arc::clone(&self.connection);
        tokio::task::spawn_blocking(move || {
            let mut connection = connection
                .lock()
                .map_err(|_| AssistantProviderSettingsRepositoryError::Unavailable)?;
            apply_mutation(&mut connection, expected_revision, mutation)
        })
        .await
        .map_err(|_| AssistantProviderSettingsRepositoryError::Unavailable)?
    }
}

#[async_trait]
impl AssistantModelRuntimeConnectionReaderInterface for SqliteDesktopBackendSettingsAdapterImpl {
    async fn load_assistant_model_runtime_connection(
        &self,
    ) -> Result<AssistantModelRuntimeConnection, AssistantApplicationError> {
        let connection = Arc::clone(&self.connection);
        tokio::task::spawn_blocking(move || {
            let mut connection =
                connection.lock().map_err(|_| AssistantApplicationError::ModelUnavailable)?;
            load_runtime_connection(&mut connection)
        })
        .await
        .map_err(|_| AssistantApplicationError::ModelUnavailable)?
    }
}

fn load_runtime_connection(
    connection: &mut Connection,
) -> Result<AssistantModelRuntimeConnection, AssistantApplicationError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .map_err(|_| AssistantApplicationError::ModelUnavailable)?;
    let Some((CONFIG_SCHEMA_VERSION, revision, encoded)) =
        super::provider_settings::current_config_row(&transaction)
            .map_err(|_| AssistantApplicationError::ModelUnavailable)?
    else {
        return Err(AssistantApplicationError::ModelUnavailable);
    };
    let snapshot = project_snapshot(&transaction, revision, &encoded)
        .map_err(|_| AssistantApplicationError::ModelUnavailable)?;
    if !snapshot.enabled() {
        return Err(AssistantApplicationError::ModelUnavailable);
    }
    let model_id =
        snapshot.model_id().cloned().ok_or(AssistantApplicationError::ModelUnavailable)?;
    let api_key = transaction
        .query_row(
            "SELECT secret FROM assistant_model_credentials WHERE credential_id = ?1",
            [ASSISTANT_CREDENTIAL_ID],
            |row| row.get::<_, Vec<u8>>(0),
        )
        .optional()
        .map_err(|_| AssistantApplicationError::ModelUnavailable)?
        .ok_or(AssistantApplicationError::ModelUnavailable)
        .and_then(|value| {
            AssistantProviderApiKey::try_new(value)
                .map_err(|_| AssistantApplicationError::ModelUnavailable)
        })?;
    let runtime =
        AssistantModelRuntimeConnection::new(snapshot.base_url().clone(), model_id, api_key);
    transaction.commit().map_err(|_| AssistantApplicationError::ModelUnavailable)?;
    Ok(runtime)
}

fn load_or_initialize_snapshot(
    connection: &mut Connection,
) -> Result<AssistantProviderSettingsSnapshot, AssistantProviderSettingsRepositoryError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| AssistantProviderSettingsRepositoryError::Unavailable)?;
    let row = super::provider_settings::current_config_row(&transaction)
        .map_err(|_| AssistantProviderSettingsRepositoryError::Unavailable)?;
    let (revision, encoded) = match row {
        Some((CONFIG_SCHEMA_VERSION, revision, encoded)) => (revision, encoded),
        Some(_) => return Err(AssistantProviderSettingsRepositoryError::InvalidSnapshot),
        None => initialize_config(&transaction)?,
    };
    let snapshot = project_snapshot(&transaction, revision, &encoded)?;
    transaction.commit().map_err(|_| AssistantProviderSettingsRepositoryError::Unavailable)?;
    Ok(snapshot)
}

fn initialize_config(
    connection: &Connection,
) -> Result<(i64, Vec<u8>), AssistantProviderSettingsRepositoryError> {
    let encoded = current_config::encode(&DesktopBackendConfig::default())
        .map_err(|_| AssistantProviderSettingsRepositoryError::InvalidSnapshot)?;
    connection
        .execute(
            "INSERT INTO desktop_backend_config(
                 singleton_id, schema_version, revision, config_json
             ) VALUES (1, ?1, 1, ?2)",
            params![CONFIG_SCHEMA_VERSION, encoded],
        )
        .map_err(|_| AssistantProviderSettingsRepositoryError::Unavailable)?;
    Ok((1, encoded))
}

fn apply_mutation(
    connection: &mut Connection,
    expected_revision: AssistantProviderSettingsRevision,
    mutation: AssistantProviderSettingsMutation,
) -> Result<AssistantProviderSettingsMutationResult, AssistantProviderSettingsRepositoryError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| AssistantProviderSettingsRepositoryError::Unavailable)?;
    let Some((CONFIG_SCHEMA_VERSION, revision, encoded)) =
        super::provider_settings::current_config_row(&transaction)
            .map_err(|_| AssistantProviderSettingsRepositoryError::Unavailable)?
    else {
        return Err(AssistantProviderSettingsRepositoryError::InvalidSnapshot);
    };
    let current = project_snapshot(&transaction, revision, &encoded)?;
    if current.revision() != expected_revision {
        return Ok(AssistantProviderSettingsMutationResult::RevisionConflict);
    }
    let mut config = current_config::project(&encoded)
        .map_err(|_| AssistantProviderSettingsRepositoryError::InvalidSnapshot)?;
    let replacement_key = mutate_config(&mut config, current.has_api_key(), mutation)?;
    let has_api_key = current.has_api_key() || replacement_key.is_some();
    let next_revision = next_revision(revision)?;
    persist_config(&transaction, revision, next_revision, &config)?;
    if let Some(api_key) = replacement_key {
        persist_api_key(&transaction, api_key)?;
    }
    let snapshot = project_snapshot_from_config(next_revision, config, has_api_key)?;
    transaction.commit().map_err(|_| AssistantProviderSettingsRepositoryError::Unavailable)?;
    Ok(AssistantProviderSettingsMutationResult::Committed(snapshot))
}

fn mutate_config(
    config: &mut DesktopBackendConfig,
    has_api_key: bool,
    mutation: AssistantProviderSettingsMutation,
) -> Result<Option<AssistantProviderApiKey>, AssistantProviderSettingsRepositoryError> {
    match mutation {
        AssistantProviderSettingsMutation::ApplyTestedConnection {
            base_url,
            model_id,
            api_key,
        } => {
            if api_key.is_none() && !has_api_key {
                return Err(AssistantProviderSettingsRepositoryError::MissingCredential);
            }
            config.assistant_model.enabled = true;
            config.assistant_model.base_url = base_url.as_str().to_owned();
            config.assistant_model.model_id = Some(model_id.as_str().to_owned());
            Ok(api_key)
        }
        AssistantProviderSettingsMutation::Disable => {
            config.assistant_model.enabled = false;
            Ok(None)
        }
    }
}

fn persist_config(
    connection: &Connection,
    revision: i64,
    next_revision: AssistantProviderSettingsRevision,
    config: &DesktopBackendConfig,
) -> Result<(), AssistantProviderSettingsRepositoryError> {
    let encoded = current_config::encode(config)
        .map_err(|_| AssistantProviderSettingsRepositoryError::InvalidSnapshot)?;
    let updated = connection
        .execute(
            "UPDATE desktop_backend_config SET revision = ?1, config_json = ?2
             WHERE singleton_id = 1 AND revision = ?3",
            params![next_revision.get(), encoded, revision],
        )
        .map_err(|_| AssistantProviderSettingsRepositoryError::Unavailable)?;
    if updated == 1 { Ok(()) } else { Err(AssistantProviderSettingsRepositoryError::Unavailable) }
}

fn persist_api_key(
    connection: &Connection,
    api_key: AssistantProviderApiKey,
) -> Result<(), AssistantProviderSettingsRepositoryError> {
    let mut value = api_key.as_bytes().to_vec();
    let result = connection
        .execute(
            "INSERT INTO assistant_model_credentials(credential_id, secret) VALUES (?1, ?2)
             ON CONFLICT(credential_id) DO UPDATE SET secret = excluded.secret",
            params![ASSISTANT_CREDENTIAL_ID, value],
        )
        .map(|_| ())
        .map_err(|_| AssistantProviderSettingsRepositoryError::Unavailable);
    value.fill(0);
    result
}

fn project_snapshot(
    connection: &Connection,
    revision: i64,
    encoded: &[u8],
) -> Result<AssistantProviderSettingsSnapshot, AssistantProviderSettingsRepositoryError> {
    let config = current_config::project(encoded)
        .map_err(|_| AssistantProviderSettingsRepositoryError::InvalidSnapshot)?;
    let has_api_key = connection
        .query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM assistant_model_credentials WHERE credential_id = ?1
             )",
            [ASSISTANT_CREDENTIAL_ID],
            |row| row.get(0),
        )
        .map_err(|_| AssistantProviderSettingsRepositoryError::Unavailable)?;
    project_snapshot_from_config(revision_value(revision)?, config, has_api_key)
}

fn project_snapshot_from_config(
    revision: AssistantProviderSettingsRevision,
    config: DesktopBackendConfig,
    has_api_key: bool,
) -> Result<AssistantProviderSettingsSnapshot, AssistantProviderSettingsRepositoryError> {
    let base_url = AssistantProviderBaseUrl::try_new(config.assistant_model.base_url)
        .map_err(|_| AssistantProviderSettingsRepositoryError::InvalidSnapshot)?;
    let model_id = config
        .assistant_model
        .model_id
        .map(AssistantProviderModelId::try_new)
        .transpose()
        .map_err(|_| AssistantProviderSettingsRepositoryError::InvalidSnapshot)?;
    AssistantProviderSettingsSnapshot::try_new(
        revision,
        config.assistant_model.enabled,
        base_url,
        model_id,
        has_api_key,
    )
    .map_err(|_| AssistantProviderSettingsRepositoryError::InvalidSnapshot)
}

fn next_revision(
    revision: i64,
) -> Result<AssistantProviderSettingsRevision, AssistantProviderSettingsRepositoryError> {
    revision
        .checked_add(1)
        .ok_or(AssistantProviderSettingsRepositoryError::Unavailable)
        .and_then(revision_value)
}

fn revision_value(
    value: i64,
) -> Result<AssistantProviderSettingsRevision, AssistantProviderSettingsRepositoryError> {
    u64::try_from(value)
        .ok()
        .and_then(AssistantProviderSettingsRevision::new)
        .ok_or(AssistantProviderSettingsRepositoryError::InvalidSnapshot)
}

#[cfg(test)]
#[path = "assistant_settings_tests.rs"]
mod tests;
