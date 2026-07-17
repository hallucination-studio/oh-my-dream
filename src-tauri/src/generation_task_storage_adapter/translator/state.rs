use assets::asset::domain::{AssetId, AssetMediaKind};
use tasks::generation_task::domain::*;

use super::{TaskRow, timestamp, uuid};

pub(super) struct EncodedState {
    pub status: &'static str,
    pub progress_percent: Option<i64>,
    pub remote_task_id: Option<String>,
    pub result_kind: Option<&'static str>,
    pub result_text: Option<String>,
    pub result_asset_id: Option<Vec<u8>>,
    pub result_media_kind: Option<&'static str>,
    pub failure_kind: Option<&'static str>,
    pub failure_code: Option<String>,
    pub failure_message: Option<String>,
    pub completed_at: Option<i64>,
}

pub(super) fn encode(
    state: &GenerationTaskState,
    result: Option<&GenerationTaskResult>,
) -> EncodedState {
    let state = encode_state(state);
    let result = encode_result(result);
    EncodedState {
        status: state.status,
        progress_percent: state.progress_percent,
        remote_task_id: state.remote_task_id,
        result_kind: result.kind,
        result_text: result.text,
        result_asset_id: result.asset_id,
        result_media_kind: result.media_kind,
        failure_kind: state.failure_kind,
        failure_code: state.failure_code,
        failure_message: state.failure_message,
        completed_at: state.completed_at,
    }
}

pub(super) fn decode_state(row: &TaskRow, has_result: bool) -> Result<GenerationTaskState, ()> {
    let completed = row.completed_at.map(timestamp).transpose()?;
    let handle = row
        .remote_task_id
        .as_ref()
        .map(|value| GenerationProviderTaskHandle::try_new(value.clone()).map_err(|_| ()))
        .transpose()?;
    let progress =
        row.progress_percent.map(|value| u8::try_from(value).map_err(|_| ())).transpose()?;
    match row.status.as_str() {
        "Queued" if empty_active(row, has_result) => Ok(GenerationTaskState::Queued),
        "Submitting" if empty_active(row, has_result) => Ok(GenerationTaskState::Submitting),
        "Running" if completed.is_none() && !has_result && row.failure_kind.is_none() => {
            Ok(GenerationTaskState::Running {
                handle: handle.ok_or(())?,
                progress_percent: progress,
            })
        }
        "CancelRequested"
            if completed.is_none()
                && !has_result
                && row.failure_kind.is_none()
                && progress.is_none() =>
        {
            Ok(GenerationTaskState::CancelRequested { handle })
        }
        "Succeeded"
            if completed.is_some()
                && has_result
                && no_active(row)
                && row.failure_kind.is_none() =>
        {
            Ok(GenerationTaskState::Succeeded { completed_at: completed.ok_or(())? })
        }
        "Failed" if completed.is_some() && !has_result && no_active(row) => {
            Ok(GenerationTaskState::Failed {
                completed_at: completed.ok_or(())?,
                failure: GenerationTaskFailure::try_new(
                    parse_failure_kind(row.failure_kind.as_deref().ok_or(())?)?,
                    row.failure_code.clone().ok_or(())?,
                    row.failure_message.clone().ok_or(())?,
                )
                .map_err(|_| ())?,
            })
        }
        "Cancelled"
            if completed.is_some()
                && !has_result
                && no_active(row)
                && row.failure_kind.is_none() =>
        {
            Ok(GenerationTaskState::Cancelled { completed_at: completed.ok_or(())? })
        }
        _ => Err(()),
    }
}

pub(super) fn decode_result(
    kind: Option<&str>,
    text_value: Option<String>,
    asset_id: Option<Vec<u8>>,
    media: Option<&str>,
) -> Result<Option<GenerationTaskResult>, ()> {
    match (kind, text_value, asset_id, media) {
        (None, None, None, None) => Ok(None),
        (Some("Text"), Some(value), None, None) => Ok(Some(GenerationTaskResult::Text {
            content: GenerationTaskText::try_new(value).map_err(|_| ())?,
        })),
        (Some("Asset"), None, Some(id), Some(kind)) => {
            Ok(Some(GenerationTaskResult::Asset(GenerationTaskAssetResult::new(
                AssetId::from_uuid(uuid(&id)?).map_err(|_| ())?,
                parse_media_kind(kind)?,
            ))))
        }
        _ => Err(()),
    }
}

struct StateFields {
    status: &'static str,
    progress_percent: Option<i64>,
    remote_task_id: Option<String>,
    failure_kind: Option<&'static str>,
    failure_code: Option<String>,
    failure_message: Option<String>,
    completed_at: Option<i64>,
}

fn encode_state(state: &GenerationTaskState) -> StateFields {
    let completed_at = completed_at(state);
    let fields = match state {
        GenerationTaskState::Queued => ("Queued", None, None, None, None, None),
        GenerationTaskState::Submitting => ("Submitting", None, None, None, None, None),
        GenerationTaskState::Running { handle, progress_percent } => (
            "Running",
            progress_percent.map(i64::from),
            Some(handle.as_str().into()),
            None,
            None,
            None,
        ),
        GenerationTaskState::CancelRequested { handle } => (
            "CancelRequested",
            None,
            handle.as_ref().map(|value| value.as_str().into()),
            None,
            None,
            None,
        ),
        GenerationTaskState::Succeeded { .. } => ("Succeeded", None, None, None, None, None),
        GenerationTaskState::Failed { failure, .. } => (
            "Failed",
            None,
            None,
            Some(failure_kind(failure.kind())),
            Some(failure.code().into()),
            Some(failure.message().into()),
        ),
        GenerationTaskState::Cancelled { .. } => ("Cancelled", None, None, None, None, None),
    };
    StateFields {
        status: fields.0,
        progress_percent: fields.1,
        remote_task_id: fields.2,
        failure_kind: fields.3,
        failure_code: fields.4,
        failure_message: fields.5,
        completed_at,
    }
}

struct ResultFields {
    kind: Option<&'static str>,
    text: Option<String>,
    asset_id: Option<Vec<u8>>,
    media_kind: Option<&'static str>,
}

fn encode_result(result: Option<&GenerationTaskResult>) -> ResultFields {
    let fields = match result {
        None => (None, None, None, None),
        Some(GenerationTaskResult::Text { content }) => {
            (Some("Text"), Some(content.as_str().into()), None, None)
        }
        Some(GenerationTaskResult::Asset(asset)) => (
            Some("Asset"),
            None,
            Some(asset.asset_id().as_uuid().as_bytes().to_vec()),
            Some(media_kind(asset.media_kind())),
        ),
    };
    ResultFields { kind: fields.0, text: fields.1, asset_id: fields.2, media_kind: fields.3 }
}

fn completed_at(state: &GenerationTaskState) -> Option<i64> {
    match state {
        GenerationTaskState::Succeeded { completed_at }
        | GenerationTaskState::Failed { completed_at, .. }
        | GenerationTaskState::Cancelled { completed_at } => {
            Some(completed_at.as_utc_milliseconds())
        }
        _ => None,
    }
}

fn empty_active(row: &TaskRow, has_result: bool) -> bool {
    row.completed_at.is_none() && !has_result && no_active(row) && row.failure_kind.is_none()
}

fn no_active(row: &TaskRow) -> bool {
    row.progress_percent.is_none() && row.remote_task_id.is_none()
}

fn media_kind(value: AssetMediaKind) -> &'static str {
    match value {
        AssetMediaKind::Image => "Image",
        AssetMediaKind::Video => "Video",
        AssetMediaKind::Audio => "Audio",
    }
}

fn parse_media_kind(value: &str) -> Result<AssetMediaKind, ()> {
    match value {
        "Image" => Ok(AssetMediaKind::Image),
        "Video" => Ok(AssetMediaKind::Video),
        "Audio" => Ok(AssetMediaKind::Audio),
        _ => Err(()),
    }
}

fn failure_kind(value: GenerationTaskFailureKind) -> &'static str {
    match value {
        GenerationTaskFailureKind::InvalidRequest => "InvalidRequest",
        GenerationTaskFailureKind::Authentication => "Authentication",
        GenerationTaskFailureKind::PermissionDenied => "PermissionDenied",
        GenerationTaskFailureKind::ContentPolicy => "ContentPolicy",
        GenerationTaskFailureKind::RateLimited => "RateLimited",
        GenerationTaskFailureKind::ProviderUnavailable => "ProviderUnavailable",
        GenerationTaskFailureKind::Timeout => "Timeout",
        GenerationTaskFailureKind::ProviderRejected => "ProviderRejected",
        GenerationTaskFailureKind::InvalidProviderResponse => "InvalidProviderResponse",
        GenerationTaskFailureKind::AmbiguousSubmission => "AmbiguousSubmission",
        GenerationTaskFailureKind::InputAssetUnavailable => "InputAssetUnavailable",
        GenerationTaskFailureKind::OutputAssetImport => "OutputAssetImport",
        GenerationTaskFailureKind::Internal => "Internal",
    }
}

fn parse_failure_kind(value: &str) -> Result<GenerationTaskFailureKind, ()> {
    Ok(match value {
        "InvalidRequest" => GenerationTaskFailureKind::InvalidRequest,
        "Authentication" => GenerationTaskFailureKind::Authentication,
        "PermissionDenied" => GenerationTaskFailureKind::PermissionDenied,
        "ContentPolicy" => GenerationTaskFailureKind::ContentPolicy,
        "RateLimited" => GenerationTaskFailureKind::RateLimited,
        "ProviderUnavailable" => GenerationTaskFailureKind::ProviderUnavailable,
        "Timeout" => GenerationTaskFailureKind::Timeout,
        "ProviderRejected" => GenerationTaskFailureKind::ProviderRejected,
        "InvalidProviderResponse" => GenerationTaskFailureKind::InvalidProviderResponse,
        "AmbiguousSubmission" => GenerationTaskFailureKind::AmbiguousSubmission,
        "InputAssetUnavailable" => GenerationTaskFailureKind::InputAssetUnavailable,
        "OutputAssetImport" => GenerationTaskFailureKind::OutputAssetImport,
        "Internal" => GenerationTaskFailureKind::Internal,
        _ => return Err(()),
    })
}
