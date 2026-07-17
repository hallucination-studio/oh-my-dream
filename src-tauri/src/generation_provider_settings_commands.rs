//! Tauri boundary for sanitized Mock Generation Provider Settings.

use backends::generation_provider_settings::{
    GenerationProviderSettingsBinding, GenerationProviderSettingsError,
    GenerationProviderSettingsMutation, GenerationProviderSettingsRevision,
    GenerationProviderSettingsView,
};
use nodes::{GenerationProfileId, GenerationProfileRef, GenerationProfileVersion};
use serde::{Deserialize, Serialize};
use tasks::generation_task::{
    GenerationProviderId, GenerationProviderRouteId, GenerationTaskRequestKind,
};
use tauri::State;

use crate::{
    composition::DesktopActivatedCommandDependencies,
    desktop_backend_config::{DesktopErrorCode, DesktopErrorContext, DesktopErrorDto},
};

#[cfg(test)]
mod tests;

/// Empty request for the singleton Settings projection.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GenerationProviderSettingsGetRequestDto {}

/// One closed Settings mutation action.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum GenerationProviderSettingsActionDto {
    /// Selects one exact safe Mock route.
    SetBinding {
        profile_ref: String,
        generation_kind: String,
        provider_id: String,
        route_id: String,
    },
    /// Removes one exact profile/kind binding.
    RemoveBinding { profile_ref: String, generation_kind: String },
}

/// Expected-revision CAS request.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GenerationProviderSettingsApplyRequestDto {
    pub expected_settings_revision: String,
    pub action: GenerationProviderSettingsActionDto,
}

/// Complete sanitized Settings projection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct GenerationProviderSettingsDto {
    pub settings_revision: String,
    pub profiles: Vec<GenerationProviderSettingsProfileDto>,
}

/// One profile/kind Settings item.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct GenerationProviderSettingsProfileDto {
    pub profile_ref: String,
    pub generation_kind: String,
    pub selected_binding: Option<GenerationProviderSettingsBindingDto>,
    pub provider_choices: Vec<GenerationProviderSettingsProviderChoiceDto>,
}

/// Sanitized selected binding.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct GenerationProviderSettingsBindingDto {
    pub provider_id: String,
    pub route_id: String,
}

/// Safe provider choice.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct GenerationProviderSettingsProviderChoiceDto {
    pub provider_id: String,
    pub display_name: String,
    pub routes: Vec<GenerationProviderSettingsRouteChoiceDto>,
}

/// Safe route choice.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct GenerationProviderSettingsRouteChoiceDto {
    pub route_id: String,
    pub display_name: String,
}

/// Gets current sanitized Generation Provider Settings.
#[tauri::command(rename_all = "snake_case")]
pub async fn generation_provider_settings_get(
    _request: GenerationProviderSettingsGetRequestDto,
    state: State<'_, DesktopActivatedCommandDependencies>,
) -> Result<GenerationProviderSettingsDto, DesktopErrorDto> {
    generation_provider_settings_get_with_dependencies(&state).await
}

/// Gets Settings against explicit already-composed dependencies.
pub async fn generation_provider_settings_get_with_dependencies(
    state: &DesktopActivatedCommandDependencies,
) -> Result<GenerationProviderSettingsDto, DesktopErrorDto> {
    state
        .generation_provider_settings_get
        .get_generation_provider_settings()
        .await
        .map(settings_dto)
        .map_err(settings_error)
}

/// Applies one expected-revision Settings mutation.
#[tauri::command(rename_all = "snake_case")]
pub async fn generation_provider_settings_apply(
    request: GenerationProviderSettingsApplyRequestDto,
    state: State<'_, DesktopActivatedCommandDependencies>,
) -> Result<GenerationProviderSettingsDto, DesktopErrorDto> {
    generation_provider_settings_apply_with_dependencies(request, &state).await
}

/// Applies Settings against explicit already-composed dependencies.
pub async fn generation_provider_settings_apply_with_dependencies(
    request: GenerationProviderSettingsApplyRequestDto,
    state: &DesktopActivatedCommandDependencies,
) -> Result<GenerationProviderSettingsDto, DesktopErrorDto> {
    let expected_revision = parse_revision(&request.expected_settings_revision)?;
    let mutation = parse_action(request.action)?;
    state
        .generation_provider_settings_apply
        .apply_generation_provider_settings(expected_revision, mutation)
        .await
        .map(settings_dto)
        .map_err(settings_error)
}

fn parse_action(
    action: GenerationProviderSettingsActionDto,
) -> Result<GenerationProviderSettingsMutation, DesktopErrorDto> {
    match action {
        GenerationProviderSettingsActionDto::SetBinding {
            profile_ref,
            generation_kind,
            provider_id,
            route_id,
        } => Ok(GenerationProviderSettingsMutation::SetBinding(
            GenerationProviderSettingsBinding::new(
                parse_profile_ref(&profile_ref)?,
                parse_kind(&generation_kind)?,
                GenerationProviderId::try_new(provider_id).map_err(|_| invalid_request())?,
                GenerationProviderRouteId::try_new(route_id).map_err(|_| invalid_request())?,
            ),
        )),
        GenerationProviderSettingsActionDto::RemoveBinding { profile_ref, generation_kind } => {
            Ok(GenerationProviderSettingsMutation::RemoveBinding {
                profile_ref: parse_profile_ref(&profile_ref)?,
                generation_kind: parse_kind(&generation_kind)?,
            })
        }
    }
}

fn parse_profile_ref(value: &str) -> Result<GenerationProfileRef, DesktopErrorDto> {
    let (id, version) = value.split_once('@').ok_or_else(invalid_request)?;
    if version.starts_with('0') {
        return Err(invalid_request());
    }
    Ok(GenerationProfileRef::new(
        GenerationProfileId::try_new(id).map_err(|_| invalid_request())?,
        GenerationProfileVersion::try_new(version.parse().map_err(|_| invalid_request())?)
            .map_err(|_| invalid_request())?,
    ))
}

fn parse_revision(value: &str) -> Result<GenerationProviderSettingsRevision, DesktopErrorDto> {
    if value.starts_with('0') {
        return Err(invalid_request());
    }
    value.parse().ok().and_then(GenerationProviderSettingsRevision::new).ok_or_else(invalid_request)
}

fn parse_kind(value: &str) -> Result<GenerationTaskRequestKind, DesktopErrorDto> {
    match value {
        "text" => Ok(GenerationTaskRequestKind::Text),
        "image" => Ok(GenerationTaskRequestKind::Image),
        "video" => Ok(GenerationTaskRequestKind::Video),
        "voice" => Ok(GenerationTaskRequestKind::Voice),
        _ => Err(invalid_request()),
    }
}

fn settings_dto(view: GenerationProviderSettingsView) -> GenerationProviderSettingsDto {
    GenerationProviderSettingsDto {
        settings_revision: view.settings_revision.get().to_string(),
        profiles: view
            .profiles
            .into_iter()
            .map(|profile| GenerationProviderSettingsProfileDto {
                profile_ref: profile.profile_ref.to_string(),
                generation_kind: kind_name(profile.generation_kind).to_owned(),
                selected_binding: profile.selected_binding.map(|binding| {
                    GenerationProviderSettingsBindingDto {
                        provider_id: binding.provider_id().as_str().to_owned(),
                        route_id: binding.route_id().as_str().to_owned(),
                    }
                }),
                provider_choices: profile
                    .provider_choices
                    .into_iter()
                    .map(|provider| GenerationProviderSettingsProviderChoiceDto {
                        provider_id: provider.provider_id.as_str().to_owned(),
                        display_name: provider.display_name,
                        routes: provider
                            .routes
                            .into_iter()
                            .map(|route| GenerationProviderSettingsRouteChoiceDto {
                                route_id: route.route_id.as_str().to_owned(),
                                display_name: route.display_name,
                            })
                            .collect(),
                    })
                    .collect(),
            })
            .collect(),
    }
}

const fn kind_name(kind: GenerationTaskRequestKind) -> &'static str {
    match kind {
        GenerationTaskRequestKind::Text => "text",
        GenerationTaskRequestKind::Image => "image",
        GenerationTaskRequestKind::Video => "video",
        GenerationTaskRequestKind::Voice => "voice",
    }
}

fn settings_error(error: GenerationProviderSettingsError) -> DesktopErrorDto {
    let code = match error {
        GenerationProviderSettingsError::RevisionConflict => {
            DesktopErrorCode::GenerationProviderSettingsRevisionConflict
        }
        GenerationProviderSettingsError::InvalidMutation => {
            DesktopErrorCode::GenerationProviderSettingsInvalidRequest
        }
        _ => DesktopErrorCode::StorageUnavailable,
    };
    DesktopErrorDto::from_context(DesktopErrorContext {
        code,
        retryable: false,
        retry_after_epoch_ms: None,
        target: None,
        correlation_id: None,
    })
}

fn invalid_request() -> DesktopErrorDto {
    DesktopErrorDto::from_context(DesktopErrorContext {
        code: DesktopErrorCode::GenerationProviderSettingsInvalidRequest,
        retryable: false,
        retry_after_epoch_ms: None,
        target: None,
        correlation_id: None,
    })
}
