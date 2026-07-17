use super::*;
use backends::generation_provider_settings::{
    GenerationProviderSettingsMutation, GenerationProviderSettingsMutationResult,
    GenerationProviderSettingsRepositoryInterface,
};
use tasks::generation_task::GenerationTaskRequestKind;

fn connection() -> Arc<Mutex<Connection>> {
    Arc::new(Mutex::new(Connection::open_in_memory().unwrap()))
}

#[tokio::test]
async fn config_initializes_defaults_round_trips_and_rejects_corruption() {
    let connection = connection();
    let adapter =
        SqliteDesktopBackendSettingsAdapterImpl::try_new(Arc::clone(&connection)).unwrap();

    let default = adapter.load_or_initialize_desktop_backend_config().await.unwrap();
    assert_eq!(default, DesktopBackendConfig::default());
    let (schema_version, encoded): (i64, Vec<u8>) = connection
        .lock()
        .unwrap()
        .query_row(
            "SELECT schema_version, config_json FROM desktop_backend_config WHERE singleton_id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    let stored: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(schema_version, 2);
    assert_eq!(stored["generation_provider_routes"].as_array().unwrap().len(), 3);
    let has_legacy_column: bool = connection
        .lock()
        .unwrap()
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM pragma_table_info('desktop_backend_config')
                WHERE name = 'legacy_config_json'
             )",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(!has_legacy_column);
    let mut changed = default;
    changed.workflow_run_concurrency = 2;
    adapter.save_desktop_backend_config(changed.clone()).await.unwrap();
    assert_eq!(adapter.load_or_initialize_desktop_backend_config().await.unwrap(), changed);

    connection
        .lock()
        .unwrap()
        .execute(
            "UPDATE desktop_backend_config SET config_json = X'7B7D' WHERE singleton_id = 1",
            [],
        )
        .unwrap();
    assert_eq!(
        adapter.load_or_initialize_desktop_backend_config().await,
        Err(DesktopBackendConfigRepositoryError::InvalidConfig)
    );
}

#[tokio::test]
async fn credentials_round_trip_plaintext_in_separate_namespaces_and_delete_idempotently() {
    let connection = connection();
    let adapter =
        SqliteDesktopBackendSettingsAdapterImpl::try_new(Arc::clone(&connection)).unwrap();
    let generation_id = GenerationProviderCredentialId::new("shared.default").unwrap();
    let assistant_id = AssistantModelCredentialId::new("shared.default").unwrap();
    adapter
        .save_generation_provider_credential(
            generation_id.clone(),
            GenerationProviderCredentialSecret::new(b"generation-secret".to_vec()).unwrap(),
        )
        .await
        .unwrap();
    adapter
        .save_assistant_model_credential(
            assistant_id.clone(),
            AssistantModelCredentialSecret::new(b"assistant-secret".to_vec()).unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        adapter.load_generation_provider_credential(&generation_id).await.unwrap().as_bytes(),
        b"generation-secret"
    );
    assert_eq!(
        adapter.load_assistant_model_credential(&assistant_id).await.unwrap().as_bytes(),
        b"assistant-secret"
    );
    let stored: Vec<u8> = connection
        .lock()
        .unwrap()
        .query_row(
            "SELECT secret FROM generation_provider_credentials WHERE credential_id = ?1",
            ["shared.default"],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(stored, b"generation-secret");

    adapter.delete_generation_provider_credential(&generation_id).await.unwrap();
    adapter.delete_generation_provider_credential(&generation_id).await.unwrap();
    assert_eq!(
        adapter.load_generation_provider_credential(&generation_id).await.err(),
        Some(GenerationProviderCredentialRepositoryError::NotFound)
    );
    assert_eq!(
        adapter.load_assistant_model_credential(&assistant_id).await.unwrap().as_bytes(),
        b"assistant-secret"
    );
}

#[tokio::test]
async fn readonly_database_maps_writes_to_permission_denied() {
    let connection = connection();
    let adapter =
        SqliteDesktopBackendSettingsAdapterImpl::try_new(Arc::clone(&connection)).unwrap();
    connection.lock().unwrap().pragma_update(None, "query_only", true).unwrap();

    assert_eq!(
        adapter.save_desktop_backend_config(DesktopBackendConfig::default()).await,
        Err(DesktopBackendConfigRepositoryError::PermissionDenied)
    );
    assert_eq!(
        adapter
            .save_generation_provider_credential(
                GenerationProviderCredentialId::new("legacy.inactive").unwrap(),
                GenerationProviderCredentialSecret::new(b"secret".to_vec()).unwrap(),
            )
            .await,
        Err(GenerationProviderCredentialRepositoryError::PermissionDenied)
    );
}

#[tokio::test]
async fn current_config_rejects_noncanonical_bytes() {
    let connection = connection();
    let adapter =
        SqliteDesktopBackendSettingsAdapterImpl::try_new(Arc::clone(&connection)).unwrap();
    let mut encoded = current_config::encode(&DesktopBackendConfig::default()).unwrap();
    encoded.push(b' ');
    connection
        .lock()
        .unwrap()
        .execute(
            "INSERT INTO desktop_backend_config(singleton_id, schema_version, revision, config_json)
             VALUES (1, 2, 1, ?1)",
            [encoded],
        )
        .unwrap();

    assert_eq!(
        adapter.load_or_initialize_desktop_backend_config().await,
        Err(DesktopBackendConfigRepositoryError::InvalidConfig)
    );
}

#[tokio::test]
async fn provider_settings_cas_persists_restart_and_preserves_inactive_credentials() {
    let connection = connection();
    let adapter =
        SqliteDesktopBackendSettingsAdapterImpl::try_new(Arc::clone(&connection)).unwrap();
    let credential_id = GenerationProviderCredentialId::new("legacy.inactive").unwrap();
    adapter
        .save_generation_provider_credential(
            credential_id.clone(),
            GenerationProviderCredentialSecret::new(b"preserved".to_vec()).unwrap(),
        )
        .await
        .unwrap();
    let initial = adapter.load_generation_provider_settings_snapshot().await.unwrap();
    assert_eq!(initial.revision().get(), 1);
    assert_eq!(initial.bindings().len(), 3);
    let image = initial
        .bindings()
        .iter()
        .find(|binding| binding.generation_kind() == GenerationTaskRequestKind::Image)
        .unwrap()
        .clone();
    let removal = GenerationProviderSettingsMutation::RemoveBinding {
        profile_ref: image.profile_ref().clone(),
        generation_kind: image.generation_kind(),
    };

    let GenerationProviderSettingsMutationResult::Committed(removed) = adapter
        .apply_generation_provider_settings_mutation(initial.revision(), removal.clone())
        .await
        .unwrap()
    else {
        panic!("committed removal")
    };
    assert_eq!(removed.revision().get(), 2);
    assert_eq!(removed.bindings().len(), 2);
    assert_eq!(
        adapter
            .apply_generation_provider_settings_mutation(initial.revision(), removal.clone())
            .await
            .unwrap(),
        GenerationProviderSettingsMutationResult::RevisionConflict
    );
    assert!(matches!(
        adapter
            .apply_generation_provider_settings_mutation(removed.revision(), removal)
            .await
            .unwrap(),
        GenerationProviderSettingsMutationResult::Unchanged(snapshot)
            if snapshot.revision() == removed.revision()
    ));

    let restarted =
        SqliteDesktopBackendSettingsAdapterImpl::try_new(Arc::clone(&connection)).unwrap();
    assert_eq!(restarted.load_generation_provider_settings_snapshot().await.unwrap(), removed);
    assert_eq!(
        restarted
            .load_or_initialize_desktop_backend_config()
            .await
            .unwrap()
            .generation_provider_routes
            .len(),
        2
    );
    assert_eq!(
        restarted.load_generation_provider_credential(&credential_id).await.unwrap().as_bytes(),
        b"preserved"
    );

    let GenerationProviderSettingsMutationResult::Committed(restored) = restarted
        .apply_generation_provider_settings_mutation(
            removed.revision(),
            GenerationProviderSettingsMutation::SetBinding(image),
        )
        .await
        .unwrap()
    else {
        panic!("committed restore")
    };
    assert_eq!(restored.revision().get(), 3);
    assert_eq!(restored.bindings().len(), 3);
}
