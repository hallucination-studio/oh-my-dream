//! Task-aware classification of non-terminal Workflow Runs after process restart.

use std::sync::Arc;

use crate::workflow::WorkflowGenerationTaskOrigin;

use super::{
    WorkflowApplicationError, WorkflowGenerationTaskRecoveryObservation,
    WorkflowGenerationTaskRecoveryReaderInterface, WorkflowNodeExecutionState,
    WorkflowRunAggregate, WorkflowRunState,
};

/// Closed startup disposition for one non-terminal Workflow Run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkflowRunRestartDisposition {
    /// Durable Workflow or Task evidence permits idempotent effect replay.
    ReplaySafe,
    /// Active in-process capability work had no durable Task handoff.
    InterruptUnsafe,
}

/// Classifies exact Run snapshots without reading provider or adapter state.
pub struct WorkflowClassifyRunsAfterRestartUseCase {
    task_recovery: Arc<dyn WorkflowGenerationTaskRecoveryReaderInterface>,
}

impl WorkflowClassifyRunsAfterRestartUseCase {
    /// Wires the only Task recovery evidence boundary.
    #[must_use]
    pub fn new(task_recovery: Arc<dyn WorkflowGenerationTaskRecoveryReaderInterface>) -> Self {
        Self { task_recovery }
    }

    /// Returns the safe replay or interruption decision for one non-terminal Run.
    pub async fn classify_workflow_run_after_restart(
        &self,
        run: &WorkflowRunAggregate,
    ) -> Result<WorkflowRunRestartDisposition, WorkflowApplicationError> {
        if run.state() == WorkflowRunState::Queued {
            return Ok(WorkflowRunRestartDisposition::ReplaySafe);
        }
        if run.state() != WorkflowRunState::Running {
            return Err(WorkflowApplicationError::WorkflowGenerationTaskRecoveryReadFailure);
        }
        let mut disposition = WorkflowRunRestartDisposition::ReplaySafe;
        for (index, execution) in run.node_executions().iter().enumerate() {
            if !matches!(
                execution.state(),
                WorkflowNodeExecutionState::Running
                    | WorkflowNodeExecutionState::WaitingForExternalCompletion
            ) {
                continue;
            }
            let planned = &run.plan().nodes()[index];
            let origin = WorkflowGenerationTaskOrigin {
                project_id: run.project_id(),
                workflow_id: run.workflow_id(),
                workflow_revision: run.workflow_revision(),
                workflow_run_id: run.run_id(),
                workflow_node_id: execution.node_id(),
                node_execution_id: execution.execution_id(),
                capability_contract_ref: planned.capability_contract.clone(),
            };
            let observed =
                self.task_recovery.read_workflow_generation_task_recovery(&origin).await?;
            match (execution.state(), observed) {
                (
                    WorkflowNodeExecutionState::Running,
                    WorkflowGenerationTaskRecoveryObservation::QueuedPreHandoff,
                )
                | (
                    WorkflowNodeExecutionState::WaitingForExternalCompletion,
                    WorkflowGenerationTaskRecoveryObservation::Active
                    | WorkflowGenerationTaskRecoveryObservation::TerminalNotificationPending,
                ) => {}
                (
                    WorkflowNodeExecutionState::Running,
                    WorkflowGenerationTaskRecoveryObservation::Absent,
                ) => disposition = WorkflowRunRestartDisposition::InterruptUnsafe,
                _ => {
                    return Err(
                        WorkflowApplicationError::WorkflowGenerationTaskRecoveryReadFailure,
                    );
                }
            }
        }
        Ok(disposition)
    }
}
