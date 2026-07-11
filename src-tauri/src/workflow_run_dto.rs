use crate::dto::{NodeProgressEventDto, RunOutputDto, RunWorkflowResultDto};
use crate::workflow_runs::{CancellationRequest, WorkflowRunEvent, WorkflowRunOutcome};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Ordered event delivered through one scoped Tauri channel.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum WorkflowRunEventDto {
    /// Confirms that the run is registered and cancellable.
    Started {
        /// Caller-provided run identity.
        run_id: String,
        /// Project execution slot owned by the run.
        project_id: String,
    },
    /// Reports one engine-owned node progress event.
    Progress {
        /// Caller-provided run identity.
        run_id: String,
        /// Node progress payload.
        node: NodeProgressEventDto,
    },
}

impl From<WorkflowRunEvent> for WorkflowRunEventDto {
    fn from(event: WorkflowRunEvent) -> Self {
        match event {
            WorkflowRunEvent::Started { run_id, project_id } => {
                Self::Started { run_id: run_id.as_str().to_owned(), project_id }
            }
            WorkflowRunEvent::Progress { run_id, node } => Self::Progress {
                run_id: run_id.as_str().to_owned(),
                node: NodeProgressEventDto::from(node),
            },
        }
    }
}

/// Authoritative terminal result returned by `start_workflow_run`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum WorkflowRunResultDto {
    /// The workflow committed all node outputs.
    Succeeded {
        /// Caller-provided run identity.
        run_id: String,
        /// Node id -> output name -> output value.
        outputs: BTreeMap<String, BTreeMap<String, RunOutputDto>>,
    },
    /// Cancellation reached an authoritative terminal state.
    Cancelled {
        /// Caller-provided run identity.
        run_id: String,
    },
    /// Execution failed for a reason other than cancellation.
    Failed {
        /// Caller-provided run identity.
        run_id: String,
        /// Actionable engine error without internal adapter details.
        reason: String,
    },
}

impl WorkflowRunResultDto {
    /// Translates the application outcome into its stable wire representation.
    #[must_use]
    pub fn from_outcome(run_id: &str, outcome: WorkflowRunOutcome) -> Self {
        match outcome {
            WorkflowRunOutcome::Succeeded(outputs) => Self::Succeeded {
                run_id: run_id.to_owned(),
                outputs: RunWorkflowResultDto::from_outputs(&outputs).outputs,
            },
            WorkflowRunOutcome::Cancelled => Self::Cancelled { run_id: run_id.to_owned() },
            WorkflowRunOutcome::Failed(source) => {
                Self::Failed { run_id: run_id.to_owned(), reason: source.to_string() }
            }
        }
    }
}

/// Result of the idempotent `cancel_workflow_run` command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum CancelWorkflowRunResultDto {
    /// An active run now carries the cancellation request.
    Requested {
        /// Caller-provided run identity.
        run_id: String,
    },
    /// No active run currently owns the identity.
    NotActive {
        /// Caller-provided run identity.
        run_id: String,
    },
}

impl CancelWorkflowRunResultDto {
    /// Translates an application cancellation lookup into its wire representation.
    #[must_use]
    pub fn from_request(run_id: &str, request: CancellationRequest) -> Self {
        match request {
            CancellationRequest::Requested => Self::Requested { run_id: run_id.to_owned() },
            CancellationRequest::NotActive => Self::NotActive { run_id: run_id.to_owned() },
        }
    }
}
