//! Canonical Tauri entry points for the Rust-owned Assistant application slice.

use assistant::{
    application::{
        AssistantDecideWorkflowChangeCommand, AssistantDecideWorkflowChangeUseCase,
        AssistantGetPendingWorkflowChangeUseCase, AssistantSendMessageCommand,
        AssistantSendMessageUseCase, AssistantWorkflowChangeDecision,
    },
    domain::{
        AssistantApprovalScopeId, AssistantModelInvocationId, AssistantUserIntent,
        AssistantWorkflowChangeAggregate, AssistantWorkflowChangeDecisionScope,
        AssistantWorkflowChangeId, AssistantWorkflowMutationDigest, WorkflowRevisionBoundaryValue,
    },
    interfaces::{
        AssistantApplicationError, AssistantClockInterface,
        AssistantModelContinuationStoreInterface, AssistantModelRunnerInterface,
        AssistantSelectedAssetId, AssistantSelectedWorkflowNodeId,
        AssistantWorkflowChangeRepositoryInterface, AssistantWorkspaceSnapshotReaderInterface,
        AssistantWorkspaceSnapshotRequest,
    },
};
use async_trait::async_trait;
use projects::project::domain::ProjectId;
use tauri::State;
use uuid::Uuid;

use crate::{
    assistant_adapters::SystemAssistantClockAdapterImpl,
    assistant_command_dto::{
        AssistantDecideWorkflowChangeRequestDto, AssistantGetPendingWorkflowChangeRequestDto,
        AssistantPendingWorkflowChangeDto, AssistantSendMessageRequestDto,
        AssistantSendMessageResultDto, AssistantWorkflowChangeDecisionDto,
        AssistantWorkflowChangeDecisionResultDto, pending_workflow_change_dto,
        workflow_change_decision_result_dto,
    },
    assistant_tool_runtime::assistant_session_id,
    composition::DesktopActivatedCommandDependencies,
    desktop_backend_config::{DesktopErrorCode, DesktopErrorContext, DesktopErrorDto},
    project_commands::project_error,
};

#[async_trait]
pub trait DesktopAssistantCommandInterface: Send + Sync {
    async fn send_assistant_message(
        &self,
        command: AssistantSendMessageCommand,
    ) -> Result<assistant::interfaces::AssistantModelTurnResult, AssistantApplicationError>;

    async fn get_pending_assistant_workflow_change(
        &self,
        project_id: ProjectId,
        session_id: assistant::domain::AssistantSessionId,
    ) -> Result<Option<AssistantWorkflowChangeAggregate>, AssistantApplicationError>;

    async fn decide_assistant_workflow_change(
        &self,
        command: AssistantDecideWorkflowChangeCommand,
    ) -> Result<AssistantWorkflowChangeAggregate, AssistantApplicationError>;
}

pub struct DesktopAssistantCommandAdapterImpl<M, W, R, S> {
    send: AssistantSendMessageUseCase<M, W>,
    pending: AssistantGetPendingWorkflowChangeUseCase<R>,
    decide: AssistantDecideWorkflowChangeUseCase<R, S>,
}

impl<M, W, R, S> DesktopAssistantCommandAdapterImpl<M, W, R, S> {
    #[must_use]
    pub const fn new(
        send: AssistantSendMessageUseCase<M, W>,
        pending: AssistantGetPendingWorkflowChangeUseCase<R>,
        decide: AssistantDecideWorkflowChangeUseCase<R, S>,
    ) -> Self {
        Self { send, pending, decide }
    }
}

#[async_trait]
impl<M, W, R, S> DesktopAssistantCommandInterface for DesktopAssistantCommandAdapterImpl<M, W, R, S>
where
    M: AssistantModelRunnerInterface,
    W: AssistantWorkspaceSnapshotReaderInterface,
    R: AssistantWorkflowChangeRepositoryInterface,
    S: AssistantModelContinuationStoreInterface,
{
    async fn send_assistant_message(
        &self,
        command: AssistantSendMessageCommand,
    ) -> Result<assistant::interfaces::AssistantModelTurnResult, AssistantApplicationError> {
        self.send.send_message(command).await
    }

    async fn get_pending_assistant_workflow_change(
        &self,
        project_id: ProjectId,
        session_id: assistant::domain::AssistantSessionId,
    ) -> Result<Option<AssistantWorkflowChangeAggregate>, AssistantApplicationError> {
        self.pending.get_pending_workflow_change(project_id, session_id).await
    }

    async fn decide_assistant_workflow_change(
        &self,
        command: AssistantDecideWorkflowChangeCommand,
    ) -> Result<AssistantWorkflowChangeAggregate, AssistantApplicationError> {
        self.decide.decide_workflow_change(command).await
    }
}

#[tauri::command(rename_all = "snake_case")]
pub async fn assistant_send_message(
    request: AssistantSendMessageRequestDto,
    state: State<'_, DesktopActivatedCommandDependencies>,
) -> Result<AssistantSendMessageResultDto, DesktopErrorDto> {
    assistant_send_message_with_dependencies(request, &state).await
}

pub async fn assistant_send_message_with_dependencies(
    request: AssistantSendMessageRequestDto,
    state: &DesktopActivatedCommandDependencies,
) -> Result<AssistantSendMessageResultDto, DesktopErrorDto> {
    let project_id = resolve_project(&request.project_id, state).await?;
    let invocation_id =
        AssistantModelInvocationId::from_uuid(Uuid::new_v4()).map_err(|_| invalid_request())?;
    let workspace_request = workspace_request(project_id, &request)?;
    let result = state
        .assistant
        .send_assistant_message(AssistantSendMessageCommand {
            workspace_request,
            invocation_id,
            intent: AssistantUserIntent::new(request.text).map_err(|_| invalid_request())?,
        })
        .await
        .map_err(assistant_error)?;
    let final_text =
        String::from_utf8(result.as_bytes().to_vec()).map_err(|_| invalid_request())?;
    Ok(AssistantSendMessageResultDto {
        invocation_id: invocation_id.as_uuid().to_string(),
        final_text,
    })
}

#[tauri::command(rename_all = "snake_case")]
pub async fn assistant_get_pending_workflow_change(
    request: AssistantGetPendingWorkflowChangeRequestDto,
    state: State<'_, DesktopActivatedCommandDependencies>,
) -> Result<Option<AssistantPendingWorkflowChangeDto>, DesktopErrorDto> {
    assistant_get_pending_workflow_change_with_dependencies(request, &state).await
}

pub async fn assistant_get_pending_workflow_change_with_dependencies(
    request: AssistantGetPendingWorkflowChangeRequestDto,
    state: &DesktopActivatedCommandDependencies,
) -> Result<Option<AssistantPendingWorkflowChangeDto>, DesktopErrorDto> {
    let project_id = resolve_project(&request.project_id, state).await?;
    state
        .assistant
        .get_pending_assistant_workflow_change(
            project_id,
            assistant_session_id(project_id).map_err(assistant_error)?,
        )
        .await
        .map_err(assistant_error)?
        .map(pending_workflow_change_dto)
        .transpose()
        .map_err(|_| invalid_request())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn assistant_decide_workflow_change(
    request: AssistantDecideWorkflowChangeRequestDto,
    state: State<'_, DesktopActivatedCommandDependencies>,
) -> Result<AssistantWorkflowChangeDecisionResultDto, DesktopErrorDto> {
    assistant_decide_workflow_change_with_dependencies(request, &state).await
}

pub async fn assistant_decide_workflow_change_with_dependencies(
    request: AssistantDecideWorkflowChangeRequestDto,
    state: &DesktopActivatedCommandDependencies,
) -> Result<AssistantWorkflowChangeDecisionResultDto, DesktopErrorDto> {
    let project_id = resolve_project(&request.project_id, state).await?;
    let session_id = assistant_session_id(project_id).map_err(assistant_error)?;
    let workflow_change_id = workflow_change_id(&request.workflow_change_id)?;
    let scope = AssistantWorkflowChangeDecisionScope {
        project_id,
        session_id,
        change_id: workflow_change_id,
        approval_scope_id: approval_scope_id(&request.approval_scope_id)?,
        mutation_digest: mutation_digest(&request.mutation_digest_hex)?,
    };
    let now = SystemAssistantClockAdapterImpl
        .current_assistant_time()
        .map_err(assistant_error)?
        .epoch_ms();
    state
        .assistant
        .decide_assistant_workflow_change(AssistantDecideWorkflowChangeCommand {
            workflow_change_id,
            scope,
            decision: match request.decision {
                AssistantWorkflowChangeDecisionDto::Approve => {
                    AssistantWorkflowChangeDecision::Approve
                }
                AssistantWorkflowChangeDecisionDto::Reject => {
                    AssistantWorkflowChangeDecision::Reject
                }
            },
            now_epoch_ms: now,
        })
        .await
        .map_err(assistant_error)
        .map(workflow_change_decision_result_dto)
}

async fn resolve_project(
    value: &str,
    state: &DesktopActivatedCommandDependencies,
) -> Result<ProjectId, DesktopErrorDto> {
    let project_id = ProjectId::from_uuid(uuid(value)?).ok_or_else(invalid_request)?;
    state.get.get_project(project_id).await.map_err(project_error)?;
    Ok(project_id)
}

fn workspace_request(
    project_id: ProjectId,
    request: &AssistantSendMessageRequestDto,
) -> Result<AssistantWorkspaceSnapshotRequest, DesktopErrorDto> {
    let observed = match (request.workflow_present, request.workflow_revision.as_deref()) {
        (false, None) => None,
        (true, Some(value)) => Some(
            WorkflowRevisionBoundaryValue::new(decimal(value)?).map_err(|_| invalid_request())?,
        ),
        _ => return Err(invalid_request()),
    };
    AssistantWorkspaceSnapshotRequest::try_new(
        project_id,
        assistant_session_id(project_id).map_err(assistant_error)?,
        observed,
        selections(&request.selected_node_ids, AssistantSelectedWorkflowNodeId::from_bytes)?,
        selections(&request.selected_asset_ids, AssistantSelectedAssetId::from_bytes)?,
    )
    .map_err(assistant_error)
}

fn selections<T>(
    values: &[String],
    parse: impl Fn([u8; 16]) -> Result<T, AssistantApplicationError>,
) -> Result<Vec<T>, DesktopErrorDto> {
    values.iter().map(|value| parse(*uuid(value)?.as_bytes()).map_err(assistant_error)).collect()
}

fn workflow_change_id(value: &str) -> Result<AssistantWorkflowChangeId, DesktopErrorDto> {
    AssistantWorkflowChangeId::from_uuid(uuid(value)?).map_err(|_| invalid_request())
}

fn approval_scope_id(value: &str) -> Result<AssistantApprovalScopeId, DesktopErrorDto> {
    AssistantApprovalScopeId::from_uuid(uuid(value)?).map_err(|_| invalid_request())
}

fn mutation_digest(value: &str) -> Result<AssistantWorkflowMutationDigest, DesktopErrorDto> {
    if value.len() != 64 {
        return Err(invalid_request());
    }
    let mut bytes = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        bytes[index] = (digit(pair[0])? << 4) | digit(pair[1])?;
    }
    Ok(AssistantWorkflowMutationDigest::new(bytes))
}

fn digit(value: u8) -> Result<u8, DesktopErrorDto> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        _ => Err(invalid_request()),
    }
}

fn uuid(value: &str) -> Result<Uuid, DesktopErrorDto> {
    let parsed = Uuid::parse_str(value).map_err(|_| invalid_request())?;
    if parsed.to_string() == value { Ok(parsed) } else { Err(invalid_request()) }
}

fn decimal(value: &str) -> Result<u64, DesktopErrorDto> {
    let parsed: u64 = value.parse().map_err(|_| invalid_request())?;
    if parsed.to_string() == value { Ok(parsed) } else { Err(invalid_request()) }
}

fn invalid_request() -> DesktopErrorDto {
    assistant_error(AssistantApplicationError::ProtocolViolation)
}

fn assistant_error(error: AssistantApplicationError) -> DesktopErrorDto {
    let (code, retryable, target) = match error {
        AssistantApplicationError::NotFound => (DesktopErrorCode::AssistantNotFound, false, None),
        AssistantApplicationError::NotVisible => {
            (DesktopErrorCode::AssistantNotVisible, false, None)
        }
        AssistantApplicationError::ConcurrentInvocation => {
            (DesktopErrorCode::AssistantBusy, true, None)
        }
        AssistantApplicationError::PendingApprovalExists => {
            (DesktopErrorCode::AssistantPendingApproval, false, None)
        }
        AssistantApplicationError::ApprovalExpired => {
            (DesktopErrorCode::AssistantApprovalExpired, false, None)
        }
        AssistantApplicationError::ApprovalMismatch
        | AssistantApplicationError::InvalidTransition => {
            (DesktopErrorCode::AssistantApprovalMismatch, false, None)
        }
        AssistantApplicationError::ModelUnavailable
        | AssistantApplicationError::DeadlineExceeded => {
            (DesktopErrorCode::ProviderUnavailable, true, None)
        }
        _ => (DesktopErrorCode::AssistantProtocolViolation, false, None),
    };
    DesktopErrorDto::from_context(DesktopErrorContext {
        code,
        retryable,
        retry_after_epoch_ms: None,
        target,
        correlation_id: None,
    })
}
