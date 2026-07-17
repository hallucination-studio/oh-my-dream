//! Project-scoped Generation Task get and list command entry points.

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use tasks::generation_task::{
    GenerationTaskApplicationError, GenerationTaskId, GenerationTaskListCursor,
    GenerationTaskListQuery, GenerationTaskRequestKind, GenerationTaskStatus,
    GenerationTaskTimestamp,
};
use tauri::State;
use uuid::Uuid;

use crate::{
    composition::DesktopActivatedCommandDependencies,
    desktop_backend_config::{DesktopErrorCode, DesktopErrorContext, DesktopErrorDto},
    generation_task_command_dto::{
        GenerationTaskDto, GenerationTaskGetRequestDto, GenerationTaskListPageDto,
        GenerationTaskListRequestDto, summary_dto, task_dto,
    },
    workflow_commands::{trusted_project, uuid},
};

/// Returns one exact Project-scoped Generation Task.
#[tauri::command(rename_all = "snake_case")]
pub async fn generation_task_get(
    request: GenerationTaskGetRequestDto,
    state: State<'_, DesktopActivatedCommandDependencies>,
) -> Result<GenerationTaskDto, DesktopErrorDto> {
    generation_task_get_with_dependencies(request, &state).await
}

/// Returns one Task through explicit already-composed dependencies.
pub(crate) async fn generation_task_get_with_dependencies(
    request: GenerationTaskGetRequestDto,
    state: &DesktopActivatedCommandDependencies,
) -> Result<GenerationTaskDto, DesktopErrorDto> {
    let project_id = trusted_project(&request.project_id, state).await?;
    let task_id = task_id(&request.generation_task_id)?;
    state
        .generation_task_get
        .get_generation_task(project_id, task_id)
        .await
        .map(|task| task_dto(&task, state.generation_task_provider_contracts.as_ref()))
        .map_err(task_error)
}

/// Returns one stable bounded Project-scoped Generation Task page.
#[tauri::command(rename_all = "snake_case")]
pub async fn generation_task_list(
    request: GenerationTaskListRequestDto,
    state: State<'_, DesktopActivatedCommandDependencies>,
) -> Result<GenerationTaskListPageDto, DesktopErrorDto> {
    generation_task_list_with_dependencies(request, &state).await
}

/// Returns a Task page through explicit already-composed dependencies.
pub(crate) async fn generation_task_list_with_dependencies(
    request: GenerationTaskListRequestDto,
    state: &DesktopActivatedCommandDependencies,
) -> Result<GenerationTaskListPageDto, DesktopErrorDto> {
    let project_id = trusted_project(&request.project_id, state).await?;
    let query = GenerationTaskListQuery::try_new(
        project_id,
        request.status.as_deref().map(parse_status).transpose()?,
        request.request_kind.as_deref().map(parse_request_kind).transpose()?,
        request.cursor.as_deref().map(decode_cursor).transpose()?,
        u8::try_from(request.limit).map_err(|_| invalid_request())?,
    )
    .map_err(task_error)?;
    state
        .generation_task_list
        .list_generation_tasks(query)
        .await
        .map(|page| GenerationTaskListPageDto {
            tasks: page
                .items
                .iter()
                .map(|task| summary_dto(task, state.generation_task_provider_contracts.as_ref()))
                .collect(),
            next_cursor: page.next_cursor.map(encode_cursor),
        })
        .map_err(task_error)
}

fn task_id(value: &str) -> Result<GenerationTaskId, DesktopErrorDto> {
    GenerationTaskId::from_uuid(uuid(value)?).map_err(|_| invalid_request())
}

fn parse_status(value: &str) -> Result<GenerationTaskStatus, DesktopErrorDto> {
    match value {
        "queued" => Ok(GenerationTaskStatus::Queued),
        "running" => Ok(GenerationTaskStatus::Running),
        "cancel_requested" => Ok(GenerationTaskStatus::CancelRequested),
        "succeeded" => Ok(GenerationTaskStatus::Succeeded),
        "failed" => Ok(GenerationTaskStatus::Failed),
        "cancelled" => Ok(GenerationTaskStatus::Cancelled),
        _ => Err(invalid_request()),
    }
}

fn parse_request_kind(value: &str) -> Result<GenerationTaskRequestKind, DesktopErrorDto> {
    match value {
        "text" => Ok(GenerationTaskRequestKind::Text),
        "image" => Ok(GenerationTaskRequestKind::Image),
        "video" => Ok(GenerationTaskRequestKind::Video),
        "voice" => Ok(GenerationTaskRequestKind::Voice),
        _ => Err(invalid_request()),
    }
}

fn encode_cursor(cursor: GenerationTaskListCursor) -> String {
    let mut bytes = [0_u8; 24];
    bytes[..8].copy_from_slice(&cursor.created_at.as_utc_milliseconds().to_be_bytes());
    bytes[8..].copy_from_slice(cursor.task_id.as_uuid().as_bytes());
    URL_SAFE_NO_PAD.encode(bytes)
}

fn decode_cursor(value: &str) -> Result<GenerationTaskListCursor, DesktopErrorDto> {
    let bytes = URL_SAFE_NO_PAD.decode(value).map_err(|_| invalid_request())?;
    let bytes: [u8; 24] = bytes.try_into().map_err(|_| invalid_request())?;
    let created_at = GenerationTaskTimestamp::from_utc_milliseconds(i64::from_be_bytes(
        bytes[..8].try_into().map_err(|_| invalid_request())?,
    ))
    .map_err(|_| invalid_request())?;
    let task_id = GenerationTaskId::from_uuid(Uuid::from_bytes(
        bytes[8..].try_into().map_err(|_| invalid_request())?,
    ))
    .map_err(|_| invalid_request())?;
    let cursor = GenerationTaskListCursor { created_at, task_id };
    (encode_cursor(cursor) == value).then_some(cursor).ok_or_else(invalid_request)
}

fn task_error(error: GenerationTaskApplicationError) -> DesktopErrorDto {
    let code = match error {
        GenerationTaskApplicationError::InvalidArgument
        | GenerationTaskApplicationError::Domain(_) => {
            DesktopErrorCode::GenerationTaskInvalidRequest
        }
        GenerationTaskApplicationError::TaskNotFound => DesktopErrorCode::GenerationTaskNotFound,
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
        code: DesktopErrorCode::GenerationTaskInvalidRequest,
        retryable: false,
        retry_after_epoch_ms: None,
        target: None,
        correlation_id: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_only_closed_task_filters() {
        assert_eq!(parse_status("running").unwrap(), GenerationTaskStatus::Running);
        assert_eq!(parse_request_kind("image").unwrap(), GenerationTaskRequestKind::Image);
        assert!(parse_status("submitted").is_err());
        assert!(parse_request_kind("audio").is_err());
    }

    #[test]
    fn task_cursor_is_canonical_and_stable() {
        let cursor = GenerationTaskListCursor {
            created_at: GenerationTaskTimestamp::from_utc_milliseconds(42).unwrap(),
            task_id: GenerationTaskId::from_uuid(
                Uuid::parse_str("123e4567-e89b-42d3-a456-426600000030").unwrap(),
            )
            .unwrap(),
        };
        let encoded = encode_cursor(cursor);
        assert_eq!(decode_cursor(&encoded).unwrap(), cursor);
        assert!(decode_cursor("AQID").is_err());
        assert!(decode_cursor(&(encoded + "=")).is_err());
    }

    #[test]
    fn task_errors_use_closed_safe_codes() {
        assert_eq!(
            task_error(GenerationTaskApplicationError::TaskNotFound).code,
            "generation_task.not_found"
        );
        assert_eq!(
            task_error(GenerationTaskApplicationError::InvalidArgument).code,
            "generation_task.invalid_request"
        );
    }
}
