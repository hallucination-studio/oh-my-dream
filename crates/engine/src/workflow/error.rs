use thiserror::Error;

/// Closed failures owned by Workflow Run domain values and transitions.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum WorkflowDomainError {
    /// A Run transition is outside the frozen state machine.
    #[error("workflow run transition is illegal")]
    WorkflowIllegalRunTransition,
    /// A node-execution transition is outside the frozen state machine.
    #[error("workflow node execution transition is illegal")]
    WorkflowIllegalNodeExecutionTransition,
    /// Progress is greater than 10,000 basis points.
    #[error("workflow progress is out of range")]
    WorkflowProgressOutOfRange,
    /// Progress moved backwards for one running node.
    #[error("workflow progress regressed")]
    WorkflowProgressRegression,
    /// A durable event sequence could not advance.
    #[error("workflow run event sequence overflowed")]
    WorkflowRunEventSequenceOverflow,
    /// A complete output set was required for success.
    #[error("workflow node output set is incomplete")]
    WorkflowIncompleteOutputSet,
    /// A terminal Run or node execution cannot be reopened.
    #[error("workflow terminal state is immutable")]
    WorkflowTerminalStateImmutable,
    /// A Run identity, time, plan, or restored shape is invalid.
    #[error("workflow run value is invalid")]
    InvalidWorkflowRunValue,
    /// A frozen plan is empty, duplicated, or not topologically closed.
    #[error("workflow execution plan is invalid")]
    InvalidWorkflowExecutionPlan,
}
