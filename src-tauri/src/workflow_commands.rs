//! Canonical Workflow read, creation, readiness, and Run command entry points.

use engine::{
    node_capability::WorkflowRunId,
    workflow::{
        WorkflowApplicationError, WorkflowCreateCommand, WorkflowCreateRequestId,
        WorkflowRunEventSequence, WorkflowRunRequestId, WorkflowRunScope, WorkflowStartRunCommand,
    },
    workflow_graph::{WorkflowId, WorkflowNodeId, WorkflowRevision},
};
use projects::project::{application::ProjectApplicationError, domain::ProjectId};
use serde::Deserialize;
use tauri::State;
use uuid::Uuid;

use crate::{
    composition::DesktopActivatedCommandDependencies,
    desktop_backend_config::{
        DesktopErrorCode, DesktopErrorContext, DesktopErrorDto, DesktopErrorTarget,
    },
    workflow_command_dto::{
        WorkflowDto, WorkflowRunDto, WorkflowRunEventPageDto, event_dto, run_dto, workflow_dto,
    },
    workflow_readiness_dto::{WorkflowReadinessDto, WorkflowWithReadinessDto, readiness_dto},
};

/// Idempotent current Workflow creation request.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowCreateRequestDto {
    /// Stable UUIDv4 request identity.
    pub request_id: String,
    /// Owning Project identity.
    pub project_id: String,
}

/// Current Workflow query request.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowGetCurrentRequestDto {
    /// Owning Project identity.
    pub project_id: String,
}

/// Workflow readiness query request.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowCheckReadinessRequestDto {
    /// Owning Project identity.
    pub project_id: String,
    /// Workflow identity.
    pub workflow_id: String,
}

/// Durable Run admission request.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowStartRunRequestDto {
    /// Stable UUIDv4 request identity.
    pub request_id: String,
    /// Owning Project identity.
    pub project_id: String,
    /// Workflow identity.
    pub workflow_id: String,
    /// Exact source revision as decimal text.
    pub workflow_revision: String,
    /// Closed execution scope.
    pub scope: WorkflowRunScopeDto,
}

/// Whole-graph or selected-node execution scope.
#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkflowRunScopeDto {
    /// Execute the entire Workflow.
    WholeWorkflow,
    /// Execute one node and every transitive ancestor.
    ThroughNode {
        /// Selected terminal node.
        node_id: String,
    },
}

/// Project-scoped Run request.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowRunRequestDto {
    /// Owning Project identity.
    pub project_id: String,
    /// Run identity.
    pub workflow_run_id: String,
}

/// Bounded durable Run event page request.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowListRunEventsRequestDto {
    /// Owning Project identity.
    pub project_id: String,
    /// Run identity.
    pub workflow_run_id: String,
    /// Exclusive event sequence as decimal text.
    pub after_sequence: Option<String>,
    /// Page size in `1..=500`.
    pub limit: u16,
}

/// Current node presentation request.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowGetNodePresentationRequestDto {
    /// Owning Project identity.
    pub project_id: String,
    /// Current Workflow identity.
    pub workflow_id: String,
    /// Current node identity.
    pub node_id: String,
}

/// Creates or exactly replays one Project's current Workflow.
#[tauri::command(rename_all = "snake_case")]
pub async fn workflow_create(
    request: WorkflowCreateRequestDto,
    state: State<'_, DesktopActivatedCommandDependencies>,
) -> Result<WorkflowDto, DesktopErrorDto> {
    let project_id = trusted_project(&request.project_id, &state).await?;
    let request_id = WorkflowCreateRequestId::from_uuid(uuid(&request.request_id)?)
        .ok_or_else(invalid_request)?;
    state
        .workflow_create
        .create_workflow(WorkflowCreateCommand::new(request_id, project_id))
        .await
        .map(|workflow| workflow_dto(&workflow))
        .map_err(workflow_error)
}

/// Returns the current Workflow and same-snapshot readiness.
#[tauri::command(rename_all = "snake_case")]
pub async fn workflow_get_current(
    request: WorkflowGetCurrentRequestDto,
    state: State<'_, DesktopActivatedCommandDependencies>,
) -> Result<WorkflowWithReadinessDto, DesktopErrorDto> {
    let project_id = trusted_project(&request.project_id, &state).await?;
    let result = state
        .workflow_get_current
        .get_current_workflow_with_readiness(project_id, &state.node_capabilities)
        .await
        .map_err(workflow_error)?;
    Ok(WorkflowWithReadinessDto {
        workflow: workflow_dto(&result.workflow),
        readiness: readiness_dto(result.readiness),
    })
}

/// Returns authoritative readiness for one Workflow.
#[tauri::command(rename_all = "snake_case")]
pub async fn workflow_check_readiness(
    request: WorkflowCheckReadinessRequestDto,
    state: State<'_, DesktopActivatedCommandDependencies>,
) -> Result<WorkflowReadinessDto, DesktopErrorDto> {
    let project_id = trusted_project(&request.project_id, &state).await?;
    state
        .workflow_check_readiness
        .check_project_workflow_readiness(project_id, workflow_id(&request.workflow_id)?)
        .await
        .map(readiness_dto)
        .map_err(workflow_error)
}

/// Admits and returns one durable queued Run.
#[tauri::command(rename_all = "snake_case")]
pub async fn workflow_start_run(
    request: WorkflowStartRunRequestDto,
    state: State<'_, DesktopActivatedCommandDependencies>,
) -> Result<WorkflowRunDto, DesktopErrorDto> {
    let project_id = trusted_project(&request.project_id, &state).await?;
    let command = WorkflowStartRunCommand::new(
        WorkflowRunRequestId::from_uuid(uuid(&request.request_id)?).ok_or_else(invalid_request)?,
        workflow_id(&request.workflow_id)?,
        revision(&request.workflow_revision)?,
        scope(request.scope)?,
    );
    state
        .workflow_start_run
        .start_project_workflow_run(project_id, command)
        .await
        .map(|run| run_dto(&run))
        .map_err(workflow_error)
}

/// Durably cancels one Run.
#[tauri::command(rename_all = "snake_case")]
pub async fn workflow_cancel_run(
    request: WorkflowRunRequestDto,
    state: State<'_, DesktopActivatedCommandDependencies>,
) -> Result<WorkflowRunDto, DesktopErrorDto> {
    let project_id = trusted_project(&request.project_id, &state).await?;
    state
        .workflow_cancel_run
        .cancel_project_workflow_run(project_id, run_id(&request.workflow_run_id)?)
        .await
        .map(|run| run_dto(&run))
        .map_err(workflow_error)
}

/// Returns one Project-scoped durable Run.
#[tauri::command(rename_all = "snake_case")]
pub async fn workflow_get_run(
    request: WorkflowRunRequestDto,
    state: State<'_, DesktopActivatedCommandDependencies>,
) -> Result<WorkflowRunDto, DesktopErrorDto> {
    let project_id = trusted_project(&request.project_id, &state).await?;
    state
        .workflow_get_run
        .get_workflow_run(project_id, run_id(&request.workflow_run_id)?)
        .await
        .map(|run| run_dto(&run))
        .map_err(workflow_error)
}

/// Returns one bounded ascending page of durable Run events.
#[tauri::command(rename_all = "snake_case")]
pub async fn workflow_list_run_events(
    request: WorkflowListRunEventsRequestDto,
    state: State<'_, DesktopActivatedCommandDependencies>,
) -> Result<WorkflowRunEventPageDto, DesktopErrorDto> {
    let project_id = trusted_project(&request.project_id, &state).await?;
    let page = state
        .workflow_list_run_events
        .list_workflow_run_events(
            project_id,
            run_id(&request.workflow_run_id)?,
            request.after_sequence.as_deref().map(sequence).transpose()?,
            request.limit,
        )
        .await
        .map_err(workflow_error)?;
    Ok(WorkflowRunEventPageDto {
        events: page.events.iter().map(event_dto).collect(),
        next_sequence: page.next_sequence.map(|value| value.get().to_string()),
    })
}

/// Returns the current Text, Image, Video, or Audio node shell.
#[tauri::command(rename_all = "snake_case")]
pub async fn workflow_get_node_presentation(
    request: WorkflowGetNodePresentationRequestDto,
    state: State<'_, DesktopActivatedCommandDependencies>,
) -> Result<crate::workflow_presentation_dto::WorkflowNodePresentationDto, DesktopErrorDto> {
    let project_id = trusted_project(&request.project_id, &state).await?;
    state
        .workflow_get_node_presentation
        .get_workflow_node_presentation(
            project_id,
            workflow_id(&request.workflow_id)?,
            workflow_node_id(&request.node_id)?,
        )
        .await
        .map(crate::workflow_presentation_dto::node_presentation_dto)
        .map_err(workflow_error)
}

pub(crate) async fn trusted_project(
    value: &str,
    state: &DesktopActivatedCommandDependencies,
) -> Result<ProjectId, DesktopErrorDto> {
    let id = ProjectId::from_uuid(uuid(value)?).ok_or_else(invalid_request)?;
    state.get.get_project(id).await.map_err(project_error)?;
    Ok(id)
}

fn scope(value: WorkflowRunScopeDto) -> Result<WorkflowRunScope, DesktopErrorDto> {
    match value {
        WorkflowRunScopeDto::WholeWorkflow => Ok(WorkflowRunScope::WholeWorkflow),
        WorkflowRunScopeDto::ThroughNode { node_id } => {
            Ok(WorkflowRunScope::ThroughNode(workflow_node_id(&node_id)?))
        }
    }
}

pub(crate) fn workflow_id(value: &str) -> Result<WorkflowId, DesktopErrorDto> {
    WorkflowId::from_uuid(uuid(value)?).map_err(|_| invalid_request())
}

pub(crate) fn workflow_node_id(value: &str) -> Result<WorkflowNodeId, DesktopErrorDto> {
    WorkflowNodeId::from_uuid(uuid(value)?).map_err(|_| invalid_request())
}

fn run_id(value: &str) -> Result<WorkflowRunId, DesktopErrorDto> {
    WorkflowRunId::from_uuid(uuid(value)?).ok_or_else(invalid_request)
}

pub(crate) fn revision(value: &str) -> Result<WorkflowRevision, DesktopErrorDto> {
    canonical_u64(value)
        .and_then(|value| WorkflowRevision::new(value).map_err(|_| invalid_request()))
}

fn sequence(value: &str) -> Result<WorkflowRunEventSequence, DesktopErrorDto> {
    canonical_u64(value)
        .and_then(|value| WorkflowRunEventSequence::new(value).map_err(|_| invalid_request()))
}

pub(crate) fn canonical_u64(value: &str) -> Result<u64, DesktopErrorDto> {
    let parsed = value.parse::<u64>().map_err(|_| invalid_request())?;
    (parsed.to_string() == value).then_some(parsed).ok_or_else(invalid_request)
}

pub(crate) fn uuid(value: &str) -> Result<Uuid, DesktopErrorDto> {
    let parsed = Uuid::parse_str(value).map_err(|_| invalid_request())?;
    (parsed.hyphenated().to_string() == value).then_some(parsed).ok_or_else(invalid_request)
}

fn project_error(error: ProjectApplicationError) -> DesktopErrorDto {
    error_dto(
        match error {
            ProjectApplicationError::ProjectNotFound { .. } => DesktopErrorCode::ProjectNotFound,
            _ => DesktopErrorCode::StorageUnavailable,
        },
        None,
    )
}

pub(crate) fn workflow_error(error: WorkflowApplicationError) -> DesktopErrorDto {
    let (code, target) = match error {
        WorkflowApplicationError::WorkflowNotFound { key } => (
            DesktopErrorCode::WorkflowNotFound,
            match key {
                engine::workflow::WorkflowLoadKey::Workflow(id) => {
                    Some(DesktopErrorTarget::Workflow {
                        workflow_id: id.as_uuid().hyphenated().to_string(),
                    })
                }
                engine::workflow::WorkflowLoadKey::Project(_) => None,
            },
        ),
        WorkflowApplicationError::WorkflowRunNotFound => {
            (DesktopErrorCode::WorkflowRunNotFound, None)
        }
        WorkflowApplicationError::WorkflowRevisionConflict
        | WorkflowApplicationError::WorkflowRunRevisionMismatch => {
            (DesktopErrorCode::WorkflowRevisionConflict, None)
        }
        WorkflowApplicationError::WorkflowNotReady { .. } => {
            (DesktopErrorCode::WorkflowNotReady, None)
        }
        WorkflowApplicationError::WorkflowCreationIdempotencyConflict
        | WorkflowApplicationError::WorkflowMutationIdempotencyConflict
        | WorkflowApplicationError::WorkflowRunIdempotencyConflict => {
            (DesktopErrorCode::WorkflowMutationConflict, None)
        }
        WorkflowApplicationError::WorkflowGraph(_)
        | WorkflowApplicationError::WorkflowDomain(_)
        | WorkflowApplicationError::WorkflowRunEventLimitOutOfBounds { .. } => {
            (DesktopErrorCode::WorkflowInvalidRequest, None)
        }
        _ => (DesktopErrorCode::StorageUnavailable, None),
    };
    error_dto(code, target)
}

pub(crate) fn invalid_request() -> DesktopErrorDto {
    error_dto(DesktopErrorCode::WorkflowInvalidRequest, None)
}

fn error_dto(code: DesktopErrorCode, target: Option<DesktopErrorTarget>) -> DesktopErrorDto {
    DesktopErrorDto::from_context(DesktopErrorContext {
        code,
        retryable: false,
        retry_after_epoch_ms: None,
        target,
        correlation_id: None,
    })
}

#[cfg(test)]
mod tests;
