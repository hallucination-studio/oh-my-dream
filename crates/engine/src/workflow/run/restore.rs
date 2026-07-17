use std::collections::BTreeSet;

use projects::project::domain::ProjectId;

use crate::node_capability::{WorkflowNodeExecutionId, WorkflowNodeOutputSet, WorkflowRunId};
use crate::workflow_graph::WorkflowNodeId;

use super::{
    WorkflowDomainError, WorkflowExecutionPlan, WorkflowNodeExecutionBlockReason,
    WorkflowNodeExecutionEntity, WorkflowNodeExecutionFailure, WorkflowNodeExecutionState,
    WorkflowRunAggregate, WorkflowRunEvent, WorkflowRunFailure, WorkflowRunState, WorkflowRunTime,
};

/// Persisted fields needed to reconstruct one node execution without replaying events.
pub struct WorkflowNodeExecutionRestoreData {
    /// Frozen source node.
    pub node_id: WorkflowNodeId,
    /// Run-local execution identity.
    pub execution_id: WorkflowNodeExecutionId,
    /// Persisted state.
    pub state: WorkflowNodeExecutionState,
    /// Optional in-flight progress.
    pub progress_basis_points: Option<u16>,
    /// Optional start time.
    pub started_at: Option<WorkflowRunTime>,
    /// Optional terminal time.
    pub finished_at: Option<WorkflowRunTime>,
    /// Complete successful outputs.
    pub outputs: Option<WorkflowNodeOutputSet>,
    /// Structured execution failure.
    pub failure: Option<WorkflowNodeExecutionFailure>,
    /// Structured upstream block reason.
    pub block_reason: Option<WorkflowNodeExecutionBlockReason>,
}

/// Persisted fields needed to reconstruct one Workflow Run aggregate.
pub struct WorkflowRunRestoreData {
    /// Run identity.
    pub run_id: WorkflowRunId,
    /// Owning Project.
    pub project_id: ProjectId,
    /// Immutable frozen execution plan.
    pub plan: WorkflowExecutionPlan,
    /// Persisted Run state.
    pub state: WorkflowRunState,
    /// Node executions in plan order.
    pub node_executions: Vec<WorkflowNodeExecutionRestoreData>,
    /// Durable events in sequence order.
    pub events: Vec<WorkflowRunEvent>,
    /// Admission time.
    pub created_at: WorkflowRunTime,
    /// Latest transition time.
    pub updated_at: WorkflowRunTime,
    /// Closed failure present exactly for failed Runs.
    pub failure: Option<WorkflowRunFailure>,
}

impl WorkflowRunAggregate {
    /// Reconstructs only a state/outcome/event shape permitted by the frozen domain model.
    pub fn try_restore(data: WorkflowRunRestoreData) -> Result<Self, WorkflowDomainError> {
        validate_run_shape(&data)?;
        let node_executions = data
            .node_executions
            .into_iter()
            .map(|node| WorkflowNodeExecutionEntity {
                node_id: node.node_id,
                execution_id: node.execution_id,
                state: node.state,
                progress_basis_points: node.progress_basis_points,
                started_at: node.started_at,
                finished_at: node.finished_at,
                outputs: node.outputs,
                failure: node.failure,
                block_reason: node.block_reason,
            })
            .collect();
        Ok(Self {
            run_id: data.run_id,
            project_id: data.project_id,
            plan: data.plan,
            state: data.state,
            node_executions,
            events: data.events,
            created_at: data.created_at,
            updated_at: data.updated_at,
            failure: data.failure,
        })
    }
}

fn validate_run_shape(data: &WorkflowRunRestoreData) -> Result<(), WorkflowDomainError> {
    if data.updated_at < data.created_at || !run_failure_matches(data.state, &data.failure) {
        return Err(WorkflowDomainError::InvalidWorkflowRunValue);
    }
    let planned = data.plan.nodes();
    if planned.len() != data.node_executions.len()
        || planned.iter().zip(&data.node_executions).any(|(planned, restored)| {
            planned.node_id != restored.node_id
                || planned.node_execution_id != restored.execution_id
                || !node_shape_is_valid(restored)
        })
    {
        return Err(WorkflowDomainError::InvalidWorkflowRunValue);
    }
    let execution_ids =
        data.node_executions.iter().map(|node| node.execution_id).collect::<BTreeSet<_>>();
    if execution_ids.len() != data.node_executions.len()
        || !run_and_nodes_match(data)
        || !events_are_valid(data)
    {
        return Err(WorkflowDomainError::InvalidWorkflowRunValue);
    }
    Ok(())
}

fn run_and_nodes_match(data: &WorkflowRunRestoreData) -> bool {
    let no_active = data.node_executions.iter().all(|node| {
        !matches!(
            node.state,
            WorkflowNodeExecutionState::Pending
                | WorkflowNodeExecutionState::Running
                | WorkflowNodeExecutionState::WaitingForExternalCompletion
        )
    });
    match (&data.state, &data.failure) {
        (WorkflowRunState::Queued, None) => data
            .node_executions
            .iter()
            .all(|node| node.state == WorkflowNodeExecutionState::Pending),
        (WorkflowRunState::Running, None) => true,
        (WorkflowRunState::Succeeded, None) => data
            .node_executions
            .iter()
            .all(|node| node.state == WorkflowNodeExecutionState::Succeeded),
        (WorkflowRunState::Cancelled, None) => no_active,
        (
            WorkflowRunState::Failed,
            Some(WorkflowRunFailure::NodeExecutionFailed { sorted_failed_node_ids }),
        ) => {
            let mut actual = data
                .node_executions
                .iter()
                .filter_map(|node| {
                    (node.state == WorkflowNodeExecutionState::Failed).then_some(node.node_id)
                })
                .collect::<Vec<_>>();
            actual.sort_unstable();
            no_active
                && !actual.is_empty()
                && actual == *sorted_failed_node_ids
                && actual.windows(2).all(|pair| pair[0] < pair[1])
        }
        (WorkflowRunState::Failed, Some(WorkflowRunFailure::InterruptedByRestart)) => true,
        _ => false,
    }
}

fn run_failure_matches(state: WorkflowRunState, failure: &Option<WorkflowRunFailure>) -> bool {
    matches!((state, failure), (WorkflowRunState::Failed, Some(_)))
        || (!matches!(state, WorkflowRunState::Failed) && failure.is_none())
}

fn node_shape_is_valid(node: &WorkflowNodeExecutionRestoreData) -> bool {
    let no_outcome =
        node.outputs.is_none() && node.failure.is_none() && node.block_reason.is_none();
    match node.state {
        WorkflowNodeExecutionState::Pending => {
            no_outcome
                && node.started_at.is_none()
                && node.finished_at.is_none()
                && node.progress_basis_points.is_none()
        }
        WorkflowNodeExecutionState::Running => {
            no_outcome
                && node.started_at.is_some()
                && node.finished_at.is_none()
                && node.progress_basis_points.is_none_or(|value| value <= 10_000)
        }
        WorkflowNodeExecutionState::WaitingForExternalCompletion => {
            no_outcome
                && node.started_at.is_some()
                && node.finished_at.is_none()
                && node.progress_basis_points.is_none()
        }
        WorkflowNodeExecutionState::Succeeded => {
            node.outputs.is_some()
                && node.failure.is_none()
                && node.block_reason.is_none()
                && node.started_at.is_some()
                && node.finished_at.is_some()
                && node.progress_basis_points.is_none()
        }
        WorkflowNodeExecutionState::Failed => {
            node.outputs.is_none()
                && node.failure.is_some()
                && node.block_reason.is_none()
                && node.started_at.is_some()
                && node.finished_at.is_some()
                && node.progress_basis_points.is_none()
        }
        WorkflowNodeExecutionState::Blocked => {
            node.outputs.is_none()
                && node.failure.is_none()
                && block_reason_is_valid(&node.block_reason)
                && node.started_at.is_none()
                && node.finished_at.is_some()
                && node.progress_basis_points.is_none()
        }
        WorkflowNodeExecutionState::Cancelled => {
            no_outcome && node.finished_at.is_some() && node.progress_basis_points.is_none()
        }
    }
}

fn block_reason_is_valid(reason: &Option<WorkflowNodeExecutionBlockReason>) -> bool {
    let Some(WorkflowNodeExecutionBlockReason::UpstreamNodeFailed { sorted_upstream_node_ids }) =
        reason
    else {
        return false;
    };
    !sorted_upstream_node_ids.is_empty()
        && sorted_upstream_node_ids.windows(2).all(|pair| pair[0] < pair[1])
}

fn events_are_valid(data: &WorkflowRunRestoreData) -> bool {
    !data.events.is_empty()
        && data.events.iter().enumerate().all(|(index, event)| {
            event.run_id() == data.run_id
                && event.sequence().get()
                    == u64::try_from(index).ok().and_then(|value| value.checked_add(1)).unwrap_or(0)
                && event.occurred_at() >= data.created_at
                && event.occurred_at() <= data.updated_at
                && (index == 0 || event.occurred_at() >= data.events[index - 1].occurred_at())
        })
}
