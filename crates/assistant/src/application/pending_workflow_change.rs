use projects::project::domain::ProjectId;

use crate::{
    domain::{AssistantSessionId, AssistantWorkflowChangeAggregate},
    interfaces::{AssistantApplicationError, AssistantWorkflowChangeRepositoryInterface},
};

/// Reads the one pending approval visible to an exact Project/Session.
pub struct AssistantGetPendingWorkflowChangeUseCase<R> {
    repository: R,
}

impl<R: AssistantWorkflowChangeRepositoryInterface> AssistantGetPendingWorkflowChangeUseCase<R> {
    #[must_use]
    pub const fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn get_pending_workflow_change(
        &self,
        project_id: ProjectId,
        session_id: AssistantSessionId,
    ) -> Result<Option<AssistantWorkflowChangeAggregate>, AssistantApplicationError> {
        self.repository.load_pending_assistant_workflow_change(project_id, session_id).await
    }
}
