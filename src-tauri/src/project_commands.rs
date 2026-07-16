//! Frozen Project command DTOs, translation, and Tauri entry points.

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use projects::project::{
    application::{
        ProjectApplicationError, ProjectCreateRequest, ProjectListCursor, ProjectListLimit,
        ProjectListQuery, ProjectMutationRequestId, ProjectRenameRequest,
        ProjectWorkflowReadinessSummary, ProjectWorkspaceView,
    },
    domain::{ProjectAggregate, ProjectId, ProjectName, ProjectRevision, ProjectUpdatedAt},
};
use serde::{Deserialize, Serialize};
use tauri::State;
use uuid::Uuid;

use crate::{
    composition::DesktopActivatedCommandDependencies,
    desktop_backend_config::{DesktopErrorContext, DesktopErrorDto, DesktopErrorTarget},
};

/// Project creation transport request.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectCreateRequestDto {
    /// Stable UUIDv4 idempotency key.
    pub request_id: String,
    /// User-visible Project name.
    pub name: String,
}

/// Project rename transport request.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectRenameRequestDto {
    /// Stable UUIDv4 idempotency key.
    pub request_id: String,
    /// Project identity.
    pub project_id: String,
    /// Non-zero decimal current revision.
    pub expected_revision: String,
    /// Replacement Project name.
    pub name: String,
}

/// Exact Project read request.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectGetRequestDto {
    /// Project identity.
    pub project_id: String,
}

/// Stable Project list-page request.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectListRequestDto {
    /// Page size in `1..=100`.
    pub limit: u16,
    /// Opaque exclusive keyset cursor.
    pub cursor: Option<String>,
}

/// Project open request.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectOpenRequestDto {
    /// Project identity.
    pub project_id: String,
}

/// Canonical Project transport projection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ProjectDto {
    /// Lowercase hyphenated UUIDv4.
    pub id: String,
    /// Normalized user-visible name.
    pub name: String,
    /// Non-zero decimal revision.
    pub revision: String,
    /// Non-negative UTC milliseconds as decimal text.
    pub created_at_epoch_ms: String,
    /// Non-negative UTC milliseconds as decimal text.
    pub updated_at_epoch_ms: String,
}

/// One stable Project list page.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ProjectListPageDto {
    /// Projects in authoritative descending keyset order.
    pub projects: Vec<ProjectDto>,
    /// Opaque cursor only when another page exists.
    pub next_cursor: Option<String>,
}

/// Minimal current Workflow projection returned by Project open.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ProjectWorkflowSummaryDto {
    /// Opaque canonical Workflow identity.
    pub workflow_id: String,
    /// Non-zero decimal Workflow revision.
    pub workflow_revision: String,
    /// Same-snapshot readiness summary.
    pub readiness: ProjectWorkflowReadinessDto,
}

/// Closed Project-owned Workflow readiness summary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectWorkflowReadinessDto {
    /// No authoritative readiness issues exist.
    Ready,
    /// At least one authoritative readiness issue exists.
    Blocked,
}

/// Project plus optional same-snapshot current Workflow summary.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ProjectWorkspaceDto {
    /// Opened authoritative Project.
    pub project: ProjectDto,
    /// Minimal current Workflow summary, or `null` when none exists.
    pub current_workflow_summary: Option<ProjectWorkflowSummaryDto>,
}

/// Creates or idempotently replays one Project.
#[tauri::command(rename_all = "snake_case")]
pub async fn project_create(
    request: ProjectCreateRequestDto,
    state: State<'_, DesktopActivatedCommandDependencies>,
) -> Result<ProjectDto, DesktopErrorDto> {
    project_create_with_dependencies(request, &state).await
}

pub(crate) async fn project_create_with_dependencies(
    request: ProjectCreateRequestDto,
    state: &DesktopActivatedCommandDependencies,
) -> Result<ProjectDto, DesktopErrorDto> {
    let request = ProjectCreateRequest {
        request_id: mutation_request_id(&request.request_id)?,
        name: ProjectName::new(request.name).map_err(project_error)?,
    };
    state.create.create_project(request).await.map(project_dto).map_err(project_error)
}

/// Renames one exact Project revision.
#[tauri::command(rename_all = "snake_case")]
pub async fn project_rename(
    request: ProjectRenameRequestDto,
    state: State<'_, DesktopActivatedCommandDependencies>,
) -> Result<ProjectDto, DesktopErrorDto> {
    project_rename_with_dependencies(request, &state).await
}

pub(crate) async fn project_rename_with_dependencies(
    request: ProjectRenameRequestDto,
    state: &DesktopActivatedCommandDependencies,
) -> Result<ProjectDto, DesktopErrorDto> {
    let request = ProjectRenameRequest {
        request_id: mutation_request_id(&request.request_id)?,
        project_id: project_id(&request.project_id)?,
        expected_revision: revision(&request.expected_revision)?,
        name: ProjectName::new(request.name).map_err(project_error)?,
    };
    state.rename.rename_project(request).await.map(project_dto).map_err(project_error)
}

/// Returns one exact Project.
#[tauri::command(rename_all = "snake_case")]
pub async fn project_get(
    request: ProjectGetRequestDto,
    state: State<'_, DesktopActivatedCommandDependencies>,
) -> Result<ProjectDto, DesktopErrorDto> {
    project_get_with_dependencies(request, &state).await
}

pub(crate) async fn project_get_with_dependencies(
    request: ProjectGetRequestDto,
    state: &DesktopActivatedCommandDependencies,
) -> Result<ProjectDto, DesktopErrorDto> {
    state
        .get
        .get_project(project_id(&request.project_id)?)
        .await
        .map(project_dto)
        .map_err(project_error)
}

/// Returns one bounded stable Project page.
#[tauri::command(rename_all = "snake_case")]
pub async fn project_list(
    request: ProjectListRequestDto,
    state: State<'_, DesktopActivatedCommandDependencies>,
) -> Result<ProjectListPageDto, DesktopErrorDto> {
    project_list_with_dependencies(request, &state).await
}

pub(crate) async fn project_list_with_dependencies(
    request: ProjectListRequestDto,
    state: &DesktopActivatedCommandDependencies,
) -> Result<ProjectListPageDto, DesktopErrorDto> {
    let page = state
        .list
        .list_projects(ProjectListQuery {
            limit: ProjectListLimit::new(request.limit).map_err(project_error)?,
            cursor: request.cursor.as_deref().map(decode_cursor).transpose()?,
        })
        .await
        .map_err(project_error)?;
    Ok(ProjectListPageDto {
        projects: page.projects.into_iter().map(project_dto).collect(),
        next_cursor: page.next_cursor.map(encode_cursor),
    })
}

/// Opens one Project and its optional same-snapshot Workflow summary.
#[tauri::command(rename_all = "snake_case")]
pub async fn project_open(
    request: ProjectOpenRequestDto,
    state: State<'_, DesktopActivatedCommandDependencies>,
) -> Result<ProjectWorkspaceDto, DesktopErrorDto> {
    project_open_with_dependencies(request, &state).await
}

pub(crate) async fn project_open_with_dependencies(
    request: ProjectOpenRequestDto,
    state: &DesktopActivatedCommandDependencies,
) -> Result<ProjectWorkspaceDto, DesktopErrorDto> {
    state
        .open
        .open_project(project_id(&request.project_id)?)
        .await
        .map(workspace_dto)
        .map_err(project_error)
}

fn project_dto(project: ProjectAggregate) -> ProjectDto {
    ProjectDto {
        id: project.id().as_uuid().hyphenated().to_string(),
        name: project.name().as_str().to_owned(),
        revision: project.revision().get().to_string(),
        created_at_epoch_ms: project.created_at().get().to_string(),
        updated_at_epoch_ms: project.updated_at().get().to_string(),
    }
}

fn workspace_dto(view: ProjectWorkspaceView) -> ProjectWorkspaceDto {
    ProjectWorkspaceDto {
        project: project_dto(view.project),
        current_workflow_summary: view.current_workflow_summary.map(|summary| {
            ProjectWorkflowSummaryDto {
                workflow_id: summary.workflow_id.as_str().to_owned(),
                workflow_revision: summary.workflow_revision.get().to_string(),
                readiness: match summary.readiness {
                    ProjectWorkflowReadinessSummary::Ready => ProjectWorkflowReadinessDto::Ready,
                    ProjectWorkflowReadinessSummary::Blocked => {
                        ProjectWorkflowReadinessDto::Blocked
                    }
                },
            }
        }),
    }
}

fn mutation_request_id(value: &str) -> Result<ProjectMutationRequestId, DesktopErrorDto> {
    ProjectMutationRequestId::from_uuid(uuid(value)?).ok_or_else(invalid_request)
}

fn project_id(value: &str) -> Result<ProjectId, DesktopErrorDto> {
    ProjectId::from_uuid(uuid(value)?).ok_or_else(invalid_request)
}

fn uuid(value: &str) -> Result<Uuid, DesktopErrorDto> {
    let parsed = Uuid::parse_str(value).map_err(|_| invalid_request())?;
    if parsed.hyphenated().to_string() != value {
        return Err(invalid_request());
    }
    Ok(parsed)
}

fn revision(value: &str) -> Result<ProjectRevision, DesktopErrorDto> {
    let parsed = value.parse::<u64>().map_err(|_| invalid_request())?;
    if parsed.to_string() != value {
        return Err(invalid_request());
    }
    ProjectRevision::from_non_zero(parsed).ok_or_else(invalid_request)
}

fn encode_cursor(cursor: ProjectListCursor) -> String {
    let mut bytes = Vec::with_capacity(24);
    bytes.extend_from_slice(&cursor.updated_at.get().to_be_bytes());
    bytes.extend_from_slice(cursor.project_id.as_uuid().as_bytes());
    URL_SAFE_NO_PAD.encode(bytes)
}

fn decode_cursor(value: &str) -> Result<ProjectListCursor, DesktopErrorDto> {
    let bytes = URL_SAFE_NO_PAD.decode(value).map_err(|_| invalid_request())?;
    let bytes: [u8; 24] = bytes.try_into().map_err(|_| invalid_request())?;
    if URL_SAFE_NO_PAD.encode(bytes) != value {
        return Err(invalid_request());
    }
    let updated_at = i64::from_be_bytes(bytes[..8].try_into().map_err(|_| invalid_request())?);
    let project_uuid = Uuid::from_bytes(bytes[8..].try_into().map_err(|_| invalid_request())?);
    Ok(ProjectListCursor {
        updated_at: ProjectUpdatedAt::new(updated_at).map_err(|_| invalid_request())?,
        project_id: ProjectId::from_uuid(project_uuid).ok_or_else(invalid_request)?,
    })
}

fn project_error(error: impl Into<ProjectApplicationError>) -> DesktopErrorDto {
    let error = error.into();
    use crate::desktop_backend_config::DesktopErrorCode;
    let (code, target) = match error {
        ProjectApplicationError::ProjectNotFound { project_id } => (
            DesktopErrorCode::ProjectNotFound,
            Some(DesktopErrorTarget::Project {
                project_id: project_id.as_uuid().hyphenated().to_string(),
            }),
        ),
        ProjectApplicationError::ProjectRevisionConflict { project_id, .. } => (
            DesktopErrorCode::ProjectRevisionConflict,
            Some(DesktopErrorTarget::Project {
                project_id: project_id.as_uuid().hyphenated().to_string(),
            }),
        ),
        ProjectApplicationError::ProjectMutationIdempotencyConflict { .. } => {
            (DesktopErrorCode::ProjectMutationConflict, None)
        }
        ProjectApplicationError::ProjectDomain(_)
        | ProjectApplicationError::ProjectListLimitOutOfBounds { .. } => {
            (DesktopErrorCode::ProjectInvalidRequest, None)
        }
        ProjectApplicationError::ProjectWorkflowSummaryReadFailure
        | ProjectApplicationError::ProjectPersistenceFailure => {
            (DesktopErrorCode::StorageUnavailable, None)
        }
    };
    DesktopErrorDto::from_context(DesktopErrorContext {
        code,
        retryable: false,
        retry_after_epoch_ms: None,
        target,
        correlation_id: None,
    })
}

fn invalid_request() -> DesktopErrorDto {
    use crate::desktop_backend_config::DesktopErrorCode;
    DesktopErrorDto::from_context(DesktopErrorContext {
        code: DesktopErrorCode::ProjectInvalidRequest,
        retryable: false,
        retry_after_epoch_ms: None,
        target: None,
        correlation_id: None,
    })
}

#[cfg(test)]
mod tests;
