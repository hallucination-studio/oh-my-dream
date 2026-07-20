use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use super::*;
use crate::assistant_provider_settings::AssistantProviderSettingsSnapshot;

struct FakeProbeImpl {
    list_result: Mutex<Option<Result<Vec<AssistantProviderModelId>, AssistantProviderProbeError>>>,
    test_result: Mutex<Option<Result<(), AssistantProviderProbeError>>>,
    tested_keys: Mutex<Vec<Vec<u8>>>,
}

#[async_trait]
impl AssistantProviderProbeInterface for FakeProbeImpl {
    async fn list_assistant_provider_models(
        &self,
        _base_url: &AssistantProviderBaseUrl,
        api_key: &AssistantProviderApiKey,
    ) -> Result<Vec<AssistantProviderModelId>, AssistantProviderProbeError> {
        self.tested_keys.lock().unwrap().push(api_key.as_bytes().to_vec());
        self.list_result.lock().unwrap().take().unwrap()
    }

    async fn test_assistant_provider_model(
        &self,
        _base_url: &AssistantProviderBaseUrl,
        api_key: &AssistantProviderApiKey,
        _model_id: &AssistantProviderModelId,
    ) -> Result<(), AssistantProviderProbeError> {
        self.tested_keys.lock().unwrap().push(api_key.as_bytes().to_vec());
        self.test_result.lock().unwrap().take().unwrap()
    }
}

struct FakeRepositoryImpl {
    snapshot: Mutex<AssistantProviderSettingsSnapshot>,
    stored_key: Mutex<Option<Vec<u8>>>,
    mutation_result: Mutex<
        Option<
            Result<
                AssistantProviderSettingsMutationResult,
                AssistantProviderSettingsRepositoryError,
            >,
        >,
    >,
    mutations: Mutex<Vec<AssistantProviderSettingsMutation>>,
}

#[async_trait]
impl AssistantProviderSettingsRepositoryInterface for FakeRepositoryImpl {
    async fn load_assistant_provider_settings_snapshot(
        &self,
    ) -> Result<AssistantProviderSettingsSnapshot, AssistantProviderSettingsRepositoryError> {
        Ok(self.snapshot.lock().unwrap().clone())
    }

    async fn load_assistant_provider_api_key(
        &self,
    ) -> Result<AssistantProviderApiKey, AssistantProviderSettingsRepositoryError> {
        self.stored_key
            .lock()
            .unwrap()
            .clone()
            .ok_or(AssistantProviderSettingsRepositoryError::MissingCredential)
            .and_then(|value| {
                AssistantProviderApiKey::try_new(value)
                    .map_err(|_| AssistantProviderSettingsRepositoryError::InvalidSnapshot)
            })
    }

    async fn apply_assistant_provider_settings_mutation(
        &self,
        _expected_revision: AssistantProviderSettingsRevision,
        mutation: AssistantProviderSettingsMutation,
    ) -> Result<AssistantProviderSettingsMutationResult, AssistantProviderSettingsRepositoryError>
    {
        self.mutations.lock().unwrap().push(mutation);
        self.mutation_result.lock().unwrap().take().unwrap()
    }
}

#[tokio::test]
async fn gets_only_the_sanitized_settings_view() {
    let repository = Arc::new(fake_repository(Some(b"stored-key".to_vec())));
    let use_case = AssistantProviderSettingsGetUseCase::new(repository);

    let view = use_case.get_assistant_provider_settings().await.unwrap();

    assert!(view.enabled);
    assert!(view.has_api_key);
    assert_eq!(view.settings_revision.get(), 2);
    assert_eq!(view.base_url.as_str(), "https://api.openai.com/v1");
    assert_eq!(view.model_id.unwrap().as_str(), "alpha");
}

#[tokio::test]
async fn lists_models_with_the_stored_key_and_returns_stable_ids() {
    let probe = Arc::new(fake_probe(Ok(vec![model("zeta"), model("alpha"), model("alpha")])));
    let repository = Arc::new(fake_repository(Some(b"stored-key".to_vec())));
    let use_case = AssistantProviderModelsListUseCase::new(probe.clone(), repository);

    let models = use_case.list_assistant_provider_models(base_url(), None).await.unwrap();

    assert_eq!(
        models.iter().map(AssistantProviderModelId::as_str).collect::<Vec<_>>(),
        vec!["alpha", "zeta"]
    );
    assert_eq!(*probe.tested_keys.lock().unwrap(), vec![b"stored-key".to_vec()]);
}

#[tokio::test]
async fn model_listing_requires_a_supplied_or_stored_key() {
    let probe = Arc::new(fake_probe(Ok(vec![model("alpha")])));
    let repository = Arc::new(fake_repository(None));
    let use_case = AssistantProviderModelsListUseCase::new(probe.clone(), repository);

    let result = use_case.list_assistant_provider_models(base_url(), None).await;

    assert_eq!(result, Err(AssistantProviderSettingsError::MissingCredential));
    assert!(probe.tested_keys.lock().unwrap().is_empty());
}

#[tokio::test]
async fn failed_compatibility_test_performs_no_settings_mutation() {
    let probe = Arc::new(FakeProbeImpl {
        list_result: Mutex::new(None),
        test_result: Mutex::new(Some(Err(
            AssistantProviderProbeError::MissingFunctionToolBehavior,
        ))),
        tested_keys: Mutex::new(Vec::new()),
    });
    let repository = Arc::new(fake_repository(Some(b"stored-key".to_vec())));
    let use_case = AssistantProviderSettingsTestAndApplyUseCase::new(probe, repository.clone());

    let result = use_case
        .test_and_apply_assistant_provider_settings(revision(1), base_url(), None, model("alpha"))
        .await;

    assert_eq!(result, Err(AssistantProviderSettingsError::MissingFunctionToolBehavior));
    assert!(repository.mutations.lock().unwrap().is_empty());
}

#[tokio::test]
async fn successful_test_applies_the_candidate_connection_once() {
    let probe = Arc::new(FakeProbeImpl {
        list_result: Mutex::new(None),
        test_result: Mutex::new(Some(Ok(()))),
        tested_keys: Mutex::new(Vec::new()),
    });
    let repository = Arc::new(fake_repository(Some(b"stored-key".to_vec())));
    *repository.mutation_result.lock().unwrap() =
        Some(Ok(AssistantProviderSettingsMutationResult::Committed(snapshot(true, 2))));
    let use_case = AssistantProviderSettingsTestAndApplyUseCase::new(probe, repository.clone());

    let view = use_case
        .test_and_apply_assistant_provider_settings(
            revision(1),
            base_url(),
            Some(AssistantProviderApiKey::try_new(b"new-key".to_vec()).unwrap()),
            model("alpha"),
        )
        .await
        .unwrap();

    assert!(view.enabled);
    assert_eq!(view.model_id.unwrap().as_str(), "alpha");
    assert_eq!(repository.mutations.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn successful_test_reports_revision_conflict_without_a_view() {
    let probe = Arc::new(FakeProbeImpl {
        list_result: Mutex::new(None),
        test_result: Mutex::new(Some(Ok(()))),
        tested_keys: Mutex::new(Vec::new()),
    });
    let repository = Arc::new(fake_repository(Some(b"stored-key".to_vec())));
    *repository.mutation_result.lock().unwrap() =
        Some(Ok(AssistantProviderSettingsMutationResult::RevisionConflict));
    let use_case = AssistantProviderSettingsTestAndApplyUseCase::new(probe, repository);

    let result = use_case
        .test_and_apply_assistant_provider_settings(revision(1), base_url(), None, model("alpha"))
        .await;

    assert_eq!(result, Err(AssistantProviderSettingsError::RevisionConflict));
}

#[tokio::test]
async fn disable_does_not_contact_the_provider() {
    let probe = Arc::new(fake_probe(Ok(vec![model("alpha")])));
    let repository = Arc::new(fake_repository(Some(b"stored-key".to_vec())));
    let use_case = AssistantProviderSettingsDisableUseCase::new(repository.clone());

    let view = use_case.disable_assistant_provider_settings(revision(1)).await.unwrap();

    assert!(!view.enabled);
    assert!(probe.tested_keys.lock().unwrap().is_empty());
    assert!(matches!(
        repository.mutations.lock().unwrap().as_slice(),
        [AssistantProviderSettingsMutation::Disable]
    ));
}

fn fake_probe(
    list_result: Result<Vec<AssistantProviderModelId>, AssistantProviderProbeError>,
) -> FakeProbeImpl {
    FakeProbeImpl {
        list_result: Mutex::new(Some(list_result)),
        test_result: Mutex::new(None),
        tested_keys: Mutex::new(Vec::new()),
    }
}

fn fake_repository(stored_key: Option<Vec<u8>>) -> FakeRepositoryImpl {
    FakeRepositoryImpl {
        snapshot: Mutex::new(snapshot(true, 2)),
        stored_key: Mutex::new(stored_key),
        mutation_result: Mutex::new(Some(Ok(AssistantProviderSettingsMutationResult::Committed(
            snapshot(false, 2),
        )))),
        mutations: Mutex::new(Vec::new()),
    }
}

fn snapshot(enabled: bool, revision_value: u64) -> AssistantProviderSettingsSnapshot {
    AssistantProviderSettingsSnapshot::try_new(
        revision(revision_value),
        enabled,
        base_url(),
        Some(model("alpha")),
        true,
    )
    .unwrap()
}

fn base_url() -> AssistantProviderBaseUrl {
    AssistantProviderBaseUrl::try_new("https://api.openai.com/v1").unwrap()
}

fn model(value: &str) -> AssistantProviderModelId {
    AssistantProviderModelId::try_new(value).unwrap()
}

fn revision(value: u64) -> AssistantProviderSettingsRevision {
    AssistantProviderSettingsRevision::new(value).unwrap()
}
