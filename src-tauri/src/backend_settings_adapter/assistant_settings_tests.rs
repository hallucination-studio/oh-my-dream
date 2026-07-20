use std::sync::{Arc, Mutex};

use assistant::interfaces::AssistantApplicationError;
use rusqlite::Connection;

use super::super::SqliteDesktopBackendSettingsAdapterImpl;
use crate::assistant_model_runner::AssistantModelRuntimeConnectionReaderInterface;
use crate::assistant_provider_settings::{
    AssistantProviderApiKey, AssistantProviderBaseUrl, AssistantProviderModelId,
    AssistantProviderSettingsMutation, AssistantProviderSettingsMutationResult,
    AssistantProviderSettingsRepositoryError, AssistantProviderSettingsRepositoryInterface,
    AssistantProviderSettingsRevision,
};

#[tokio::test]
async fn initializes_and_loads_sanitized_assistant_settings() {
    let (_connection, adapter) = adapter();

    let snapshot = adapter.load_assistant_provider_settings_snapshot().await.unwrap();

    assert_eq!(snapshot.revision().get(), 1);
    assert!(!snapshot.enabled());
    assert_eq!(snapshot.base_url().as_str(), "https://api.openai.com/v1");
    assert_eq!(snapshot.model_id(), None);
    assert!(!snapshot.has_api_key());
}

#[tokio::test]
async fn runtime_reader_fails_closed_until_enabled_and_returns_one_consistent_connection() {
    let (_connection, adapter) = adapter();
    assert_eq!(
        adapter.load_assistant_model_runtime_connection().await.err(),
        Some(AssistantApplicationError::ModelUnavailable)
    );
    let initial = adapter.load_assistant_provider_settings_snapshot().await.unwrap();
    adapter
        .apply_assistant_provider_settings_mutation(
            initial.revision(),
            tested_connection("http://127.0.0.1:11434/v1", "model-a", Some(b"runtime-key")),
        )
        .await
        .unwrap();

    let runtime = adapter.load_assistant_model_runtime_connection().await.unwrap();

    assert_eq!(runtime.base_url().as_str(), "http://127.0.0.1:11434/v1");
    assert_eq!(runtime.model_id().as_str(), "model-a");
    assert_eq!(runtime.api_key().as_bytes(), b"runtime-key");
}

#[tokio::test]
async fn atomically_applies_tested_connection_and_can_retain_its_key() {
    let (connection, adapter) = adapter();
    let initial = adapter.load_assistant_provider_settings_snapshot().await.unwrap();

    let AssistantProviderSettingsMutationResult::Committed(applied) = adapter
        .apply_assistant_provider_settings_mutation(
            initial.revision(),
            tested_connection("http://127.0.0.1:11434/v1", "model-a", Some(b"new-key")),
        )
        .await
        .unwrap()
    else {
        panic!("expected committed settings")
    };
    assert!(applied.enabled());
    assert_eq!(applied.revision().get(), 2);
    assert_eq!(adapter.load_assistant_provider_api_key().await.unwrap().as_bytes(), b"new-key");
    let config_json: Vec<u8> = connection
        .lock()
        .unwrap()
        .query_row(
            "SELECT config_json FROM desktop_backend_config WHERE singleton_id = 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(!config_json.windows(b"new-key".len()).any(|value| value == b"new-key"));

    let AssistantProviderSettingsMutationResult::Committed(changed_model) = adapter
        .apply_assistant_provider_settings_mutation(
            applied.revision(),
            tested_connection("http://127.0.0.1:11434/v1", "model-b", None),
        )
        .await
        .unwrap()
    else {
        panic!("expected committed settings")
    };
    assert_eq!(changed_model.model_id().unwrap().as_str(), "model-b");
    assert_eq!(adapter.load_assistant_provider_api_key().await.unwrap().as_bytes(), b"new-key");
}

#[tokio::test]
async fn stale_revision_changes_neither_config_nor_credential() {
    let (_connection, adapter) = adapter();
    let initial = adapter.load_assistant_provider_settings_snapshot().await.unwrap();
    let result = adapter
        .apply_assistant_provider_settings_mutation(
            AssistantProviderSettingsRevision::new(2).unwrap(),
            tested_connection("https://example.com/v1", "model-a", Some(b"rejected-key")),
        )
        .await
        .unwrap();

    assert_eq!(result, AssistantProviderSettingsMutationResult::RevisionConflict);
    assert_eq!(adapter.load_assistant_provider_settings_snapshot().await.unwrap(), initial);
    assert_eq!(
        adapter.load_assistant_provider_api_key().await,
        Err(AssistantProviderSettingsRepositoryError::MissingCredential)
    );
}

#[tokio::test]
async fn credential_write_failure_rolls_back_the_config_write() {
    let (connection, adapter) = adapter();
    let initial = adapter.load_assistant_provider_settings_snapshot().await.unwrap();
    let AssistantProviderSettingsMutationResult::Committed(applied) = adapter
        .apply_assistant_provider_settings_mutation(
            initial.revision(),
            tested_connection("https://example.com/v1", "model-old", Some(b"old-key")),
        )
        .await
        .unwrap()
    else {
        panic!("expected committed settings")
    };
    connection
        .lock()
        .unwrap()
        .execute_batch(
            "CREATE TRIGGER reject_assistant_credential
             BEFORE INSERT ON assistant_model_credentials
             BEGIN SELECT RAISE(ABORT, 'injected failure'); END;",
        )
        .unwrap();

    let result = adapter
        .apply_assistant_provider_settings_mutation(
            applied.revision(),
            tested_connection("https://example.com/v1", "model-a", Some(b"new-key")),
        )
        .await;

    assert_eq!(result, Err(AssistantProviderSettingsRepositoryError::Unavailable));
    assert_eq!(adapter.load_assistant_provider_settings_snapshot().await.unwrap(), applied);
    assert_eq!(adapter.load_assistant_provider_api_key().await.unwrap().as_bytes(), b"old-key");
}

#[tokio::test]
async fn disable_retains_the_last_tested_connection_and_key() {
    let (_connection, adapter) = adapter();
    let initial = adapter.load_assistant_provider_settings_snapshot().await.unwrap();
    let AssistantProviderSettingsMutationResult::Committed(applied) = adapter
        .apply_assistant_provider_settings_mutation(
            initial.revision(),
            tested_connection("https://example.com/v1", "model-a", Some(b"saved-key")),
        )
        .await
        .unwrap()
    else {
        panic!("expected committed settings")
    };

    let AssistantProviderSettingsMutationResult::Committed(disabled) = adapter
        .apply_assistant_provider_settings_mutation(
            applied.revision(),
            AssistantProviderSettingsMutation::Disable,
        )
        .await
        .unwrap()
    else {
        panic!("expected committed settings")
    };

    assert!(!disabled.enabled());
    assert_eq!(disabled.base_url(), applied.base_url());
    assert_eq!(disabled.model_id(), applied.model_id());
    assert!(disabled.has_api_key());
    assert_eq!(adapter.load_assistant_provider_api_key().await.unwrap().as_bytes(), b"saved-key");
}

fn adapter() -> (Arc<Mutex<Connection>>, SqliteDesktopBackendSettingsAdapterImpl) {
    let connection = Arc::new(Mutex::new(Connection::open_in_memory().unwrap()));
    let adapter =
        SqliteDesktopBackendSettingsAdapterImpl::try_new(Arc::clone(&connection)).unwrap();
    (connection, adapter)
}

fn tested_connection(
    base_url: &str,
    model_id: &str,
    api_key: Option<&[u8]>,
) -> AssistantProviderSettingsMutation {
    AssistantProviderSettingsMutation::ApplyTestedConnection {
        base_url: AssistantProviderBaseUrl::try_new(base_url).unwrap(),
        model_id: AssistantProviderModelId::try_new(model_id).unwrap(),
        api_key: api_key.map(|value| AssistantProviderApiKey::try_new(value.to_vec()).unwrap()),
    }
}
