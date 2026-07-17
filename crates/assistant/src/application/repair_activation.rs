use projects::project::domain::ProjectId;

use crate::{
    application::AssistantActiveInvocationRegistry,
    domain::{AssistantModelInvocationId, AssistantRepairActivationId, AssistantSessionId},
    interfaces::{
        AssistantApplicationError, AssistantFailedWorkflowRunId, AssistantModelRunnerInterface,
        AssistantModelTurnRequest, AssistantModelTurnResult, AssistantModelTurnStart,
        AssistantRepairActivation, AssistantRepairActivationRecordResult,
        AssistantRepairActivationRepositoryInterface, AssistantWorkflowRunReaderInterface,
        AssistantWorkspaceSnapshotReaderInterface,
    },
};

/// Trusted failed-Run fact admitted after Desktop resolves process identities.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssistantActivateRepairCommand {
    pub project_id: ProjectId,
    pub session_id: AssistantSessionId,
    pub activation_id: AssistantRepairActivationId,
    pub invocation_id: AssistantModelInvocationId,
    pub failed_workflow_run_id: AssistantFailedWorkflowRunId,
    pub created_at_epoch_ms: i64,
}

/// Records one factual activation and starts at most one repair model turn.
pub struct AssistantActivateRepairUseCase<R, F, M, W> {
    activation_repository: R,
    workflow_run_reader: F,
    model_runner: M,
    workspace_reader: W,
    active_invocations: AssistantActiveInvocationRegistry,
}

impl<R, F, M, W> AssistantActivateRepairUseCase<R, F, M, W>
where
    R: AssistantRepairActivationRepositoryInterface,
    F: AssistantWorkflowRunReaderInterface,
    M: AssistantModelRunnerInterface,
    W: AssistantWorkspaceSnapshotReaderInterface,
{
    #[must_use]
    pub const fn new(
        activation_repository: R,
        workflow_run_reader: F,
        model_runner: M,
        workspace_reader: W,
        active_invocations: AssistantActiveInvocationRegistry,
    ) -> Self {
        Self {
            activation_repository,
            workflow_run_reader,
            model_runner,
            workspace_reader,
            active_invocations,
        }
    }

    pub async fn activate_repair(
        &self,
        command: AssistantActivateRepairCommand,
    ) -> Result<Option<AssistantModelTurnResult>, AssistantApplicationError> {
        let _guard = self.active_invocations.claim(command.project_id, command.session_id)?;
        let failed_run = self
            .workflow_run_reader
            .read_assistant_workflow_run(command.project_id, command.failed_workflow_run_id)
            .await?
            .ok_or(AssistantApplicationError::NotFound)?;
        let activation = AssistantRepairActivation::new(
            command.activation_id,
            command.project_id,
            command.session_id,
            command.failed_workflow_run_id,
            failed_run.canonical_bytes().to_vec(),
            command.created_at_epoch_ms,
        )?;
        let recorded =
            self.activation_repository.record_or_get_repair_activation(activation).await?;
        let AssistantRepairActivationRecordResult::Created(activation) = recorded else {
            return Ok(None);
        };

        let workspace_request = crate::interfaces::AssistantWorkspaceSnapshotRequest::try_new(
            command.project_id,
            command.session_id,
            None,
            Vec::new(),
            Vec::new(),
        )?;
        let workspace_snapshot = self
            .workspace_reader
            .read_assistant_workspace_snapshot(workspace_request.clone())
            .await?;
        let result = self
            .model_runner
            .start_assistant_model_turn(AssistantModelTurnRequest {
                project_id: command.project_id,
                session_id: command.session_id,
                invocation_id: command.invocation_id,
                start: AssistantModelTurnStart::RepairActivation(activation),
                workspace_request,
                workspace_snapshot,
            })
            .await?;
        Ok(Some(result))
    }
}

#[cfg(test)]
mod tests;
