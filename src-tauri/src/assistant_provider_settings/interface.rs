use async_trait::async_trait;

use super::{
    AssistantProviderApiKey, AssistantProviderBaseUrl, AssistantProviderModelId,
    AssistantProviderSettingsMutation, AssistantProviderSettingsMutationResult,
    AssistantProviderSettingsRevision, AssistantProviderSettingsSnapshot,
};

/// Closed failures returned by the external OpenAI-compatible probe.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum AssistantProviderProbeError {
    /// The provider rejected the candidate credential.
    #[error("Assistant provider authentication was rejected")]
    AuthenticationRejected,
    /// The provider could not be reached.
    #[error("Assistant provider is unreachable")]
    ProviderUnreachable,
    /// The bounded provider operation timed out.
    #[error("Assistant provider operation timed out")]
    ProviderTimedOut,
    /// The Models endpoint is unavailable.
    #[error("Assistant provider Models endpoint is unavailable")]
    ModelsEndpointUnavailable,
    /// The Models response failed strict validation.
    #[error("Assistant provider Models response is invalid")]
    InvalidModelsResponse,
    /// The provider rejected the selected model.
    #[error("Assistant provider rejected the selected model")]
    SelectedModelRejected,
    /// The Responses endpoint is unavailable.
    #[error("Assistant provider Responses endpoint is unavailable")]
    ResponsesEndpointUnavailable,
    /// The tested response omitted the exact required function call.
    #[error("Assistant provider model lacks required function-tool behavior")]
    MissingFunctionToolBehavior,
}

/// Failures owned by durable Assistant Provider Settings persistence.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum AssistantProviderSettingsRepositoryError {
    /// Persisted state violates the settings contract.
    #[error("Assistant provider settings snapshot is invalid")]
    InvalidSnapshot,
    /// No stored Assistant credential exists.
    #[error("Assistant provider credential is missing")]
    MissingCredential,
    /// Settings storage is unavailable.
    #[error("Assistant provider settings storage is unavailable")]
    Unavailable,
}

/// External model discovery and Responses compatibility boundary.
#[async_trait]
pub trait AssistantProviderProbeInterface: Send + Sync {
    /// Lists bounded validated model IDs from the candidate endpoint.
    async fn list_assistant_provider_models(
        &self,
        base_url: &AssistantProviderBaseUrl,
        api_key: &AssistantProviderApiKey,
    ) -> Result<Vec<AssistantProviderModelId>, AssistantProviderProbeError>;

    /// Requires the exact no-argument function call from one Responses request.
    async fn test_assistant_provider_model(
        &self,
        base_url: &AssistantProviderBaseUrl,
        api_key: &AssistantProviderApiKey,
        model_id: &AssistantProviderModelId,
    ) -> Result<(), AssistantProviderProbeError>;
}

/// Atomic persistence boundary consumed by Assistant Provider Settings.
#[async_trait]
pub trait AssistantProviderSettingsRepositoryInterface: Send + Sync {
    /// Loads the current sanitized Settings snapshot.
    async fn load_assistant_provider_settings_snapshot(
        &self,
    ) -> Result<AssistantProviderSettingsSnapshot, AssistantProviderSettingsRepositoryError>;

    /// Loads the fixed write-only credential for immediate provider use.
    async fn load_assistant_provider_api_key(
        &self,
    ) -> Result<AssistantProviderApiKey, AssistantProviderSettingsRepositoryError>;

    /// Applies one mutation under expected-revision CAS in one transaction.
    async fn apply_assistant_provider_settings_mutation(
        &self,
        expected_revision: AssistantProviderSettingsRevision,
        mutation: AssistantProviderSettingsMutation,
    ) -> Result<AssistantProviderSettingsMutationResult, AssistantProviderSettingsRepositoryError>;
}
