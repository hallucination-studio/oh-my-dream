//! Exact Workflow-origin state reader for Generation Task effects.

use async_trait::async_trait;
use engine::workflow::{
    WorkflowNodeExecutionState, WorkflowRunLoadKey, WorkflowRunRepositoryInterface,
    WorkflowRunState,
};
use tasks::generation_task::{
    GenerationTaskAggregate, GenerationTaskBoundaryError, GenerationTaskOriginState,
    GenerationTaskOriginStateReaderInterface,
};

/// Reads only the canonical Workflow Run aggregate for one exact Task origin.
#[derive(Clone)]
pub struct DesktopGenerationTaskOriginStateReaderAdapterImpl<R> {
    repository: R,
}

impl<R> DesktopGenerationTaskOriginStateReaderAdapterImpl<R> {
    /// Wires the Workflow-owned Run repository.
    #[must_use]
    pub const fn new(repository: R) -> Self {
        Self { repository }
    }
}

#[async_trait]
impl<R> GenerationTaskOriginStateReaderInterface
    for DesktopGenerationTaskOriginStateReaderAdapterImpl<R>
where
    R: WorkflowRunRepositoryInterface,
{
    async fn read_generation_task_origin_state(
        &self,
        task: &GenerationTaskAggregate,
    ) -> Result<GenerationTaskOriginState, GenerationTaskBoundaryError> {
        let origin = task.origin();
        let run = self
            .repository
            .load_workflow_run(WorkflowRunLoadKey::ProjectScoped {
                project_id: origin.project_id(),
                workflow_run_id: origin.workflow_run_id(),
            })
            .await
            .map_err(|_| GenerationTaskBoundaryError::Transient)?
            .ok_or(GenerationTaskBoundaryError::Permanent)?;
        if run.workflow_id() != origin.workflow_id()
            || run.workflow_revision() != origin.workflow_revision()
        {
            return Err(GenerationTaskBoundaryError::Permanent);
        }
        let index = run
            .node_executions()
            .iter()
            .position(|execution| execution.execution_id() == origin.workflow_node_execution_id())
            .ok_or(GenerationTaskBoundaryError::Permanent)?;
        let execution = &run.node_executions()[index];
        let planned = &run.plan().nodes()[index];
        if execution.node_id() != origin.workflow_node_id()
            || planned.node_id != origin.workflow_node_id()
            || planned.capability_contract != *origin.capability_contract_ref()
        {
            return Err(GenerationTaskBoundaryError::Permanent);
        }
        if run.state() == WorkflowRunState::Cancelled
            || execution.state() == WorkflowNodeExecutionState::Cancelled
        {
            return Ok(GenerationTaskOriginState::Cancelled);
        }
        match execution.state() {
            WorkflowNodeExecutionState::Running => Ok(GenerationTaskOriginState::Running),
            WorkflowNodeExecutionState::WaitingForExternalCompletion => {
                Ok(GenerationTaskOriginState::WaitingForExternalCompletion)
            }
            WorkflowNodeExecutionState::Succeeded
            | WorkflowNodeExecutionState::Failed
            | WorkflowNodeExecutionState::Blocked => Ok(GenerationTaskOriginState::Terminal),
            WorkflowNodeExecutionState::Pending | WorkflowNodeExecutionState::Cancelled => {
                Err(GenerationTaskBoundaryError::Permanent)
            }
        }
    }
}
