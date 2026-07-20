//! Tauri boundary for the Assistant OpenAI Responses provider connection.

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    assistant_provider_settings::{
        AssistantProviderApiKey, AssistantProviderBaseUrl, AssistantProviderModelId,
        AssistantProviderModelsListUseCase, AssistantProviderProbeInterface,
        AssistantProviderSettingsDisableUseCase, AssistantProviderSettingsError,
        AssistantProviderSettingsGetUseCase, AssistantProviderSettingsRepositoryInterface,
        AssistantProviderSettingsRevision, AssistantProviderSettingsTestAndApplyUseCase,
        AssistantProviderSettingsView,
    },
    composition::DesktopActivatedCommandDependencies,
    desktop_backend_config::{DesktopErrorCode, DesktopErrorContext, DesktopErrorDto},
};

#[cfg(test)]
#[path = "assistant_provider_settings_commands_tests.rs"]
mod tests;

/// Empty request for the singleton sanitized settings view.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssistantProviderSettingsGetRequestDto {}

/// Candidate connection used only to discover models.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssistantProviderModelsListRequestDto {
    /// Candidate normalized provider endpoint.
    pub base_url: String,
    /// Replacement write-only key, or `None` to reuse the stored key.
    pub api_key: Option<String>,
}

/// Candidate connection persisted only after its Responses compatibility test succeeds.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssistantProviderSettingsTestAndApplyRequestDto {
    /// Revision compared before the final atomic write.
    pub expected_settings_revision: String,
    /// Candidate normalized provider endpoint.
    pub base_url: String,
    /// Replacement write-only key, or `None` to reuse the stored key.
    pub api_key: Option<String>,
    /// Candidate provider-native model identifier.
    pub model_id: String,
}

/// Expected-revision request that disables new Assistant invocations.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssistantProviderSettingsDisableRequestDto {
    /// Revision compared by the disable mutation.
    pub expected_settings_revision: String,
}

/// Complete sanitized Assistant provider settings projection.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AssistantProviderSettingsDto {
    /// Current monotonic settings revision.
    pub settings_revision: String,
    /// Whether new Assistant invocations are enabled.
    pub enabled: bool,
    /// Current normalized provider endpoint.
    pub base_url: String,
    /// Last successfully tested model identifier.
    pub model_id: Option<String>,
    /// Whether the fixed write-only credential exists.
    pub has_api_key: bool,
}

/// Sorted provider-native model IDs returned by `GET /models`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AssistantProviderModelsDto {
    /// Sorted, de-duplicated provider-native model identifiers.
    pub models: Vec<String>,
}

/// Returns the current sanitized Assistant provider settings.
#[tauri::command(rename_all = "snake_case")]
pub async fn assistant_provider_settings_get(
    _request: AssistantProviderSettingsGetRequestDto,
    state: State<'_, DesktopActivatedCommandDependencies>,
) -> Result<AssistantProviderSettingsDto, DesktopErrorDto> {
    get_with_use_case(&state.assistant_provider_settings_get).await
}

/// Discovers models through one candidate connection without writing settings.
#[tauri::command(rename_all = "snake_case")]
pub async fn assistant_provider_models_list(
    request: AssistantProviderModelsListRequestDto,
    state: State<'_, DesktopActivatedCommandDependencies>,
) -> Result<AssistantProviderModelsDto, DesktopErrorDto> {
    list_with_use_case(request, &state.assistant_provider_models_list).await
}

/// Tests one model and atomically saves the connection only after success.
#[tauri::command(rename_all = "snake_case")]
pub async fn assistant_provider_settings_test_and_apply(
    request: AssistantProviderSettingsTestAndApplyRequestDto,
    state: State<'_, DesktopActivatedCommandDependencies>,
) -> Result<AssistantProviderSettingsDto, DesktopErrorDto> {
    test_and_apply_with_use_case(request, &state.assistant_provider_settings_test_and_apply).await
}

/// Disables new Assistant invocations while retaining the tested connection.
#[tauri::command(rename_all = "snake_case")]
pub async fn assistant_provider_settings_disable(
    request: AssistantProviderSettingsDisableRequestDto,
    state: State<'_, DesktopActivatedCommandDependencies>,
) -> Result<AssistantProviderSettingsDto, DesktopErrorDto> {
    disable_with_use_case(request, &state.assistant_provider_settings_disable).await
}

async fn get_with_use_case<R: AssistantProviderSettingsRepositoryInterface>(
    use_case: &AssistantProviderSettingsGetUseCase<R>,
) -> Result<AssistantProviderSettingsDto, DesktopErrorDto> {
    use_case.get_assistant_provider_settings().await.map(settings_dto).map_err(settings_error)
}

async fn list_with_use_case<P, R>(
    request: AssistantProviderModelsListRequestDto,
    use_case: &AssistantProviderModelsListUseCase<P, R>,
) -> Result<AssistantProviderModelsDto, DesktopErrorDto>
where
    P: AssistantProviderProbeInterface,
    R: AssistantProviderSettingsRepositoryInterface,
{
    let models = use_case
        .list_assistant_provider_models(
            parse_base_url(&request.base_url)?,
            parse_api_key(request.api_key)?,
        )
        .await
        .map_err(settings_error)?;
    Ok(AssistantProviderModelsDto {
        models: models.into_iter().map(|model| model.as_str().to_owned()).collect(),
    })
}

async fn test_and_apply_with_use_case<P, R>(
    request: AssistantProviderSettingsTestAndApplyRequestDto,
    use_case: &AssistantProviderSettingsTestAndApplyUseCase<P, R>,
) -> Result<AssistantProviderSettingsDto, DesktopErrorDto>
where
    P: AssistantProviderProbeInterface,
    R: AssistantProviderSettingsRepositoryInterface,
{
    use_case
        .test_and_apply_assistant_provider_settings(
            parse_revision(&request.expected_settings_revision)?,
            parse_base_url(&request.base_url)?,
            parse_api_key(request.api_key)?,
            AssistantProviderModelId::try_new(request.model_id).map_err(|_| invalid_request())?,
        )
        .await
        .map(settings_dto)
        .map_err(settings_error)
}

async fn disable_with_use_case<R: AssistantProviderSettingsRepositoryInterface>(
    request: AssistantProviderSettingsDisableRequestDto,
    use_case: &AssistantProviderSettingsDisableUseCase<R>,
) -> Result<AssistantProviderSettingsDto, DesktopErrorDto> {
    use_case
        .disable_assistant_provider_settings(parse_revision(&request.expected_settings_revision)?)
        .await
        .map(settings_dto)
        .map_err(settings_error)
}

fn parse_base_url(value: &str) -> Result<AssistantProviderBaseUrl, DesktopErrorDto> {
    AssistantProviderBaseUrl::try_new(value).map_err(|_| invalid_request())
}

fn parse_api_key(
    value: Option<String>,
) -> Result<Option<AssistantProviderApiKey>, DesktopErrorDto> {
    value
        .map(|key| {
            AssistantProviderApiKey::try_new(key.into_bytes()).map_err(|_| invalid_request())
        })
        .transpose()
}

fn parse_revision(value: &str) -> Result<AssistantProviderSettingsRevision, DesktopErrorDto> {
    if value.starts_with('0') {
        return Err(invalid_request());
    }
    value.parse().ok().and_then(AssistantProviderSettingsRevision::new).ok_or_else(invalid_request)
}

fn settings_dto(view: AssistantProviderSettingsView) -> AssistantProviderSettingsDto {
    AssistantProviderSettingsDto {
        settings_revision: view.settings_revision.get().to_string(),
        enabled: view.enabled,
        base_url: view.base_url.as_str().to_owned(),
        model_id: view.model_id.map(|model| model.as_str().to_owned()),
        has_api_key: view.has_api_key,
    }
}

fn settings_error(error: AssistantProviderSettingsError) -> DesktopErrorDto {
    let code = match error {
        AssistantProviderSettingsError::RevisionConflict => {
            DesktopErrorCode::AssistantProviderSettingsRevisionConflict
        }
        AssistantProviderSettingsError::MissingCredential => {
            DesktopErrorCode::AssistantProviderCredentialMissing
        }
        AssistantProviderSettingsError::AuthenticationRejected => {
            DesktopErrorCode::AssistantProviderAuthenticationRejected
        }
        AssistantProviderSettingsError::ProviderTimedOut => {
            DesktopErrorCode::AssistantProviderTimedOut
        }
        AssistantProviderSettingsError::MissingFunctionToolBehavior
        | AssistantProviderSettingsError::SelectedModelRejected => {
            DesktopErrorCode::AssistantProviderIncompatible
        }
        AssistantProviderSettingsError::StorageUnavailable => DesktopErrorCode::StorageUnavailable,
        _ => DesktopErrorCode::AssistantProviderUnavailable,
    };
    DesktopErrorDto::from_context(DesktopErrorContext {
        code,
        retryable: matches!(
            error,
            AssistantProviderSettingsError::ProviderUnreachable
                | AssistantProviderSettingsError::ProviderTimedOut
        ),
        retry_after_epoch_ms: None,
        target: None,
        correlation_id: None,
    })
}

fn invalid_request() -> DesktopErrorDto {
    DesktopErrorDto::from_context(DesktopErrorContext {
        code: DesktopErrorCode::AssistantProviderSettingsInvalidRequest,
        retryable: false,
        retry_after_epoch_ms: None,
        target: None,
        correlation_id: None,
    })
}
