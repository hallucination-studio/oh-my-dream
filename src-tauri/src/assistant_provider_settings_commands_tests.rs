use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use rusqlite::Connection;

use super::*;
use crate::assistant_provider_settings::{
    AssistantProviderProbeError, AssistantProviderSettingsGetUseCase,
};
use crate::backend_settings_adapter::SqliteDesktopBackendSettingsAdapterImpl;

#[derive(Clone)]
struct ProbeFakeImpl {
    test_error: Option<AssistantProviderProbeError>,
    calls: Arc<Mutex<Vec<String>>>,
}

#[async_trait]
impl AssistantProviderProbeInterface for ProbeFakeImpl {
    async fn list_assistant_provider_models(
        &self,
        base_url: &AssistantProviderBaseUrl,
        api_key: &AssistantProviderApiKey,
    ) -> Result<Vec<AssistantProviderModelId>, AssistantProviderProbeError> {
        assert_eq!(base_url.as_str(), "http://localhost:11434/v1");
        assert_eq!(api_key.as_bytes(), b"test-key");
        self.calls.lock().unwrap().push("list".to_owned());
        Ok(vec![
            AssistantProviderModelId::try_new("model-b").unwrap(),
            AssistantProviderModelId::try_new("model-a").unwrap(),
        ])
    }

    async fn test_assistant_provider_model(
        &self,
        base_url: &AssistantProviderBaseUrl,
        api_key: &AssistantProviderApiKey,
        model_id: &AssistantProviderModelId,
    ) -> Result<(), AssistantProviderProbeError> {
        assert_eq!(base_url.as_str(), "http://localhost:11434/v1");
        assert_eq!(api_key.as_bytes(), b"test-key");
        assert_eq!(model_id.as_str(), "model-a");
        self.calls.lock().unwrap().push("test".to_owned());
        self.test_error.map_or(Ok(()), Err)
    }
}

#[tokio::test]
async fn commands_list_test_apply_get_and_disable_without_exposing_the_key() {
    let repository = repository();
    let calls = Arc::new(Mutex::new(Vec::new()));
    let probe = Arc::new(ProbeFakeImpl { test_error: None, calls: calls.clone() });
    let initial = get_with_use_case(&AssistantProviderSettingsGetUseCase::new(repository.clone()))
        .await
        .unwrap();
    assert_eq!(initial.settings_revision, "1");

    let listed = list_with_use_case(
        list_request(),
        &AssistantProviderModelsListUseCase::new(probe.clone(), repository.clone()),
    )
    .await
    .unwrap();
    assert_eq!(listed.models, ["model-a", "model-b"]);

    let apply = AssistantProviderSettingsTestAndApplyUseCase::new(probe, repository.clone());
    let applied = test_and_apply_with_use_case(apply_request("1"), &apply).await.unwrap();
    assert_eq!(applied.settings_revision, "2");
    assert_eq!(applied.model_id.as_deref(), Some("model-a"));
    assert!(applied.has_api_key);
    assert_eq!(*calls.lock().unwrap(), ["list", "test"]);

    let conflict = test_and_apply_with_use_case(apply_request("1"), &apply).await.unwrap_err();
    assert_eq!(conflict.code, "assistant_provider_settings.revision_conflict");

    let loaded = get_with_use_case(&AssistantProviderSettingsGetUseCase::new(repository.clone()))
        .await
        .unwrap();
    assert_eq!(loaded, applied);

    let disabled = disable_with_use_case(
        AssistantProviderSettingsDisableRequestDto { expected_settings_revision: "2".to_owned() },
        &AssistantProviderSettingsDisableUseCase::new(repository),
    )
    .await
    .unwrap();
    assert!(!disabled.enabled);
    assert!(disabled.has_api_key);
}

#[tokio::test]
async fn failed_test_does_not_overwrite_settings() {
    let repository = repository();
    let probe = Arc::new(ProbeFakeImpl {
        test_error: Some(AssistantProviderProbeError::AuthenticationRejected),
        calls: Arc::new(Mutex::new(Vec::new())),
    });
    let use_case = AssistantProviderSettingsTestAndApplyUseCase::new(probe, repository.clone());
    get_with_use_case(&AssistantProviderSettingsGetUseCase::new(repository.clone())).await.unwrap();

    let rejected = test_and_apply_with_use_case(apply_request("1"), &use_case).await.unwrap_err();
    assert_eq!(rejected.code, "assistant_provider_settings.authentication_rejected");

    let view =
        get_with_use_case(&AssistantProviderSettingsGetUseCase::new(repository)).await.unwrap();
    assert_eq!(view.settings_revision, "1");
    assert!(!view.enabled);
    assert!(!view.has_api_key);
}

fn repository() -> Arc<SqliteDesktopBackendSettingsAdapterImpl> {
    Arc::new(
        SqliteDesktopBackendSettingsAdapterImpl::try_new(Arc::new(Mutex::new(
            Connection::open_in_memory().unwrap(),
        )))
        .unwrap(),
    )
}

fn list_request() -> AssistantProviderModelsListRequestDto {
    AssistantProviderModelsListRequestDto {
        base_url: "http://localhost:11434/v1".to_owned(),
        api_key: Some("test-key".to_owned()),
    }
}

fn apply_request(revision: &str) -> AssistantProviderSettingsTestAndApplyRequestDto {
    AssistantProviderSettingsTestAndApplyRequestDto {
        expected_settings_revision: revision.to_owned(),
        base_url: "http://localhost:11434/v1".to_owned(),
        api_key: Some("test-key".to_owned()),
        model_id: "model-a".to_owned(),
    }
}
