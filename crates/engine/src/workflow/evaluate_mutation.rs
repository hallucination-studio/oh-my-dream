//! Non-committing canonical Workflow mutation evaluation.

use std::sync::Arc;

use projects::project::domain::ProjectId;

use crate::{
    node_capability::WorkflowNodeCapabilityRegistry,
    workflow_graph::{
        WorkflowApplyMutationCommand, WorkflowMutationAction, WorkflowMutationRequestId,
        WorkflowRevision, WorkflowUpdatedAt,
    },
};

use super::{
    WorkflowAggregateRepositoryInterface, WorkflowApplicationError, WorkflowClockInterface,
    WorkflowLoadKey, WorkflowMutationResult, use_case::check_readiness,
};

/// Non-committing Project-scoped mutation evaluation input.
pub struct WorkflowEvaluateMutationCommand {
    /// Trusted Project scope.
    pub project_id: ProjectId,
    /// Ephemeral request identity used only for canonical command construction.
    pub request_id: WorkflowMutationRequestId,
    /// Exact required current revision.
    pub base_revision: WorkflowRevision,
    /// Ordered typed Workflow actions.
    pub actions: Vec<WorkflowMutationAction>,
}

/// Evaluates one typed mutation through the canonical aggregate without persistence.
pub struct WorkflowEvaluateMutationUseCase<R, C> {
    repository: Arc<R>,
    clock: Arc<C>,
    capabilities: Arc<WorkflowNodeCapabilityRegistry>,
}

impl<R, C> WorkflowEvaluateMutationUseCase<R, C>
where
    R: WorkflowAggregateRepositoryInterface,
    C: WorkflowClockInterface,
{
    /// Wires the same boundaries and capability registry used by committed mutation.
    #[must_use]
    pub fn new(
        repository: Arc<R>,
        clock: Arc<C>,
        capabilities: Arc<WorkflowNodeCapabilityRegistry>,
    ) -> Self {
        Self { repository, clock, capabilities }
    }

    /// Returns the exact in-memory candidate and readiness without committing either.
    pub async fn evaluate_workflow_mutation(
        &self,
        command: WorkflowEvaluateMutationCommand,
    ) -> Result<WorkflowMutationResult, WorkflowApplicationError> {
        let key = WorkflowLoadKey::Project(command.project_id);
        let current = self
            .repository
            .load_workflow(key)
            .await?
            .ok_or(WorkflowApplicationError::WorkflowNotFound { key })?;
        if current.revision != command.base_revision {
            return Err(WorkflowApplicationError::WorkflowRevisionConflict);
        }
        let mutation = WorkflowApplyMutationCommand::try_new(
            command.request_id,
            current.id,
            command.base_revision,
            command.actions,
        )?;
        let observed = WorkflowUpdatedAt::from_utc_milliseconds(
            self.clock.current_workflow_time()?.as_utc_milliseconds(),
        )?;
        let workflow = current.apply_mutation_command(&mutation, observed, &self.capabilities)?;
        let readiness = check_readiness(&workflow, &self.capabilities).await?;
        Ok(WorkflowMutationResult { workflow, readiness })
    }
}
