use async_trait::async_trait;

use super::{
    AssistantModelCredentialId, AssistantModelCredentialRepositoryError,
    AssistantModelCredentialSecret, GenerationProviderCredentialId,
    GenerationProviderCredentialRepositoryError, GenerationProviderCredentialSecret,
};

/// Plaintext durable storage consumed by generation-provider configuration.
#[async_trait]
pub trait GenerationProviderCredentialRepositoryInterface: Send + Sync {
    async fn save_generation_provider_credential(
        &self,
        id: GenerationProviderCredentialId,
        secret: GenerationProviderCredentialSecret,
    ) -> Result<(), GenerationProviderCredentialRepositoryError>;

    async fn load_generation_provider_credential(
        &self,
        id: &GenerationProviderCredentialId,
    ) -> Result<GenerationProviderCredentialSecret, GenerationProviderCredentialRepositoryError>;

    async fn delete_generation_provider_credential(
        &self,
        id: &GenerationProviderCredentialId,
    ) -> Result<(), GenerationProviderCredentialRepositoryError>;
}

/// Plaintext durable storage consumed by Assistant-model configuration.
#[async_trait]
pub trait AssistantModelCredentialRepositoryInterface: Send + Sync {
    async fn save_assistant_model_credential(
        &self,
        id: AssistantModelCredentialId,
        secret: AssistantModelCredentialSecret,
    ) -> Result<(), AssistantModelCredentialRepositoryError>;

    async fn load_assistant_model_credential(
        &self,
        id: &AssistantModelCredentialId,
    ) -> Result<AssistantModelCredentialSecret, AssistantModelCredentialRepositoryError>;

    async fn delete_assistant_model_credential(
        &self,
        id: &AssistantModelCredentialId,
    ) -> Result<(), AssistantModelCredentialRepositoryError>;
}
