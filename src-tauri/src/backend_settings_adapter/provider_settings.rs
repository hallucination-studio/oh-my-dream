use std::sync::Arc;

use async_trait::async_trait;
use backends::generation_provider_settings::{
    GenerationProviderSettingsBinding, GenerationProviderSettingsError,
    GenerationProviderSettingsMutation, GenerationProviderSettingsMutationResult,
    GenerationProviderSettingsRepositoryInterface, GenerationProviderSettingsRevision,
    GenerationProviderSettingsSnapshot,
};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

use super::{CONFIG_SCHEMA_VERSION, SqliteDesktopBackendSettingsAdapterImpl, current_config};
use crate::desktop_backend_config::DesktopBackendConfig;

#[async_trait]
impl GenerationProviderSettingsRepositoryInterface for SqliteDesktopBackendSettingsAdapterImpl {
    async fn load_generation_provider_settings_snapshot(
        &self,
    ) -> Result<GenerationProviderSettingsSnapshot, GenerationProviderSettingsError> {
        let connection = Arc::clone(&self.connection);
        tokio::task::spawn_blocking(move || {
            let mut connection =
                connection.lock().map_err(|_| GenerationProviderSettingsError::Repository)?;
            load_or_initialize_settings(&mut connection)
        })
        .await
        .map_err(|_| GenerationProviderSettingsError::Repository)?
    }

    async fn apply_generation_provider_settings_mutation(
        &self,
        expected_revision: GenerationProviderSettingsRevision,
        mutation: GenerationProviderSettingsMutation,
    ) -> Result<GenerationProviderSettingsMutationResult, GenerationProviderSettingsError> {
        let connection = Arc::clone(&self.connection);
        tokio::task::spawn_blocking(move || {
            let mut connection =
                connection.lock().map_err(|_| GenerationProviderSettingsError::Repository)?;
            apply_settings_mutation(&mut connection, expected_revision, mutation)
        })
        .await
        .map_err(|_| GenerationProviderSettingsError::Repository)?
    }
}

fn load_or_initialize_settings(
    connection: &mut Connection,
) -> Result<GenerationProviderSettingsSnapshot, GenerationProviderSettingsError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| GenerationProviderSettingsError::Repository)?;
    let row = current_config_row(&transaction)
        .map_err(|_| GenerationProviderSettingsError::Repository)?;
    let snapshot = match row {
        Some((CONFIG_SCHEMA_VERSION, revision, encoded)) => {
            current_config::settings_snapshot(revision, &encoded)?
        }
        Some(_) => return Err(GenerationProviderSettingsError::InvalidSnapshot),
        None => initialize_settings(&transaction)?,
    };
    transaction.commit().map_err(|_| GenerationProviderSettingsError::Repository)?;
    Ok(snapshot)
}

fn initialize_settings(
    connection: &Connection,
) -> Result<GenerationProviderSettingsSnapshot, GenerationProviderSettingsError> {
    let config = DesktopBackendConfig::default();
    let encoded = current_config::encode(&config)
        .map_err(|_| GenerationProviderSettingsError::InvalidSnapshot)?;
    connection
        .execute(
            "INSERT INTO desktop_backend_config(
                 singleton_id, schema_version, revision, config_json
             ) VALUES (1, ?1, 1, ?2)",
            params![CONFIG_SCHEMA_VERSION, encoded],
        )
        .map_err(|_| GenerationProviderSettingsError::Repository)?;
    current_config::settings_snapshot(1, &encoded)
}

fn apply_settings_mutation(
    connection: &mut Connection,
    expected_revision: GenerationProviderSettingsRevision,
    mutation: GenerationProviderSettingsMutation,
) -> Result<GenerationProviderSettingsMutationResult, GenerationProviderSettingsError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| GenerationProviderSettingsError::Repository)?;
    let Some((CONFIG_SCHEMA_VERSION, revision, encoded)) = current_config_row(&transaction)
        .map_err(|_| GenerationProviderSettingsError::Repository)?
    else {
        return Err(GenerationProviderSettingsError::InvalidSnapshot);
    };
    let snapshot = current_config::settings_snapshot(revision, &encoded)?;
    if snapshot.revision() != expected_revision {
        return Ok(GenerationProviderSettingsMutationResult::RevisionConflict);
    }
    let mut bindings = snapshot.bindings().to_vec();
    let index = bindings.iter().position(|binding| mutation_matches_binding(&mutation, binding));
    if !mutate_bindings(&mut bindings, index, mutation) {
        transaction.commit().map_err(|_| GenerationProviderSettingsError::Repository)?;
        return Ok(GenerationProviderSettingsMutationResult::Unchanged(snapshot));
    }
    let next_revision = next_revision(revision)?;
    persist_bindings(&transaction, revision, next_revision, &encoded, &bindings)?;
    transaction.commit().map_err(|_| GenerationProviderSettingsError::Repository)?;
    Ok(GenerationProviderSettingsMutationResult::Committed(
        GenerationProviderSettingsSnapshot::try_new(next_revision, bindings)?,
    ))
}

fn persist_bindings(
    connection: &Connection,
    revision: i64,
    next_revision: GenerationProviderSettingsRevision,
    encoded: &[u8],
    bindings: &[GenerationProviderSettingsBinding],
) -> Result<(), GenerationProviderSettingsError> {
    let config = current_config::project(encoded)
        .map_err(|_| GenerationProviderSettingsError::InvalidSnapshot)?;
    let current_bindings =
        bindings.iter().map(current_config::CurrentProviderBinding::from_domain).collect();
    let next_encoded = current_config::encode_with_bindings(&config, current_bindings)
        .map_err(|_| GenerationProviderSettingsError::InvalidSnapshot)?;
    let updated = connection
        .execute(
            "UPDATE desktop_backend_config SET revision = ?1, config_json = ?2
             WHERE singleton_id = 1 AND revision = ?3",
            params![
                i64::try_from(next_revision.get())
                    .map_err(|_| GenerationProviderSettingsError::Repository)?,
                next_encoded,
                revision
            ],
        )
        .map_err(|_| GenerationProviderSettingsError::Repository)?;
    if updated == 1 { Ok(()) } else { Err(GenerationProviderSettingsError::Repository) }
}

pub(super) fn current_config_row(
    connection: &Connection,
) -> rusqlite::Result<Option<(i64, i64, Vec<u8>)>> {
    connection
        .query_row(
            "SELECT schema_version, revision, config_json
             FROM desktop_backend_config WHERE singleton_id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
}

fn next_revision(
    revision: i64,
) -> Result<GenerationProviderSettingsRevision, GenerationProviderSettingsError> {
    revision
        .checked_add(1)
        .and_then(|value| u64::try_from(value).ok())
        .and_then(GenerationProviderSettingsRevision::new)
        .ok_or(GenerationProviderSettingsError::Repository)
}

fn mutate_bindings(
    bindings: &mut Vec<GenerationProviderSettingsBinding>,
    index: Option<usize>,
    mutation: GenerationProviderSettingsMutation,
) -> bool {
    match mutation {
        GenerationProviderSettingsMutation::SetBinding(binding) => match index {
            Some(index) if bindings[index] == binding => false,
            Some(index) => {
                bindings[index] = binding;
                true
            }
            None => {
                bindings.push(binding);
                true
            }
        },
        GenerationProviderSettingsMutation::RemoveBinding { .. } => match index {
            Some(index) => {
                bindings.remove(index);
                true
            }
            None => false,
        },
    }
}

fn mutation_matches_binding(
    mutation: &GenerationProviderSettingsMutation,
    binding: &GenerationProviderSettingsBinding,
) -> bool {
    match mutation {
        GenerationProviderSettingsMutation::SetBinding(candidate) => {
            candidate.profile_ref() == binding.profile_ref()
                && candidate.generation_kind() == binding.generation_kind()
        }
        GenerationProviderSettingsMutation::RemoveBinding { profile_ref, generation_kind } => {
            profile_ref == binding.profile_ref() && *generation_kind == binding.generation_kind()
        }
    }
}
