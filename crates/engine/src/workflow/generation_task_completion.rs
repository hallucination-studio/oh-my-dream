//! Canonical completion of one waiting Workflow node from a terminal Generation Task.

use std::collections::BTreeMap;
use std::sync::Arc;

use projects::project::domain::ProjectId;
use uuid::{Uuid, Variant, Version};

use crate::node_capability::{
    NodeCapabilityContractRef, WorkflowManagedAudioRef, WorkflowManagedImageRef,
    WorkflowManagedVideoRef, WorkflowNodeCapabilityRegistry, WorkflowNodeExecutionId,
    WorkflowNodeOutputSet, WorkflowRunId, WorkflowRuntimeValue, WorkflowTextValue,
};
use crate::workflow_graph::{WorkflowId, WorkflowNodeId, WorkflowRevision};

use super::{
    WorkflowApplicationError, WorkflowClockInterface, WorkflowExecuteRunEffect,
    WorkflowGenerationTaskCompletionCommit, WorkflowNodeExecutionFailure,
    WorkflowNodeExecutionState, WorkflowRunLoadKey, WorkflowRunRepositoryInterface,
};

/// Idempotency identity of one terminal Generation Task notification.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct WorkflowGenerationTaskCompletionId(Uuid);

impl WorkflowGenerationTaskCompletionId {
    /// Restores only an RFC 9562 UUIDv4 Task identity.
    pub fn from_uuid(value: Uuid) -> Result<Self, WorkflowApplicationError> {
        if value.get_version() != Some(Version::Random) || value.get_variant() != Variant::RFC4122 {
            return Err(WorkflowApplicationError::WorkflowGenerationTaskCompletionConflict);
        }
        Ok(Self(value))
    }
    /// Returns the UUID without selecting a wire encoding.
    #[must_use]
    pub const fn as_uuid(self) -> Uuid {
        self.0
    }
}

/// Exact Workflow coordinates retained by one Generation Task.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowGenerationTaskOrigin {
    /// Owning Project.
    pub project_id: ProjectId,
    /// Frozen Workflow identity.
    pub workflow_id: WorkflowId,
    /// Frozen Workflow revision.
    pub workflow_revision: WorkflowRevision,
    /// Exact Workflow Run.
    pub workflow_run_id: WorkflowRunId,
    /// Exact Workflow node.
    pub workflow_node_id: WorkflowNodeId,
    /// Exact planned node execution.
    pub node_execution_id: WorkflowNodeExecutionId,
    /// Exact capability contract used by the frozen plan.
    pub capability_contract_ref: NodeCapabilityContractRef,
}

/// Provider-independent terminal Generation Task failure applied to Workflow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkflowGenerationTaskFailure {
    /// Semantic request rejection.
    InvalidRequest,
    /// Provider authentication failure.
    Authentication,
    /// Provider authorization failure.
    PermissionDenied,
    /// Content policy rejection.
    ContentPolicy,
    /// Provider rate limiting.
    RateLimited,
    /// Provider unavailable.
    ProviderUnavailable,
    /// Provider deadline elapsed.
    Timeout,
    /// Provider terminal rejection.
    ProviderRejected,
    /// Provider response was invalid.
    InvalidProviderResponse,
    /// Submission acceptance was ambiguous.
    AmbiguousSubmission,
    /// Exact input Asset became unavailable.
    InputAssetUnavailable,
    /// Generated output Asset could not be finalized.
    OutputAssetImport,
    /// Internal Task invariant or adapter failure.
    Internal,
    /// Provider-originated cancellation was terminal.
    GenerationTaskCancelled,
}

/// Closed Workflow runtime value produced by a terminal Generation Task.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkflowGenerationTaskCompletionValue {
    /// Inline generated Text.
    Text(WorkflowTextValue),
    /// Available managed Image.
    Image(WorkflowManagedImageRef),
    /// Available managed Video.
    Video(WorkflowManagedVideoRef),
    /// Available managed Audio.
    Audio(WorkflowManagedAudioRef),
}

impl WorkflowGenerationTaskCompletionValue {
    fn into_runtime_value(self) -> WorkflowRuntimeValue {
        match self {
            Self::Text(value) => WorkflowRuntimeValue::Text(value),
            Self::Image(value) => WorkflowRuntimeValue::Image(value),
            Self::Video(value) => WorkflowRuntimeValue::Video(value),
            Self::Audio(value) => WorkflowRuntimeValue::Audio(value),
        }
    }
}

/// Closed terminal outcome delivered by a Generation Task.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkflowGenerationTaskCompletionOutcome {
    /// One primary value completed successfully.
    Succeeded(WorkflowGenerationTaskCompletionValue),
    /// Task failed with one safe structured category.
    Failed(WorkflowGenerationTaskFailure),
}

/// Complete exact command for one terminal Task notification.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowCompleteGenerationTaskCommand {
    /// Stable Task notification identity.
    pub completion_id: WorkflowGenerationTaskCompletionId,
    /// Frozen exact Workflow origin.
    pub origin: WorkflowGenerationTaskOrigin,
    /// Terminal provider-independent outcome.
    pub outcome: WorkflowGenerationTaskCompletionOutcome,
}

/// Idempotent canonical Workflow completion result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkflowCompleteGenerationTaskOutcome {
    /// Terminal node state was committed now.
    Applied,
    /// The exact equivalent outcome was already committed.
    AlreadyApplied,
    /// The origin is terminal and cannot be reopened.
    OriginTerminal,
}

/// Applies one terminal Generation Task outcome to its exact waiting node.
pub struct WorkflowCompleteGenerationTaskUseCase<R, C> {
    repository: Arc<R>,
    clock: Arc<C>,
    capabilities: Arc<WorkflowNodeCapabilityRegistry>,
}

impl<R, C> WorkflowCompleteGenerationTaskUseCase<R, C>
where
    R: WorkflowRunRepositoryInterface,
    C: WorkflowClockInterface,
{
    /// Wires the canonical Run repository, clock, and frozen capability registry.
    #[must_use]
    pub fn new(
        repository: Arc<R>,
        clock: Arc<C>,
        capabilities: Arc<WorkflowNodeCapabilityRegistry>,
    ) -> Self {
        Self { repository, clock, capabilities }
    }

    /// Applies, idempotently observes, or rejects one exact terminal notification.
    pub async fn complete_generation_task(
        &self,
        command: WorkflowCompleteGenerationTaskCommand,
    ) -> Result<WorkflowCompleteGenerationTaskOutcome, WorkflowApplicationError> {
        let mut run = self
            .repository
            .load_workflow_run(WorkflowRunLoadKey::Run(command.origin.workflow_run_id))
            .await?
            .ok_or(WorkflowApplicationError::WorkflowRunNotFound)?;
        let node_index = validate_origin(&run, &command.origin)?;
        let desired = desired_node_outcome(&self.capabilities, &command)?;
        let node = &run.node_executions()[node_index];
        if node_outcome_matches(node, &desired) {
            return Ok(WorkflowCompleteGenerationTaskOutcome::AlreadyApplied);
        }
        if node.state() != WorkflowNodeExecutionState::WaitingForExternalCompletion {
            return if node_is_terminal(node.state()) || run_is_terminal(run.state()) {
                Ok(WorkflowCompleteGenerationTaskOutcome::OriginTerminal)
            } else {
                Err(WorkflowApplicationError::WorkflowGenerationTaskCompletionConflict)
            };
        }
        let previous_event_count = run.events().len();
        let occurred_at = self.clock.current_workflow_time()?;
        match desired {
            DesiredNodeOutcome::Succeeded(outputs) => {
                run.succeed_node(command.origin.node_execution_id, outputs, occurred_at)?
            }
            DesiredNodeOutcome::Failed(failure) => run.fail_node(
                command.origin.node_execution_id,
                WorkflowNodeExecutionFailure::GenerationTask(failure),
                occurred_at,
            )?,
        }
        let commit = WorkflowGenerationTaskCompletionCommit::try_new(
            run,
            previous_event_count,
            command.completion_id,
            WorkflowExecuteRunEffect { workflow_run_id: command.origin.workflow_run_id },
        )?;
        self.repository.commit_workflow_generation_task_completion(commit).await?;
        Ok(WorkflowCompleteGenerationTaskOutcome::Applied)
    }
}

const fn node_is_terminal(state: WorkflowNodeExecutionState) -> bool {
    matches!(
        state,
        WorkflowNodeExecutionState::Succeeded
            | WorkflowNodeExecutionState::Failed
            | WorkflowNodeExecutionState::Blocked
            | WorkflowNodeExecutionState::Cancelled
    )
}

const fn run_is_terminal(state: super::WorkflowRunState) -> bool {
    matches!(
        state,
        super::WorkflowRunState::Succeeded
            | super::WorkflowRunState::Failed
            | super::WorkflowRunState::Cancelled
    )
}

enum DesiredNodeOutcome {
    Succeeded(WorkflowNodeOutputSet),
    Failed(WorkflowGenerationTaskFailure),
}

fn validate_origin(
    run: &super::WorkflowRunAggregate,
    origin: &WorkflowGenerationTaskOrigin,
) -> Result<usize, WorkflowApplicationError> {
    if run.project_id() != origin.project_id
        || run.workflow_id() != origin.workflow_id
        || run.workflow_revision() != origin.workflow_revision
    {
        return Err(WorkflowApplicationError::WorkflowGenerationTaskCompletionConflict);
    }
    let index = run
        .node_executions()
        .iter()
        .position(|node| node.execution_id() == origin.node_execution_id)
        .ok_or(WorkflowApplicationError::WorkflowGenerationTaskCompletionConflict)?;
    let planned = &run.plan().nodes()[index];
    if planned.node_id != origin.workflow_node_id
        || planned.capability_contract != origin.capability_contract_ref
    {
        return Err(WorkflowApplicationError::WorkflowGenerationTaskCompletionConflict);
    }
    Ok(index)
}

fn desired_node_outcome(
    capabilities: &WorkflowNodeCapabilityRegistry,
    command: &WorkflowCompleteGenerationTaskCommand,
) -> Result<DesiredNodeOutcome, WorkflowApplicationError> {
    match &command.outcome {
        WorkflowGenerationTaskCompletionOutcome::Failed(failure) => {
            Ok(DesiredNodeOutcome::Failed(*failure))
        }
        WorkflowGenerationTaskCompletionOutcome::Succeeded(value) => {
            let capability = capabilities
                .resolve_node_capability(&command.origin.capability_contract_ref)
                .map_err(|_| WorkflowApplicationError::WorkflowGenerationTaskCompletionConflict)?;
            let contract = capability.node_capability_contract();
            let primary = contract
                .outputs()
                .iter()
                .find(|output| output.is_primary())
                .ok_or(WorkflowApplicationError::WorkflowGenerationTaskCompletionConflict)?;
            let outputs = WorkflowNodeOutputSet::try_new(
                contract,
                BTreeMap::from([(primary.key().clone(), value.clone().into_runtime_value())]),
            )
            .map_err(|_| WorkflowApplicationError::WorkflowGenerationTaskCompletionConflict)?;
            Ok(DesiredNodeOutcome::Succeeded(outputs))
        }
    }
}

fn node_outcome_matches(
    node: &super::WorkflowNodeExecutionEntity,
    desired: &DesiredNodeOutcome,
) -> bool {
    match desired {
        DesiredNodeOutcome::Succeeded(outputs) => node.outputs() == Some(outputs),
        DesiredNodeOutcome::Failed(failure) => {
            node.failure() == Some(&WorkflowNodeExecutionFailure::GenerationTask(*failure))
        }
    }
}
