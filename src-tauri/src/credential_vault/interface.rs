use async_trait::async_trait;

use super::{
    AssistantModelCredentialId, AssistantModelCredentialSecret, AssistantModelCredentialVaultError,
    GenerationProviderCredentialId, GenerationProviderCredentialSecret,
    GenerationProviderCredentialVaultError,
};

/// Secure storage consumed by generation-provider configuration.
#[async_trait]
pub trait GenerationProviderCredentialVaultInterface: Send + Sync {
    async fn save_generation_provider_credential(
        &self,
        id: GenerationProviderCredentialId,
        secret: GenerationProviderCredentialSecret,
    ) -> Result<(), GenerationProviderCredentialVaultError>;

    async fn load_generation_provider_credential(
        &self,
        id: &GenerationProviderCredentialId,
    ) -> Result<GenerationProviderCredentialSecret, GenerationProviderCredentialVaultError>;

    async fn delete_generation_provider_credential(
        &self,
        id: &GenerationProviderCredentialId,
    ) -> Result<(), GenerationProviderCredentialVaultError>;
}

/// Secure storage consumed by Assistant-model configuration.
#[async_trait]
pub trait AssistantModelCredentialVaultInterface: Send + Sync {
    async fn save_assistant_model_credential(
        &self,
        id: AssistantModelCredentialId,
        secret: AssistantModelCredentialSecret,
    ) -> Result<(), AssistantModelCredentialVaultError>;

    async fn load_assistant_model_credential(
        &self,
        id: &AssistantModelCredentialId,
    ) -> Result<AssistantModelCredentialSecret, AssistantModelCredentialVaultError>;

    async fn delete_assistant_model_credential(
        &self,
        id: &AssistantModelCredentialId,
    ) -> Result<(), AssistantModelCredentialVaultError>;
}
