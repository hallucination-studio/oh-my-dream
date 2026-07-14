//! Mock execution and factual failure activation for the reviewed-repair MVP.

mod activation;

pub use activation::RepairActivation;

use crate::state::AppState;
use crate::workflow_authority::WorkflowAuthority;
use crate::workflow_run_dto::WorkflowRunResultDto;
use crate::workflow_runs::{
    RunId, WorkflowRunEvent, WorkflowRunEventError, WorkflowRunEventSink, WorkflowRuns,
};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use thiserror::Error;

/// Trusted identity of one user-approved Workflow action.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApprovedWorkflowAction {
    project_id: String,
    approval_scope_id: String,
    workflow_revision: u64,
}

impl ApprovedWorkflowAction {
    /// Creates the identity used to derive a separate mock Run.
    #[must_use]
    pub fn new(
        project_id: impl Into<String>,
        approval_scope_id: impl Into<String>,
        workflow_revision: u64,
    ) -> Self {
        Self {
            project_id: project_id.into(),
            approval_scope_id: approval_scope_id.into(),
            workflow_revision,
        }
    }
}

/// One terminal mock Run plus an optional factual failure activation.
pub struct AssistantRepairRun {
    pub run_id: String,
    pub outcome: WorkflowRunResultDto,
    pub activation: Option<RepairActivation>,
}

/// Executes an approved canonical Workflow through the existing local runner.
pub struct AssistantRepairService {
    authority: Arc<WorkflowAuthority>,
    runs: Arc<WorkflowRuns>,
}

impl AssistantRepairService {
    /// Creates the capability from application composition state.
    #[must_use]
    pub fn from_state(state: &AppState) -> Self {
        Self {
            authority: Arc::clone(&state.workflow_authority),
            runs: Arc::clone(&state.workflow_runs),
        }
    }

    /// Derives a stable, bounded Run identity from the approved action identity.
    #[must_use]
    pub fn run_id(action: &ApprovedWorkflowAction) -> String {
        let subject = format!(
            "{}\n{}\n{}",
            action.project_id, action.approval_scope_id, action.workflow_revision
        );
        let digest = format!("{:x}", Sha256::digest(subject.as_bytes()));
        format!("assistant-run-{}", &digest[..24])
    }

    /// Runs the exact approved Workflow and returns facts, never repair instructions.
    pub fn execute(
        &self,
        action: &ApprovedWorkflowAction,
    ) -> Result<AssistantRepairRun, AssistantRepairError> {
        self.execute_with_events(action, &mut IgnoreEvents)
    }

    /// Runs the approved Workflow while forwarding existing run events.
    pub fn execute_with_events(
        &self,
        action: &ApprovedWorkflowAction,
        events: &mut dyn WorkflowRunEventSink,
    ) -> Result<AssistantRepairRun, AssistantRepairError> {
        validate_action(action)?;
        let head = self
            .authority
            .load_head(&action.project_id)
            .map_err(|error| AssistantRepairError::Workflow(error.to_string()))?
            .ok_or(AssistantRepairError::WorkflowMissing)?;
        if head.revision != action.workflow_revision {
            return Err(AssistantRepairError::RevisionConflict {
                expected: action.workflow_revision,
                actual: head.revision,
            });
        }
        let run_id = Self::run_id(action);
        let parsed =
            RunId::parse(&run_id).map_err(|error| AssistantRepairError::Run(error.to_string()))?;
        let outcome = self
            .runs
            .run(parsed, head.workflow, events)
            .map_err(|error| AssistantRepairError::Run(error.to_string()))?;
        let dto = WorkflowRunResultDto::from_outcome(&run_id, outcome);
        let activation = match &dto {
            WorkflowRunResultDto::Failed { reason, .. } => Some(RepairActivation::failed(
                &action.project_id,
                &run_id,
                action.workflow_revision,
                reason.clone(),
            )),
            WorkflowRunResultDto::Succeeded { .. } | WorkflowRunResultDto::Cancelled { .. } => None,
        };
        Ok(AssistantRepairRun { run_id, outcome: dto, activation })
    }
}

/// Validation or execution failure before a terminal Workflow outcome exists.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum AssistantRepairError {
    #[error("approved action identifiers must not be empty")]
    EmptyIdentity,
    #[error("canonical Workflow is absent")]
    WorkflowMissing,
    #[error("canonical Workflow revision conflict: expected {expected}, actual {actual}")]
    RevisionConflict { expected: u64, actual: u64 },
    #[error("read canonical Workflow: {0}")]
    Workflow(String),
    #[error("run approved Workflow: {0}")]
    Run(String),
}

fn validate_action(action: &ApprovedWorkflowAction) -> Result<(), AssistantRepairError> {
    if action.project_id.trim().is_empty() || action.approval_scope_id.trim().is_empty() {
        return Err(AssistantRepairError::EmptyIdentity);
    }
    Ok(())
}

struct IgnoreEvents;

impl WorkflowRunEventSink for IgnoreEvents {
    fn send(&mut self, _event: WorkflowRunEvent) -> Result<(), WorkflowRunEventError> {
        Ok(())
    }
}
