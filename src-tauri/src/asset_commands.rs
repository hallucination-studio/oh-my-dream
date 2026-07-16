//! Canonical Project-scoped Asset command entry points.

use assets::asset::{
    application::{
        AssetGetQuery, AssetImportCommand, AssetIssuePreviewCommand, AssetListCursor,
        AssetListQuery, AssetPageLimit,
    },
    domain::{AssetCreatedAt, AssetDisplayName, AssetId, AssetMediaKind},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::Deserialize;
use tauri::State;
use uuid::Uuid;

use crate::{
    asset_command_dto::{AssetDto, AssetListPageDto, AssetPreviewDto, asset_dto, page_dto},
    composition::DesktopActivatedCommandDependencies,
    desktop_backend_config::{
        DesktopErrorCode, DesktopErrorContext, DesktopErrorDto, DesktopErrorTarget,
    },
    workflow_commands::{invalid_request, trusted_project, uuid},
};

/// Trusted native-file import request. No path or media bytes enter JSON.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetImportRequestDto {
    pub project_id: String,
    pub expected_media_kind: String,
}

/// Exact Project-scoped Asset lookup.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetGetRequestDto {
    pub project_id: String,
    pub asset_id: String,
}

/// Stable bounded Project Asset page request.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetListRequestDto {
    pub project_id: String,
    pub media_kind: Option<String>,
    pub cursor: Option<String>,
    pub limit: u16,
}

/// Exact Project-scoped preview request.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetIssuePreviewRequestDto {
    pub project_id: String,
    pub asset_id: String,
}

/// Opens the native picker and imports one selected media file.
#[tauri::command(rename_all = "snake_case")]
pub async fn asset_import(
    request: AssetImportRequestDto,
    state: State<'_, DesktopActivatedCommandDependencies>,
) -> Result<Option<AssetDto>, DesktopErrorDto> {
    let project_id = trusted_project(&request.project_id, &state).await?;
    let expected_media_kind = parse_media_kind(&request.expected_media_kind)?;
    let selected = state
        .asset_import_source_picker
        .pick_asset_import_source(expected_media_kind)
        .await
        .map_err(|_| storage_error())?;
    let Some(selected) = selected else {
        return Ok(None);
    };
    let display_name = AssetDisplayName::try_new(selected.original_file_name.as_str())
        .map_err(|_| invalid_request())?;
    state
        .asset_import
        .import_asset(AssetImportCommand::new(
            project_id,
            expected_media_kind,
            display_name,
            selected.original_file_name,
            selected.source,
        ))
        .await
        .map(|asset| Some(asset_dto(&asset)))
        .map_err(|error| asset_error(error, None))
}

/// Returns one exact Project-visible Asset.
#[tauri::command(rename_all = "snake_case")]
pub async fn asset_get(
    request: AssetGetRequestDto,
    state: State<'_, DesktopActivatedCommandDependencies>,
) -> Result<AssetDto, DesktopErrorDto> {
    let project_id = trusted_project(&request.project_id, &state).await?;
    let asset_id = asset_id(&request.asset_id)?;
    state
        .asset_get
        .get_asset(AssetGetQuery::new(project_id, asset_id))
        .await
        .map(|asset| asset_dto(&asset))
        .map_err(|error| asset_error(error, Some(asset_id)))
}

/// Returns one stable bounded Project Asset page.
#[tauri::command(rename_all = "snake_case")]
pub async fn asset_list(
    request: AssetListRequestDto,
    state: State<'_, DesktopActivatedCommandDependencies>,
) -> Result<AssetListPageDto, DesktopErrorDto> {
    let project_id = trusted_project(&request.project_id, &state).await?;
    let media_kind = request.media_kind.as_deref().map(parse_media_kind).transpose()?;
    let cursor = request.cursor.as_deref().map(decode_cursor).transpose()?;
    let limit = AssetPageLimit::from_u16(request.limit).ok_or_else(invalid_request)?;
    state
        .asset_list
        .list_assets(AssetListQuery::new(project_id, media_kind, cursor, limit))
        .await
        .map(|page| page_dto(&page, encode_cursor))
        .map_err(|error| asset_error(error, None))
}

/// Issues one signed five-minute preview URI.
#[tauri::command(rename_all = "snake_case")]
pub async fn asset_issue_preview(
    request: AssetIssuePreviewRequestDto,
    state: State<'_, DesktopActivatedCommandDependencies>,
) -> Result<AssetPreviewDto, DesktopErrorDto> {
    let project_id = trusted_project(&request.project_id, &state).await?;
    let asset_id = asset_id(&request.asset_id)?;
    let lease = state
        .asset_issue_preview
        .issue_asset_preview(AssetIssuePreviewCommand::new(project_id, asset_id))
        .await
        .map_err(|error| asset_error(error, Some(asset_id)))?;
    Ok(AssetPreviewDto {
        asset_id: asset_id.as_uuid().hyphenated().to_string(),
        preview_uri: state.asset_preview_protocol.issue_preview_uri(&lease),
        expires_at_epoch_ms: lease.expires_at_utc_milliseconds().to_string(),
    })
}

fn parse_media_kind(value: &str) -> Result<AssetMediaKind, DesktopErrorDto> {
    match value {
        "image" => Ok(AssetMediaKind::Image),
        "video" => Ok(AssetMediaKind::Video),
        "audio" => Ok(AssetMediaKind::Audio),
        _ => Err(invalid_request()),
    }
}

fn asset_id(value: &str) -> Result<AssetId, DesktopErrorDto> {
    AssetId::from_uuid(uuid(value)?).map_err(|_| invalid_request())
}

fn encode_cursor(cursor: AssetListCursor) -> String {
    let mut bytes = [0_u8; 24];
    bytes[..8].copy_from_slice(&cursor.created_at().as_utc_milliseconds().to_be_bytes());
    bytes[8..].copy_from_slice(cursor.asset_id().as_uuid().as_bytes());
    URL_SAFE_NO_PAD.encode(bytes)
}

fn decode_cursor(value: &str) -> Result<AssetListCursor, DesktopErrorDto> {
    let bytes = URL_SAFE_NO_PAD.decode(value).map_err(|_| invalid_request())?;
    let bytes: [u8; 24] = bytes.try_into().map_err(|_| invalid_request())?;
    let created_at = AssetCreatedAt::from_utc_milliseconds(i64::from_be_bytes(
        bytes[..8].try_into().map_err(|_| invalid_request())?,
    ))
    .map_err(|_| invalid_request())?;
    let asset_id =
        AssetId::from_uuid(Uuid::from_bytes(bytes[8..].try_into().map_err(|_| invalid_request())?))
            .map_err(|_| invalid_request())?;
    let cursor = AssetListCursor::new(created_at, asset_id);
    (encode_cursor(cursor) == value).then_some(cursor).ok_or_else(invalid_request)
}

fn asset_error(
    error: assets::asset::application::AssetApplicationError,
    asset_id: Option<AssetId>,
) -> DesktopErrorDto {
    use assets::asset::application::AssetApplicationError;
    let code = match error {
        AssetApplicationError::NotFound => DesktopErrorCode::AssetNotFound,
        AssetApplicationError::NotVisible => DesktopErrorCode::AssetNotVisible,
        AssetApplicationError::ContentPending => DesktopErrorCode::AssetContentPending,
        AssetApplicationError::ContentMissing => DesktopErrorCode::AssetContentMissing,
        AssetApplicationError::InvalidMedia
        | AssetApplicationError::MediaSizeLimitExceeded
        | AssetApplicationError::MediaKindMismatch { .. } => DesktopErrorCode::AssetInvalidMedia,
        AssetApplicationError::Cancelled | AssetApplicationError::DeadlineExceeded => {
            DesktopErrorCode::AssetInvalidRequest
        }
        _ => DesktopErrorCode::StorageUnavailable,
    };
    error_dto(code, asset_id)
}

fn storage_error() -> DesktopErrorDto {
    error_dto(DesktopErrorCode::StorageUnavailable, None)
}

fn error_dto(code: DesktopErrorCode, asset_id: Option<AssetId>) -> DesktopErrorDto {
    DesktopErrorDto::from_context(DesktopErrorContext {
        code,
        retryable: false,
        retry_after_epoch_ms: None,
        target: asset_id.map(|value| DesktopErrorTarget::Asset {
            asset_id: value.as_uuid().hyphenated().to_string(),
        }),
        correlation_id: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_cursor_is_canonical_opaque_base64url() {
        let cursor = AssetListCursor::new(
            AssetCreatedAt::from_utc_milliseconds(42).unwrap(),
            AssetId::from_uuid(Uuid::parse_str("10000000-0000-4000-8000-000000000001").unwrap())
                .unwrap(),
        );
        let encoded = encode_cursor(cursor);
        assert_eq!(decode_cursor(&encoded), Ok(cursor));
        assert!(decode_cursor(&format!("{encoded}=")).is_err());
    }
}
