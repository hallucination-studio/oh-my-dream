use std::sync::Arc;

use super::{
    AssistantProviderApiKey, AssistantProviderBaseUrl, AssistantProviderModelId,
    AssistantProviderProbeError, AssistantProviderProbeInterface,
    AssistantProviderSettingsMutation, AssistantProviderSettingsMutationResult,
    AssistantProviderSettingsRepositoryError, AssistantProviderSettingsRepositoryInterface,
    AssistantProviderSettingsRevision, AssistantProviderSettingsView, normalize_model_list,
};

/// Closed application failures for Assistant Provider Settings commands.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum AssistantProviderSettingsError {
    #[error("Assistant provider credential is missing")]
    MissingCredential,
    #[error("Assistant provider authentication was rejected")]
    AuthenticationRejected,
    #[error("Assistant provider is unreachable")]
    ProviderUnreachable,
    #[error("Assistant provider operation timed out")]
    ProviderTimedOut,
    #[error("Assistant provider Models endpoint is unavailable")]
    ModelsEndpointUnavailable,
    #[error("Assistant provider Models response is invalid")]
    InvalidModelsResponse,
    #[error("Assistant provider rejected the selected model")]
    SelectedModelRejected,
    #[error("Assistant provider Responses endpoint is unavailable")]
    ResponsesEndpointUnavailable,
    #[error("Assistant provider model lacks required function-tool behavior")]
    MissingFunctionToolBehavior,
    #[error("Assistant provider settings revision conflict")]
    RevisionConflict,
    #[error("Assistant provider settings storage is unavailable")]
    StorageUnavailable,
}

/// Returns the sanitized current Assistant Provider Settings view.
pub struct AssistantProviderSettingsGetUseCase<R> {
    repository: Arc<R>,
}

impl<R: AssistantProviderSettingsRepositoryInterface> AssistantProviderSettingsGetUseCase<R> {
    /// Uses one Assistant Provider Settings repository.
    #[must_use]
    pub const fn new(repository: Arc<R>) -> Self {
        Self { repository }
    }

    /// Loads current sanitized settings without loading credential bytes.
    pub async fn get_assistant_provider_settings(
        &self,
    ) -> Result<AssistantProviderSettingsView, AssistantProviderSettingsError> {
        self.repository
            .load_assistant_provider_settings_snapshot()
            .await
            .map(Into::into)
            .map_err(map_repository_error)
    }
}

/// Lists models through one candidate Assistant provider connection.
pub struct AssistantProviderModelsListUseCase<P, R> {
    probe: Arc<P>,
    repository: Arc<R>,
}

impl<P, R> AssistantProviderModelsListUseCase<P, R>
where
    P: AssistantProviderProbeInterface,
    R: AssistantProviderSettingsRepositoryInterface,
{
    /// Uses one probe and Settings repository.
    #[must_use]
    pub const fn new(probe: Arc<P>, repository: Arc<R>) -> Self {
        Self { probe, repository }
    }

    /// Lists sorted, de-duplicated model identifiers without writing settings.
    pub async fn list_assistant_provider_models(
        &self,
        base_url: AssistantProviderBaseUrl,
        supplied_api_key: Option<AssistantProviderApiKey>,
    ) -> Result<Vec<AssistantProviderModelId>, AssistantProviderSettingsError> {
        let stored_api_key;
        let api_key = match supplied_api_key.as_ref() {
            Some(value) => value,
            None => {
                stored_api_key = self
                    .repository
                    .load_assistant_provider_api_key()
                    .await
                    .map_err(map_repository_error)?;
                &stored_api_key
            }
        };
        let models = self
            .probe
            .list_assistant_provider_models(&base_url, api_key)
            .await
            .map_err(map_probe_error)?;
        normalize_model_list(models)
            .map_err(|_| AssistantProviderSettingsError::InvalidModelsResponse)
    }
}

/// Tests one candidate connection and atomically persists it only on success.
pub struct AssistantProviderSettingsTestAndApplyUseCase<P, R> {
    probe: Arc<P>,
    repository: Arc<R>,
}

impl<P, R> AssistantProviderSettingsTestAndApplyUseCase<P, R>
where
    P: AssistantProviderProbeInterface,
    R: AssistantProviderSettingsRepositoryInterface,
{
    /// Uses one probe and Settings repository.
    #[must_use]
    pub const fn new(probe: Arc<P>, repository: Arc<R>) -> Self {
        Self { probe, repository }
    }

    /// Runs the Responses compatibility test before requesting one atomic CAS mutation.
    pub async fn test_and_apply_assistant_provider_settings(
        &self,
        expected_revision: AssistantProviderSettingsRevision,
        base_url: AssistantProviderBaseUrl,
        supplied_api_key: Option<AssistantProviderApiKey>,
        model_id: AssistantProviderModelId,
    ) -> Result<AssistantProviderSettingsView, AssistantProviderSettingsError> {
        let stored_api_key;
        let api_key = match supplied_api_key.as_ref() {
            Some(value) => value,
            None => {
                stored_api_key = self
                    .repository
                    .load_assistant_provider_api_key()
                    .await
                    .map_err(map_repository_error)?;
                &stored_api_key
            }
        };
        self.probe
            .test_assistant_provider_model(&base_url, api_key, &model_id)
            .await
            .map_err(map_probe_error)?;
        apply_mutation(
            &*self.repository,
            expected_revision,
            AssistantProviderSettingsMutation::ApplyTestedConnection {
                base_url,
                model_id,
                api_key: supplied_api_key,
            },
        )
        .await
    }
}

/// Disables new Assistant invocations without contacting the provider.
pub struct AssistantProviderSettingsDisableUseCase<R> {
    repository: Arc<R>,
}

impl<R: AssistantProviderSettingsRepositoryInterface> AssistantProviderSettingsDisableUseCase<R> {
    /// Uses one Assistant Provider Settings repository.
    #[must_use]
    pub const fn new(repository: Arc<R>) -> Self {
        Self { repository }
    }

    /// Disables under expected-revision CAS while retaining the tested connection.
    pub async fn disable_assistant_provider_settings(
        &self,
        expected_revision: AssistantProviderSettingsRevision,
    ) -> Result<AssistantProviderSettingsView, AssistantProviderSettingsError> {
        apply_mutation(
            &*self.repository,
            expected_revision,
            AssistantProviderSettingsMutation::Disable,
        )
        .await
    }
}

async fn apply_mutation<R: AssistantProviderSettingsRepositoryInterface>(
    repository: &R,
    expected_revision: AssistantProviderSettingsRevision,
    mutation: AssistantProviderSettingsMutation,
) -> Result<AssistantProviderSettingsView, AssistantProviderSettingsError> {
    match repository
        .apply_assistant_provider_settings_mutation(expected_revision, mutation)
        .await
        .map_err(map_repository_error)?
    {
        AssistantProviderSettingsMutationResult::Committed(snapshot) => Ok(snapshot.into()),
        AssistantProviderSettingsMutationResult::RevisionConflict => {
            Err(AssistantProviderSettingsError::RevisionConflict)
        }
    }
}

const fn map_probe_error(error: AssistantProviderProbeError) -> AssistantProviderSettingsError {
    match error {
        AssistantProviderProbeError::AuthenticationRejected => {
            AssistantProviderSettingsError::AuthenticationRejected
        }
        AssistantProviderProbeError::ProviderUnreachable => {
            AssistantProviderSettingsError::ProviderUnreachable
        }
        AssistantProviderProbeError::ProviderTimedOut => {
            AssistantProviderSettingsError::ProviderTimedOut
        }
        AssistantProviderProbeError::ModelsEndpointUnavailable => {
            AssistantProviderSettingsError::ModelsEndpointUnavailable
        }
        AssistantProviderProbeError::InvalidModelsResponse => {
            AssistantProviderSettingsError::InvalidModelsResponse
        }
        AssistantProviderProbeError::SelectedModelRejected => {
            AssistantProviderSettingsError::SelectedModelRejected
        }
        AssistantProviderProbeError::ResponsesEndpointUnavailable => {
            AssistantProviderSettingsError::ResponsesEndpointUnavailable
        }
        AssistantProviderProbeError::MissingFunctionToolBehavior => {
            AssistantProviderSettingsError::MissingFunctionToolBehavior
        }
    }
}

const fn map_repository_error(
    error: AssistantProviderSettingsRepositoryError,
) -> AssistantProviderSettingsError {
    match error {
        AssistantProviderSettingsRepositoryError::MissingCredential => {
            AssistantProviderSettingsError::MissingCredential
        }
        AssistantProviderSettingsRepositoryError::InvalidSnapshot
        | AssistantProviderSettingsRepositoryError::Unavailable => {
            AssistantProviderSettingsError::StorageUnavailable
        }
    }
}

#[cfg(test)]
#[path = "application_tests.rs"]
mod tests;
