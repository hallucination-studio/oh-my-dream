use std::{
    collections::BTreeSet,
    sync::{Arc, Mutex},
};

use projects::project::domain::ProjectId;

use crate::{
    domain::{AssistantModelInvocationId, AssistantSessionId, AssistantUserIntent},
    interfaces::{
        AssistantApplicationError, AssistantModelRunnerInterface, AssistantModelTurnRequest,
        AssistantModelTurnResult, AssistantWorkspaceSnapshotReaderInterface,
    },
};

/// Trusted command admitted after Desktop resolves the Project and identities.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssistantSendMessageCommand {
    pub project_id: ProjectId,
    pub session_id: AssistantSessionId,
    pub invocation_id: AssistantModelInvocationId,
    pub intent: AssistantUserIntent,
}

/// Process-local active-invocation authority shared by all send use cases.
#[derive(Clone, Default)]
pub struct AssistantActiveInvocationRegistry {
    active: Arc<Mutex<BTreeSet<(ProjectId, AssistantSessionId)>>>,
}

impl AssistantActiveInvocationRegistry {
    fn claim(
        &self,
        project_id: ProjectId,
        session_id: AssistantSessionId,
    ) -> Result<AssistantActiveInvocationGuard, AssistantApplicationError> {
        let key = (project_id, session_id);
        let mut active =
            self.active.lock().map_err(|_| AssistantApplicationError::ExternalBoundaryFailed)?;
        if !active.insert(key) {
            return Err(AssistantApplicationError::ConcurrentInvocation);
        }
        Ok(AssistantActiveInvocationGuard { active: Arc::clone(&self.active), key })
    }
}

struct AssistantActiveInvocationGuard {
    active: Arc<Mutex<BTreeSet<(ProjectId, AssistantSessionId)>>>,
    key: (ProjectId, AssistantSessionId),
}

impl Drop for AssistantActiveInvocationGuard {
    fn drop(&mut self) {
        if let Ok(mut active) = self.active.lock() {
            active.remove(&self.key);
        }
    }
}

/// Starts one bounded Assistant turn with authoritative workspace context.
pub struct AssistantSendMessageUseCase<M, W> {
    model_runner: M,
    workspace_reader: W,
    active_invocations: AssistantActiveInvocationRegistry,
}

impl<M, W> AssistantSendMessageUseCase<M, W>
where
    M: AssistantModelRunnerInterface,
    W: AssistantWorkspaceSnapshotReaderInterface,
{
    #[must_use]
    pub fn new(
        model_runner: M,
        workspace_reader: W,
        active_invocations: AssistantActiveInvocationRegistry,
    ) -> Self {
        Self { model_runner, workspace_reader, active_invocations }
    }

    pub async fn send_message(
        &self,
        command: AssistantSendMessageCommand,
    ) -> Result<AssistantModelTurnResult, AssistantApplicationError> {
        let _guard = self.active_invocations.claim(command.project_id, command.session_id)?;
        let workspace_snapshot = self
            .workspace_reader
            .read_assistant_workspace_snapshot(command.project_id, command.session_id)
            .await?;
        self.model_runner
            .start_assistant_model_turn(AssistantModelTurnRequest {
                project_id: command.project_id,
                session_id: command.session_id,
                invocation_id: command.invocation_id,
                intent: command.intent,
                workspace_snapshot,
            })
            .await
    }
}

#[cfg(test)]
mod tests;
