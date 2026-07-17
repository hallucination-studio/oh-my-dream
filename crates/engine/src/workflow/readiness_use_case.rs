use std::sync::Arc;

use projects::project::domain::ProjectId;

use crate::{node_capability::WorkflowNodeCapabilityRegistry, workflow_graph::WorkflowId};

use super::{
    WorkflowAggregateRepositoryInterface, WorkflowApplicationError, WorkflowLoadKey,
    WorkflowReadinessResult, use_case::check_readiness,
};

/// Computes structural and capability-owned external readiness.
pub struct WorkflowCheckReadinessUseCase<R> {
    repository: Arc<R>,
    capabilities: Arc<WorkflowNodeCapabilityRegistry>,
}

impl<R: WorkflowAggregateRepositoryInterface> WorkflowCheckReadinessUseCase<R> {
    /// Wires the Workflow repository and immutable exact capability registry.
    #[must_use]
    pub fn new(repository: Arc<R>, capabilities: Arc<WorkflowNodeCapabilityRegistry>) -> Self {
        Self { repository, capabilities }
    }

    /// Loads one Workflow and evaluates all nodes against one shared five-second deadline.
    pub async fn check_workflow_readiness(
        &self,
        workflow_id: WorkflowId,
    ) -> Result<WorkflowReadinessResult, WorkflowApplicationError> {
        self.check_workflow_readiness_internal(None, workflow_id).await
    }

    /// Evaluates readiness only when the Workflow belongs to the trusted Project.
    pub async fn check_project_workflow_readiness(
        &self,
        project_id: ProjectId,
        workflow_id: WorkflowId,
    ) -> Result<WorkflowReadinessResult, WorkflowApplicationError> {
        self.check_workflow_readiness_internal(Some(project_id), workflow_id).await
    }

    async fn check_workflow_readiness_internal(
        &self,
        project_id: Option<ProjectId>,
        workflow_id: WorkflowId,
    ) -> Result<WorkflowReadinessResult, WorkflowApplicationError> {
        let key = WorkflowLoadKey::Workflow(workflow_id);
        let workflow = self
            .repository
            .load_workflow(key)
            .await?
            .filter(|workflow| project_id.is_none_or(|id| workflow.project_id == id))
            .ok_or(WorkflowApplicationError::WorkflowNotFound { key })?;
        check_readiness(&workflow, &self.capabilities).await
    }
}
