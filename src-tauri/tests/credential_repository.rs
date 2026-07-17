use std::{collections::BTreeMap, sync::Mutex};

use async_trait::async_trait;
use oh_my_dream_tauri::credential_repository::*;

#[derive(Default)]
struct GenerationRepositoryFakeImpl {
    values: Mutex<BTreeMap<GenerationProviderCredentialId, Vec<u8>>>,
}

#[derive(Default)]
struct AssistantRepositoryFakeImpl {
    values: Mutex<BTreeMap<AssistantModelCredentialId, Vec<u8>>>,
    failure: Mutex<Option<AssistantModelCredentialRepositoryError>>,
}

#[async_trait]
impl AssistantModelCredentialRepositoryInterface for AssistantRepositoryFakeImpl {
    async fn save_assistant_model_credential(
        &self,
        id: AssistantModelCredentialId,
        secret: AssistantModelCredentialSecret,
    ) -> Result<(), AssistantModelCredentialRepositoryError> {
        if let Some(error) = *self.failure.lock().unwrap() {
            return Err(error);
        }
        self.values.lock().unwrap().insert(id, secret.as_bytes().to_vec());
        Ok(())
    }

    async fn load_assistant_model_credential(
        &self,
        id: &AssistantModelCredentialId,
    ) -> Result<AssistantModelCredentialSecret, AssistantModelCredentialRepositoryError> {
        if let Some(error) = *self.failure.lock().unwrap() {
            return Err(error);
        }
        self.values
            .lock()
            .unwrap()
            .get(id)
            .cloned()
            .ok_or(AssistantModelCredentialRepositoryError::NotFound)
            .and_then(AssistantModelCredentialSecret::new)
    }

    async fn delete_assistant_model_credential(
        &self,
        id: &AssistantModelCredentialId,
    ) -> Result<(), AssistantModelCredentialRepositoryError> {
        if let Some(error) = *self.failure.lock().unwrap() {
            return Err(error);
        }
        self.values.lock().unwrap().remove(id);
        Ok(())
    }
}

#[async_trait]
impl GenerationProviderCredentialRepositoryInterface for GenerationRepositoryFakeImpl {
    async fn save_generation_provider_credential(
        &self,
        id: GenerationProviderCredentialId,
        secret: GenerationProviderCredentialSecret,
    ) -> Result<(), GenerationProviderCredentialRepositoryError> {
        self.values.lock().unwrap().insert(id, secret.as_bytes().to_vec());
        Ok(())
    }

    async fn load_generation_provider_credential(
        &self,
        id: &GenerationProviderCredentialId,
    ) -> Result<GenerationProviderCredentialSecret, GenerationProviderCredentialRepositoryError>
    {
        self.values
            .lock()
            .unwrap()
            .get(id)
            .cloned()
            .ok_or(GenerationProviderCredentialRepositoryError::NotFound)
            .and_then(GenerationProviderCredentialSecret::new)
    }

    async fn delete_generation_provider_credential(
        &self,
        id: &GenerationProviderCredentialId,
    ) -> Result<(), GenerationProviderCredentialRepositoryError> {
        self.values.lock().unwrap().remove(id);
        Ok(())
    }
}

#[test]
fn credential_ids_are_typed_isolated_and_secrets_are_redacted() {
    let generation = GenerationProviderCredentialId::new("legacy.primary").unwrap();
    let assistant = AssistantModelCredentialId::new("assistant.openai.default").unwrap();
    let secret = GenerationProviderCredentialSecret::new(b"private-key".to_vec()).unwrap();

    assert_eq!(generation.as_str(), "legacy.primary");
    assert_eq!(assistant.as_str(), "assistant.openai.default");
    assert!(!format!("{secret:?}").contains("private-key"));
    assert!(GenerationProviderCredentialId::new("Legacy Key").is_err());
}

#[tokio::test]
async fn generation_repository_contract_saves_loads_deletes_and_reports_not_found() {
    let repository = GenerationRepositoryFakeImpl::default();
    let id = GenerationProviderCredentialId::new("legacy.primary").unwrap();
    repository
        .save_generation_provider_credential(
            id.clone(),
            GenerationProviderCredentialSecret::new(b"secret".to_vec()).unwrap(),
        )
        .await
        .unwrap();

    let loaded = repository.load_generation_provider_credential(&id).await.unwrap();
    assert_eq!(loaded.as_bytes(), b"secret");
    repository.delete_generation_provider_credential(&id).await.unwrap();
    repository.delete_generation_provider_credential(&id).await.unwrap();
    assert_eq!(
        repository.load_generation_provider_credential(&id).await.unwrap_err(),
        GenerationProviderCredentialRepositoryError::NotFound
    );
}

#[tokio::test]
async fn assistant_repository_is_isolated_and_preserves_access_failures() {
    let repository = AssistantRepositoryFakeImpl::default();
    let id = AssistantModelCredentialId::new("assistant.openai.default").unwrap();
    repository
        .save_assistant_model_credential(
            id.clone(),
            AssistantModelCredentialSecret::new(b"assistant-secret".to_vec()).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        repository.load_assistant_model_credential(&id).await.unwrap().as_bytes(),
        b"assistant-secret"
    );

    *repository.failure.lock().unwrap() =
        Some(AssistantModelCredentialRepositoryError::PermissionDenied);
    assert_eq!(
        repository.load_assistant_model_credential(&id).await.unwrap_err(),
        AssistantModelCredentialRepositoryError::PermissionDenied
    );
    *repository.failure.lock().unwrap() =
        Some(AssistantModelCredentialRepositoryError::Unavailable);
    assert_eq!(
        repository.delete_assistant_model_credential(&id).await.unwrap_err(),
        AssistantModelCredentialRepositoryError::Unavailable
    );
}

#[test]
fn secrets_and_errors_never_include_plaintext() {
    let secret = AssistantModelCredentialSecret::new(b"do-not-log".to_vec()).unwrap();
    let diagnostics =
        format!("{secret:?} {:?}", AssistantModelCredentialRepositoryError::InvalidCredential);
    assert!(!diagnostics.contains("do-not-log"));
}

#[test]
fn credential_secrets_enforce_the_plaintext_storage_bound() {
    assert!(GenerationProviderCredentialSecret::new(vec![1; 16 * 1024]).is_ok());
    assert!(GenerationProviderCredentialSecret::new(vec![1; 16 * 1024 + 1]).is_err());
    assert!(AssistantModelCredentialSecret::new(Vec::new()).is_err());
}
