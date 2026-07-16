use std::{collections::BTreeMap, sync::Mutex};

use async_trait::async_trait;
use oh_my_dream_tauri::credential_vault::*;

#[derive(Default)]
struct GenerationVaultFake {
    values: Mutex<BTreeMap<GenerationProviderCredentialId, Vec<u8>>>,
}

#[derive(Default)]
struct AssistantVaultFake {
    values: Mutex<BTreeMap<AssistantModelCredentialId, Vec<u8>>>,
    failure: Mutex<Option<AssistantModelCredentialVaultError>>,
}

#[async_trait]
impl AssistantModelCredentialVaultInterface for AssistantVaultFake {
    async fn save_assistant_model_credential(
        &self,
        id: AssistantModelCredentialId,
        secret: AssistantModelCredentialSecret,
    ) -> Result<(), AssistantModelCredentialVaultError> {
        if let Some(error) = *self.failure.lock().unwrap() {
            return Err(error);
        }
        self.values.lock().unwrap().insert(id, secret.as_bytes().to_vec());
        Ok(())
    }

    async fn load_assistant_model_credential(
        &self,
        id: &AssistantModelCredentialId,
    ) -> Result<AssistantModelCredentialSecret, AssistantModelCredentialVaultError> {
        if let Some(error) = *self.failure.lock().unwrap() {
            return Err(error);
        }
        self.values
            .lock()
            .unwrap()
            .get(id)
            .cloned()
            .ok_or(AssistantModelCredentialVaultError::NotFound)
            .and_then(AssistantModelCredentialSecret::new)
    }

    async fn delete_assistant_model_credential(
        &self,
        id: &AssistantModelCredentialId,
    ) -> Result<(), AssistantModelCredentialVaultError> {
        if let Some(error) = *self.failure.lock().unwrap() {
            return Err(error);
        }
        self.values.lock().unwrap().remove(id);
        Ok(())
    }
}

#[async_trait]
impl GenerationProviderCredentialVaultInterface for GenerationVaultFake {
    async fn save_generation_provider_credential(
        &self,
        id: GenerationProviderCredentialId,
        secret: GenerationProviderCredentialSecret,
    ) -> Result<(), GenerationProviderCredentialVaultError> {
        self.values.lock().unwrap().insert(id, secret.as_bytes().to_vec());
        Ok(())
    }

    async fn load_generation_provider_credential(
        &self,
        id: &GenerationProviderCredentialId,
    ) -> Result<GenerationProviderCredentialSecret, GenerationProviderCredentialVaultError> {
        self.values
            .lock()
            .unwrap()
            .get(id)
            .cloned()
            .ok_or(GenerationProviderCredentialVaultError::NotFound)
            .and_then(GenerationProviderCredentialSecret::new)
    }

    async fn delete_generation_provider_credential(
        &self,
        id: &GenerationProviderCredentialId,
    ) -> Result<(), GenerationProviderCredentialVaultError> {
        self.values.lock().unwrap().remove(id);
        Ok(())
    }
}

#[test]
fn credential_ids_are_typed_isolated_and_secrets_are_redacted() {
    let generation = GenerationProviderCredentialId::new("fal.primary").unwrap();
    let assistant = AssistantModelCredentialId::new("assistant.openai.default").unwrap();
    let secret = GenerationProviderCredentialSecret::new(b"private-key".to_vec()).unwrap();

    assert_eq!(generation.as_str(), "fal.primary");
    assert_eq!(assistant.as_str(), "assistant.openai.default");
    assert!(!format!("{secret:?}").contains("private-key"));
    assert!(GenerationProviderCredentialId::new("Fal Key").is_err());
}

#[tokio::test]
async fn generation_vault_contract_saves_loads_deletes_and_reports_not_found() {
    let vault = GenerationVaultFake::default();
    let id = GenerationProviderCredentialId::new("fal.primary").unwrap();
    vault
        .save_generation_provider_credential(
            id.clone(),
            GenerationProviderCredentialSecret::new(b"secret".to_vec()).unwrap(),
        )
        .await
        .unwrap();

    let loaded = vault.load_generation_provider_credential(&id).await.unwrap();
    assert_eq!(loaded.as_bytes(), b"secret");
    vault.delete_generation_provider_credential(&id).await.unwrap();
    assert_eq!(
        vault.load_generation_provider_credential(&id).await.unwrap_err(),
        GenerationProviderCredentialVaultError::NotFound
    );
}

#[tokio::test]
async fn assistant_vault_is_isolated_and_preserves_access_failures() {
    let vault = AssistantVaultFake::default();
    let id = AssistantModelCredentialId::new("assistant.openai.default").unwrap();
    vault
        .save_assistant_model_credential(
            id.clone(),
            AssistantModelCredentialSecret::new(b"assistant-secret".to_vec()).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        vault.load_assistant_model_credential(&id).await.unwrap().as_bytes(),
        b"assistant-secret"
    );

    *vault.failure.lock().unwrap() = Some(AssistantModelCredentialVaultError::Denied);
    assert_eq!(
        vault.load_assistant_model_credential(&id).await.unwrap_err(),
        AssistantModelCredentialVaultError::Denied
    );
    *vault.failure.lock().unwrap() = Some(AssistantModelCredentialVaultError::Unavailable);
    assert_eq!(
        vault.delete_assistant_model_credential(&id).await.unwrap_err(),
        AssistantModelCredentialVaultError::Unavailable
    );
}

#[test]
fn secrets_and_errors_never_include_plaintext() {
    let secret = AssistantModelCredentialSecret::new(b"do-not-log".to_vec()).unwrap();
    let diagnostics =
        format!("{secret:?} {:?}", AssistantModelCredentialVaultError::InvalidCredential);
    assert!(!diagnostics.contains("do-not-log"));
}
