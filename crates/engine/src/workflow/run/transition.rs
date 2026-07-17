use crate::node_capability::{WorkflowNodeExecutionId, WorkflowNodeOutputSet};
use crate::workflow_graph::WorkflowNodeId;

use super::{
    WorkflowDomainError, WorkflowNodeExecutionBlockReason, WorkflowNodeExecutionEntity,
    WorkflowNodeExecutionFailure, WorkflowNodeExecutionState, WorkflowRunAggregate,
    WorkflowRunEvent, WorkflowRunEventPayload, WorkflowRunFailure, WorkflowRunState,
    WorkflowRunTime,
};

impl WorkflowRunAggregate {
    /// Transitions `Queued` to `Running` and records the durable event.
    pub fn start(&mut self, occurred_at: WorkflowRunTime) -> Result<(), WorkflowDomainError> {
        self.ensure_event_can_be_recorded(occurred_at)?;
        match self.state {
            WorkflowRunState::Queued => {
                self.state = WorkflowRunState::Running;
                self.push_event(occurred_at, WorkflowRunEventPayload::WorkflowRunStartedEvent)
            }
            WorkflowRunState::Succeeded
            | WorkflowRunState::Failed
            | WorkflowRunState::Cancelled => {
                Err(WorkflowDomainError::WorkflowTerminalStateImmutable)
            }
            WorkflowRunState::Running => Err(WorkflowDomainError::WorkflowIllegalRunTransition),
        }
    }

    /// Starts one pending node only while the Run is running.
    pub fn start_node(
        &mut self,
        execution_id: WorkflowNodeExecutionId,
        occurred_at: WorkflowRunTime,
    ) -> Result<(), WorkflowDomainError> {
        self.ensure_event_can_be_recorded(occurred_at)?;
        if self.state != WorkflowRunState::Running {
            return Err(WorkflowDomainError::WorkflowIllegalNodeExecutionTransition);
        }
        let node = self.node_mut(execution_id)?;
        if node.state != WorkflowNodeExecutionState::Pending {
            return Err(WorkflowDomainError::WorkflowIllegalNodeExecutionTransition);
        }
        node.state = WorkflowNodeExecutionState::Running;
        node.started_at = Some(occurred_at);
        self.push_event(
            occurred_at,
            WorkflowRunEventPayload::WorkflowNodeStartedEvent { node_execution_id: execution_id },
        )
    }

    /// Advances monotonic integer basis-point progress for one running node.
    pub fn progress_node(
        &mut self,
        execution_id: WorkflowNodeExecutionId,
        progress_basis_points: u16,
        occurred_at: WorkflowRunTime,
    ) -> Result<(), WorkflowDomainError> {
        self.ensure_event_can_be_recorded(occurred_at)?;
        if progress_basis_points > 10_000 {
            return Err(WorkflowDomainError::WorkflowProgressOutOfRange);
        }
        let node = self.node_mut(execution_id)?;
        if node.state != WorkflowNodeExecutionState::Running {
            return Err(WorkflowDomainError::WorkflowIllegalNodeExecutionTransition);
        }
        if node.progress_basis_points.is_some_and(|current| progress_basis_points < current) {
            return Err(WorkflowDomainError::WorkflowProgressRegression);
        }
        node.progress_basis_points = Some(progress_basis_points);
        self.push_event(
            occurred_at,
            WorkflowRunEventPayload::WorkflowNodeProgressedEvent {
                node_execution_id: execution_id,
                progress_basis_points,
            },
        )
    }

    /// Idempotently commits a running node's durable external-completion handoff.
    pub fn wait_node_for_external_completion(
        &mut self,
        execution_id: WorkflowNodeExecutionId,
        occurred_at: WorkflowRunTime,
    ) -> Result<(), WorkflowDomainError> {
        if matches!(
            self.state,
            WorkflowRunState::Succeeded | WorkflowRunState::Failed | WorkflowRunState::Cancelled
        ) {
            return Err(WorkflowDomainError::WorkflowTerminalStateImmutable);
        }
        let state = self
            .node_executions
            .iter()
            .find(|node| node.execution_id == execution_id)
            .map(|node| node.state)
            .ok_or(WorkflowDomainError::WorkflowIllegalNodeExecutionTransition)?;
        if state == WorkflowNodeExecutionState::WaitingForExternalCompletion {
            return Ok(());
        }
        self.ensure_event_can_be_recorded(occurred_at)?;
        if self.state != WorkflowRunState::Running || state != WorkflowNodeExecutionState::Running {
            return Err(WorkflowDomainError::WorkflowIllegalNodeExecutionTransition);
        }
        let node = self.node_mut(execution_id)?;
        node.state = WorkflowNodeExecutionState::WaitingForExternalCompletion;
        node.progress_basis_points = None;
        self.push_event(
            occurred_at,
            WorkflowRunEventPayload::WorkflowNodeWaitingForExternalCompletionEvent {
                node_execution_id: execution_id,
            },
        )
    }

    /// Commits one running node's already contract-complete output set.
    pub fn succeed_node(
        &mut self,
        execution_id: WorkflowNodeExecutionId,
        outputs: WorkflowNodeOutputSet,
        occurred_at: WorkflowRunTime,
    ) -> Result<(), WorkflowDomainError> {
        self.ensure_event_can_be_recorded(occurred_at)?;
        let node = self.node_mut(execution_id)?;
        if !matches!(
            node.state,
            WorkflowNodeExecutionState::Running
                | WorkflowNodeExecutionState::WaitingForExternalCompletion
        ) {
            return Err(WorkflowDomainError::WorkflowIllegalNodeExecutionTransition);
        }
        node.state = WorkflowNodeExecutionState::Succeeded;
        node.progress_basis_points = None;
        node.finished_at = Some(occurred_at);
        node.outputs = Some(outputs.clone());
        self.push_event(
            occurred_at,
            WorkflowRunEventPayload::WorkflowNodeSucceededEvent {
                node_execution_id: execution_id,
                outputs,
            },
        )
    }

    /// Commits one running node's structured failure.
    pub fn fail_node(
        &mut self,
        execution_id: WorkflowNodeExecutionId,
        failure: WorkflowNodeExecutionFailure,
        occurred_at: WorkflowRunTime,
    ) -> Result<(), WorkflowDomainError> {
        self.ensure_event_can_be_recorded(occurred_at)?;
        let node = self.node_mut(execution_id)?;
        if !matches!(
            node.state,
            WorkflowNodeExecutionState::Running
                | WorkflowNodeExecutionState::WaitingForExternalCompletion
        ) {
            return Err(WorkflowDomainError::WorkflowIllegalNodeExecutionTransition);
        }
        node.state = WorkflowNodeExecutionState::Failed;
        node.progress_basis_points = None;
        node.finished_at = Some(occurred_at);
        node.failure = Some(failure.clone());
        self.push_event(
            occurred_at,
            WorkflowRunEventPayload::WorkflowNodeFailedEvent {
                node_execution_id: execution_id,
                failure,
            },
        )
    }

    /// Blocks one pending node on a non-empty sorted set of failed upstream nodes.
    pub fn block_node(
        &mut self,
        execution_id: WorkflowNodeExecutionId,
        mut upstream_node_ids: Vec<WorkflowNodeId>,
        occurred_at: WorkflowRunTime,
    ) -> Result<(), WorkflowDomainError> {
        self.ensure_event_can_be_recorded(occurred_at)?;
        upstream_node_ids.sort_unstable();
        upstream_node_ids.dedup();
        if upstream_node_ids.is_empty() {
            return Err(WorkflowDomainError::InvalidWorkflowRunValue);
        }
        let node = self.node_mut(execution_id)?;
        if node.state != WorkflowNodeExecutionState::Pending {
            return Err(WorkflowDomainError::WorkflowIllegalNodeExecutionTransition);
        }
        let reason = WorkflowNodeExecutionBlockReason::UpstreamNodeFailed {
            sorted_upstream_node_ids: upstream_node_ids,
        };
        node.state = WorkflowNodeExecutionState::Blocked;
        node.finished_at = Some(occurred_at);
        node.block_reason = Some(reason.clone());
        self.push_event(
            occurred_at,
            WorkflowRunEventPayload::WorkflowNodeBlockedEvent {
                node_execution_id: execution_id,
                reason,
            },
        )
    }

    /// Finishes coordination after all nodes are terminal.
    pub fn finish(&mut self, occurred_at: WorkflowRunTime) -> Result<(), WorkflowDomainError> {
        self.ensure_event_can_be_recorded(occurred_at)?;
        if self.state != WorkflowRunState::Running {
            return self.run_transition_error();
        }
        if self.node_executions.iter().any(|node| {
            matches!(
                node.state,
                WorkflowNodeExecutionState::Pending
                    | WorkflowNodeExecutionState::Running
                    | WorkflowNodeExecutionState::WaitingForExternalCompletion
            )
        }) {
            return Err(WorkflowDomainError::WorkflowIllegalRunTransition);
        }
        let mut failed = self
            .node_executions
            .iter()
            .filter_map(|node| {
                (node.state == WorkflowNodeExecutionState::Failed).then_some(node.node_id)
            })
            .collect::<Vec<_>>();
        failed.sort_unstable();
        if failed.is_empty() {
            if self
                .node_executions
                .iter()
                .all(|node| node.state == WorkflowNodeExecutionState::Succeeded)
            {
                self.state = WorkflowRunState::Succeeded;
                self.push_event(occurred_at, WorkflowRunEventPayload::WorkflowRunSucceededEvent)
            } else {
                Err(WorkflowDomainError::WorkflowIllegalRunTransition)
            }
        } else {
            let failure =
                WorkflowRunFailure::NodeExecutionFailed { sorted_failed_node_ids: failed };
            self.state = WorkflowRunState::Failed;
            self.failure = Some(failure.clone());
            self.push_event(
                occurred_at,
                WorkflowRunEventPayload::WorkflowRunFailedEvent { failure },
            )
        }
    }

    /// Marks a queued or running Run failed after process restart.
    pub fn interrupt_by_restart(
        &mut self,
        occurred_at: WorkflowRunTime,
    ) -> Result<(), WorkflowDomainError> {
        self.ensure_event_can_be_recorded(occurred_at)?;
        if matches!(
            self.state,
            WorkflowRunState::Succeeded | WorkflowRunState::Failed | WorkflowRunState::Cancelled
        ) {
            return Err(WorkflowDomainError::WorkflowTerminalStateImmutable);
        }
        let failure = WorkflowRunFailure::InterruptedByRestart;
        self.state = WorkflowRunState::Failed;
        self.failure = Some(failure.clone());
        self.push_event(occurred_at, WorkflowRunEventPayload::WorkflowRunFailedEvent { failure })
    }

    /// Idempotently cancels a queued or running Run and all non-terminal nodes.
    pub fn cancel(&mut self, occurred_at: WorkflowRunTime) -> Result<(), WorkflowDomainError> {
        if self.state != WorkflowRunState::Cancelled {
            self.ensure_event_can_be_recorded(occurred_at)?;
        }
        match self.state {
            WorkflowRunState::Cancelled => return Ok(()),
            WorkflowRunState::Succeeded | WorkflowRunState::Failed => {
                return Err(WorkflowDomainError::WorkflowTerminalStateImmutable);
            }
            WorkflowRunState::Queued | WorkflowRunState::Running => {}
        }
        let cancelled = self
            .node_executions
            .iter_mut()
            .filter(|node| {
                matches!(
                    node.state,
                    WorkflowNodeExecutionState::Pending
                        | WorkflowNodeExecutionState::Running
                        | WorkflowNodeExecutionState::WaitingForExternalCompletion
                )
            })
            .map(|node| {
                node.state = WorkflowNodeExecutionState::Cancelled;
                node.progress_basis_points = None;
                node.finished_at = Some(occurred_at);
                node.execution_id
            })
            .collect::<Vec<_>>();
        for execution_id in cancelled {
            self.push_event(
                occurred_at,
                WorkflowRunEventPayload::WorkflowNodeCancelledEvent {
                    node_execution_id: execution_id,
                },
            )?;
        }
        self.state = WorkflowRunState::Cancelled;
        self.push_event(occurred_at, WorkflowRunEventPayload::WorkflowRunCancelledEvent)
    }

    fn node_mut(
        &mut self,
        execution_id: WorkflowNodeExecutionId,
    ) -> Result<&mut WorkflowNodeExecutionEntity, WorkflowDomainError> {
        self.node_executions
            .iter_mut()
            .find(|node| node.execution_id == execution_id)
            .ok_or(WorkflowDomainError::WorkflowIllegalNodeExecutionTransition)
    }

    fn run_transition_error(&self) -> Result<(), WorkflowDomainError> {
        if matches!(
            self.state,
            WorkflowRunState::Succeeded | WorkflowRunState::Failed | WorkflowRunState::Cancelled
        ) {
            Err(WorkflowDomainError::WorkflowTerminalStateImmutable)
        } else {
            Err(WorkflowDomainError::WorkflowIllegalRunTransition)
        }
    }

    fn push_event(
        &mut self,
        occurred_at: WorkflowRunTime,
        payload: WorkflowRunEventPayload,
    ) -> Result<(), WorkflowDomainError> {
        self.ensure_event_can_be_recorded(occurred_at)?;
        let sequence = self
            .events
            .last()
            .ok_or(WorkflowDomainError::InvalidWorkflowRunValue)?
            .sequence
            .next()?;
        self.events.push(WorkflowRunEvent { run_id: self.run_id, sequence, occurred_at, payload });
        self.updated_at = occurred_at;
        Ok(())
    }

    fn ensure_event_can_be_recorded(
        &self,
        occurred_at: WorkflowRunTime,
    ) -> Result<(), WorkflowDomainError> {
        if occurred_at < self.updated_at {
            return Err(WorkflowDomainError::InvalidWorkflowRunValue);
        }
        self.events
            .last()
            .ok_or(WorkflowDomainError::InvalidWorkflowRunValue)?
            .sequence
            .next()
            .map(|_| ())
    }
}
