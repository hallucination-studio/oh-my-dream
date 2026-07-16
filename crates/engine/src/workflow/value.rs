use crate::workflow_graph::WorkflowNodeId;

use super::WorkflowDomainError;

/// Whole-graph execution or one node plus all transitive ancestors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkflowRunScope {
    /// Execute every node in the frozen Workflow revision.
    WholeWorkflow,
    /// Execute the selected node and its transitive ancestors only.
    ThroughNode(WorkflowNodeId),
}

impl WorkflowRunScope {
    /// Returns the selected terminal node only for `ThroughNode`.
    #[must_use]
    pub const fn selected_node_id(self) -> Option<WorkflowNodeId> {
        match self {
            Self::WholeWorkflow => None,
            Self::ThroughNode(node_id) => Some(node_id),
        }
    }
}

/// Non-negative UTC millisecond timestamp for one durable Run transition.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct WorkflowRunTime(i64);

impl WorkflowRunTime {
    /// Restores a non-negative timestamp.
    pub const fn from_utc_milliseconds(value: i64) -> Result<Self, WorkflowDomainError> {
        if value < 0 { Err(WorkflowDomainError::InvalidWorkflowRunValue) } else { Ok(Self(value)) }
    }

    /// Returns UTC milliseconds.
    #[must_use]
    pub const fn as_utc_milliseconds(self) -> i64 {
        self.0
    }
}

/// Non-zero monotonic sequence of one Run's durable events.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct WorkflowRunEventSequence(u64);

impl WorkflowRunEventSequence {
    /// Restores a non-zero sequence.
    pub const fn new(value: u64) -> Result<Self, WorkflowDomainError> {
        if value == 0 { Err(WorkflowDomainError::InvalidWorkflowRunValue) } else { Ok(Self(value)) }
    }

    /// Returns the sequence number.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    pub(super) const fn next(self) -> Result<Self, WorkflowDomainError> {
        match self.0.checked_add(1) {
            Some(value) => Ok(Self(value)),
            None => Err(WorkflowDomainError::WorkflowRunEventSequenceOverflow),
        }
    }
}

/// Durable state of one Workflow Run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkflowRunState {
    /// Admitted and committed before external work begins.
    Queued,
    /// At least one execution cycle has started.
    Running,
    /// Every planned node completed successfully.
    Succeeded,
    /// One or more nodes failed or restart interrupted the Run.
    Failed,
    /// Cancellation was durably committed.
    Cancelled,
}

/// Durable state of one planned node execution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkflowNodeExecutionState {
    /// Waiting for dependencies and dispatch.
    Pending,
    /// Exact capability execution is active.
    Running,
    /// A complete output set was committed.
    Succeeded,
    /// A structured execution failure was committed.
    Failed,
    /// Cancellation was committed without an outcome.
    Cancelled,
    /// An upstream failure made execution impossible.
    Blocked,
}
