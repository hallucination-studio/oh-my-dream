use async_trait::async_trait;

use super::{
    GenerationProviderSettingsError, GenerationProviderSettingsMutation,
    GenerationProviderSettingsMutationResult, GenerationProviderSettingsRevision,
    GenerationProviderSettingsSnapshot,
};

/// Atomic persistence boundary consumed by Generation Provider Settings.
#[async_trait]
pub trait GenerationProviderSettingsRepositoryInterface: Send + Sync {
    /// Loads or initializes the current Settings snapshot.
    async fn load_generation_provider_settings_snapshot(
        &self,
    ) -> Result<GenerationProviderSettingsSnapshot, GenerationProviderSettingsError>;

    /// Applies one validated mutation under expected-revision CAS.
    async fn apply_generation_provider_settings_mutation(
        &self,
        expected_revision: GenerationProviderSettingsRevision,
        mutation: GenerationProviderSettingsMutation,
    ) -> Result<GenerationProviderSettingsMutationResult, GenerationProviderSettingsError>;
}
