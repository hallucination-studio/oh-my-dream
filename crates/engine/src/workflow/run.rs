use std::collections::BTreeSet;

use projects::project::domain::ProjectId;

use crate::node_capability::{
    NodeCapabilityExecutionError, WorkflowNodeExecutionId, WorkflowNodeOutputSet, WorkflowRunId,
};
use crate::workflow_graph::{WorkflowId, WorkflowNodeId, WorkflowRevision};

use super::{
    WorkflowDomainError, WorkflowExecutionPlan, WorkflowNodeExecutionState,
    WorkflowRunEventSequence, WorkflowRunScope, WorkflowRunState, WorkflowRunTime,
};

mod restore;
mod transition;

pub use restore::*;

/// Closed durable Run and node lifecycle events.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkflowRunEventPayload {
    /// The Run and its first effect were durably admitted.
    WorkflowRunQueuedEvent,
    /// Run coordination started.
    WorkflowRunStartedEvent,
    /// One node began exact capability execution.
    WorkflowNodeStartedEvent {
        /// Started execution.
        node_execution_id: WorkflowNodeExecutionId,
    },
    /// One running node advanced integer basis-point progress.
    WorkflowNodeProgressedEvent {
        /// Progressing execution.
        node_execution_id: WorkflowNodeExecutionId,
        /// Monotonic progress from zero through 10,000.
        progress_basis_points: u16,
    },
    /// One node committed its complete output set.
    WorkflowNodeSucceededEvent {
        /// Successful execution.
        node_execution_id: WorkflowNodeExecutionId,
        /// Contract-complete outputs committed with the transition.
        outputs: WorkflowNodeOutputSet,
    },
    /// One node committed a structured capability failure.
    WorkflowNodeFailedEvent {
        /// Failed execution.
        node_execution_id: WorkflowNodeExecutionId,
        /// Safe structured failure.
        failure: WorkflowNodeExecutionFailure,
    },
    /// One pending node was blocked by failed upstream nodes.
    WorkflowNodeBlockedEvent {
        /// Blocked execution.
        node_execution_id: WorkflowNodeExecutionId,
        /// Safe structured block reason.
        reason: WorkflowNodeExecutionBlockReason,
    },
    /// One pending or running node was cancelled.
    WorkflowNodeCancelledEvent {
        /// Cancelled execution.
        node_execution_id: WorkflowNodeExecutionId,
    },
    /// Every planned node succeeded.
    WorkflowRunSucceededEvent,
    /// Coordination committed a closed Run failure.
    WorkflowRunFailedEvent {
        /// Closed Run failure.
        failure: WorkflowRunFailure,
    },
    /// The Run cancellation became durable.
    WorkflowRunCancelledEvent,
}

/// Closed reason why a Workflow Run failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkflowRunFailure {
    /// One or more nodes failed exact execution.
    NodeExecutionFailed {
        /// Non-empty sorted source node identities.
        sorted_failed_node_ids: Vec<WorkflowNodeId>,
    },
    /// A previously running process ended before completion.
    InterruptedByRestart,
}

/// One node's safe structured execution failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowNodeExecutionFailure {
    /// Capability-owned structured execution error.
    pub capability_error: NodeCapabilityExecutionError,
}

/// Closed reason why one pending node cannot execute.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkflowNodeExecutionBlockReason {
    /// One or more upstream nodes failed.
    UpstreamNodeFailed {
        /// Non-empty sorted upstream node identities.
        sorted_upstream_node_ids: Vec<WorkflowNodeId>,
    },
}

/// One durable event ordered inside a Workflow Run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowRunEvent {
    run_id: WorkflowRunId,
    sequence: WorkflowRunEventSequence,
    occurred_at: WorkflowRunTime,
    payload: WorkflowRunEventPayload,
}

impl WorkflowRunEvent {
    /// Restores one event after validating its scalar values; aggregate restore validates ordering.
    #[must_use]
    pub const fn restore(
        run_id: WorkflowRunId,
        sequence: WorkflowRunEventSequence,
        occurred_at: WorkflowRunTime,
        payload: WorkflowRunEventPayload,
    ) -> Self {
        Self { run_id, sequence, occurred_at, payload }
    }
    /// Returns the owning Run.
    #[must_use]
    pub const fn run_id(&self) -> WorkflowRunId {
        self.run_id
    }
    /// Returns the non-zero monotonic sequence.
    #[must_use]
    pub const fn sequence(&self) -> WorkflowRunEventSequence {
        self.sequence
    }
    /// Returns the event timestamp.
    #[must_use]
    pub const fn occurred_at(&self) -> WorkflowRunTime {
        self.occurred_at
    }
    /// Returns the closed typed event payload.
    #[must_use]
    pub const fn payload(&self) -> &WorkflowRunEventPayload {
        &self.payload
    }
}

/// One planned node's durable execution state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowNodeExecutionEntity {
    node_id: WorkflowNodeId,
    execution_id: WorkflowNodeExecutionId,
    state: WorkflowNodeExecutionState,
    progress_basis_points: Option<u16>,
    started_at: Option<WorkflowRunTime>,
    finished_at: Option<WorkflowRunTime>,
    outputs: Option<WorkflowNodeOutputSet>,
    failure: Option<WorkflowNodeExecutionFailure>,
    block_reason: Option<WorkflowNodeExecutionBlockReason>,
}

impl WorkflowNodeExecutionEntity {
    /// Returns the frozen Workflow node identity.
    #[must_use]
    pub const fn node_id(&self) -> WorkflowNodeId {
        self.node_id
    }
    /// Returns the provider-idempotency execution identity.
    #[must_use]
    pub const fn execution_id(&self) -> WorkflowNodeExecutionId {
        self.execution_id
    }
    /// Returns the closed node state.
    #[must_use]
    pub const fn state(&self) -> WorkflowNodeExecutionState {
        self.state
    }
    /// Returns progress only while running.
    #[must_use]
    pub const fn progress_basis_points(&self) -> Option<u16> {
        self.progress_basis_points
    }
    /// Returns complete outputs only after success.
    #[must_use]
    pub const fn outputs(&self) -> Option<&WorkflowNodeOutputSet> {
        self.outputs.as_ref()
    }
    /// Returns a structured failure only after failure.
    #[must_use]
    pub const fn failure(&self) -> Option<&WorkflowNodeExecutionFailure> {
        self.failure.as_ref()
    }
    /// Returns a block reason only after blocking.
    #[must_use]
    pub const fn block_reason(&self) -> Option<&WorkflowNodeExecutionBlockReason> {
        self.block_reason.as_ref()
    }
    /// Returns when execution began, when applicable.
    #[must_use]
    pub const fn started_at(&self) -> Option<WorkflowRunTime> {
        self.started_at
    }
    /// Returns when a terminal node transition occurred.
    #[must_use]
    pub const fn finished_at(&self) -> Option<WorkflowRunTime> {
        self.finished_at
    }
}

/// Authoritative aggregate for one durable execution of one frozen Workflow revision.
#[derive(Clone, Debug)]
pub struct WorkflowRunAggregate {
    run_id: WorkflowRunId,
    project_id: ProjectId,
    plan: WorkflowExecutionPlan,
    state: WorkflowRunState,
    node_executions: Vec<WorkflowNodeExecutionEntity>,
    events: Vec<WorkflowRunEvent>,
    created_at: WorkflowRunTime,
    updated_at: WorkflowRunTime,
    failure: Option<WorkflowRunFailure>,
}

impl WorkflowRunAggregate {
    /// Admits a queued Run with unique planned node and execution identities and its first event.
    pub fn try_new_queued(
        run_id: WorkflowRunId,
        project_id: ProjectId,
        plan: WorkflowExecutionPlan,
        created_at: WorkflowRunTime,
    ) -> Result<Self, WorkflowDomainError> {
        let execution_ids =
            plan.nodes().iter().map(|node| node.node_execution_id).collect::<BTreeSet<_>>();
        if execution_ids.len() != plan.nodes().len() {
            return Err(WorkflowDomainError::InvalidWorkflowRunValue);
        }
        let node_executions = plan
            .nodes()
            .iter()
            .map(|node| WorkflowNodeExecutionEntity {
                node_id: node.node_id,
                execution_id: node.node_execution_id,
                state: WorkflowNodeExecutionState::Pending,
                progress_basis_points: None,
                started_at: None,
                finished_at: None,
                outputs: None,
                failure: None,
                block_reason: None,
            })
            .collect();
        Ok(Self {
            run_id,
            project_id,
            plan,
            state: WorkflowRunState::Queued,
            node_executions,
            events: vec![WorkflowRunEvent {
                run_id,
                sequence: WorkflowRunEventSequence::new(1)?,
                occurred_at: created_at,
                payload: WorkflowRunEventPayload::WorkflowRunQueuedEvent,
            }],
            created_at,
            updated_at: created_at,
            failure: None,
        })
    }

    /// Returns the Run state.
    #[must_use]
    pub const fn state(&self) -> WorkflowRunState {
        self.state
    }
    /// Returns the Run identity.
    #[must_use]
    pub const fn run_id(&self) -> WorkflowRunId {
        self.run_id
    }
    /// Returns the owning Project.
    #[must_use]
    pub const fn project_id(&self) -> ProjectId {
        self.project_id
    }
    /// Returns the frozen source Workflow.
    #[must_use]
    pub const fn workflow_id(&self) -> WorkflowId {
        self.plan.workflow_id()
    }
    /// Returns the frozen source revision.
    #[must_use]
    pub const fn workflow_revision(&self) -> WorkflowRevision {
        self.plan.workflow_revision()
    }
    /// Returns the admitted scope.
    #[must_use]
    pub const fn scope(&self) -> WorkflowRunScope {
        self.plan.scope()
    }
    /// Returns the immutable execution plan admitted with this Run.
    #[must_use]
    pub const fn plan(&self) -> &WorkflowExecutionPlan {
        &self.plan
    }
    /// Returns the admission timestamp.
    #[must_use]
    pub const fn created_at(&self) -> WorkflowRunTime {
        self.created_at
    }
    /// Returns the latest durable transition timestamp.
    #[must_use]
    pub const fn updated_at(&self) -> WorkflowRunTime {
        self.updated_at
    }
    /// Returns node executions in frozen plan order.
    #[must_use]
    pub fn node_executions(&self) -> &[WorkflowNodeExecutionEntity] {
        &self.node_executions
    }
    /// Returns durable events in sequence order.
    #[must_use]
    pub fn events(&self) -> &[WorkflowRunEvent] {
        &self.events
    }
    /// Returns the terminal failure only for a failed Run.
    #[must_use]
    pub const fn failure(&self) -> Option<&WorkflowRunFailure> {
        self.failure.as_ref()
    }
}
