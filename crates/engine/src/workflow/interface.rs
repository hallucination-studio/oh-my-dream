use async_trait::async_trait;
use projects::project::domain::ProjectId;

use crate::node_capability::{
    WorkflowManagedAudioRef, WorkflowManagedImageRef, WorkflowManagedVideoRef,
    WorkflowNodeExecutionId, WorkflowRunId,
};
use crate::workflow_graph::{
    WorkflowAggregate, WorkflowGraphError, WorkflowId, WorkflowMutationReceipt,
    WorkflowMutationRequestId, WorkflowRevision,
};

use super::{
    WorkflowCreateRequestId, WorkflowGenerationTaskCompletionId, WorkflowRunAggregate,
    WorkflowRunEvent, WorkflowRunRequestId, WorkflowRunTime,
};

/// Closed application and consumer-boundary failures for Workflow operations.
#[derive(Clone, Debug, thiserror::Error, PartialEq, Eq)]
pub enum WorkflowApplicationError {
    /// The authoritative Run domain rejected a value or transition.
    #[error(transparent)]
    WorkflowDomain(#[from] super::WorkflowDomainError),
    /// The authoritative Workflow graph domain rejected a value or mutation.
    #[error(transparent)]
    WorkflowGraph(#[from] WorkflowGraphError),
    /// No current Workflow exists for the requested Project.
    #[error("Workflow was not found")]
    WorkflowNotFound {
        /// Exact lookup identity that was absent.
        key: WorkflowLoadKey,
    },
    /// The Project already owns its one current Workflow.
    #[error("Project already has a Workflow")]
    WorkflowAlreadyExistsForProject,
    /// Creation idempotency identity was reused for different content.
    #[error("Workflow creation idempotency conflict")]
    WorkflowCreationIdempotencyConflict,
    /// Mutation compare-and-swap observed another revision.
    #[error("Workflow revision conflict")]
    WorkflowRevisionConflict,
    /// Mutation idempotency identity was reused for different content.
    #[error("Workflow mutation idempotency conflict")]
    WorkflowMutationIdempotencyConflict,
    /// No Run exists for the requested identity and Project.
    #[error("Workflow Run was not found")]
    WorkflowRunNotFound,
    /// Run admission did not target the current Workflow revision.
    #[error("Workflow Run revision mismatch")]
    WorkflowRunRevisionMismatch,
    /// Run request identity was reused for different content.
    #[error("Workflow Run idempotency conflict")]
    WorkflowRunIdempotencyConflict,
    /// Current readiness blocks Run admission.
    #[error("Workflow is not ready")]
    WorkflowNotReady {
        /// Complete sorted readiness evidence.
        readiness: super::WorkflowReadinessResult,
    },
    /// A Run event page limit was outside `1..=500`.
    #[error("Workflow Run event limit must be between 1 and 500")]
    WorkflowRunEventLimitOutOfBounds {
        /// Rejected requested limit.
        requested_limit: u16,
    },
    /// A persistence operation failed without exposing implementation details.
    #[error("Workflow persistence failed")]
    WorkflowPersistenceFailure,
    /// A media preview could not be issued.
    #[error("Workflow media preview issue failed")]
    WorkflowMediaPreviewIssueFailure,
    /// Exact capability execution or coordinator task failed.
    #[error("Workflow capability execution failed")]
    WorkflowCapabilityExecutionFailure,
    /// A committed event could not be published.
    #[error("Workflow Run event publish failed")]
    WorkflowRunEventPublishFailure,
    /// A terminal Task notification does not match its exact frozen origin or prior outcome.
    #[error("Workflow Generation Task completion conflicts with the Run")]
    WorkflowGenerationTaskCompletionConflict,
    /// Task recovery evidence was absent or contradicted an exact waiting origin.
    #[error("Workflow Generation Task recovery read failed")]
    WorkflowGenerationTaskRecoveryReadFailure,
}

/// Task-owned durable evidence projected for one exact Workflow node execution at startup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkflowGenerationTaskRecoveryObservation {
    /// Queued Task and unconsumed Submit effect prove safe pre-handoff replay.
    QueuedPreHandoff,
    /// A non-terminal Task owns continued external work.
    Active,
    /// A terminal Task still owns an unconsumed Workflow notification.
    TerminalNotificationPending,
    /// The terminal Task notification was already durably consumed.
    NotificationCompleted,
    /// No Task exists for the exact origin.
    Absent,
    /// Task identity, state, or outbox evidence is contradictory.
    Corrupt,
}

/// Exact Task recovery evidence consumed by Workflow startup classification.
#[async_trait]
pub trait WorkflowGenerationTaskRecoveryReaderInterface: Send + Sync {
    /// Reads one exact Running or waiting node execution origin.
    async fn read_workflow_generation_task_recovery(
        &self,
        origin: &super::WorkflowGenerationTaskOrigin,
    ) -> Result<WorkflowGenerationTaskRecoveryObservation, WorkflowApplicationError>;
}

/// Atomic Workflow creation commit and its replay evidence.
#[derive(Clone, Debug, PartialEq)]
pub struct WorkflowCreationCommit {
    workflow: WorkflowAggregate,
    receipt: WorkflowCreateReceipt,
}

impl WorkflowCreationCommit {
    /// Creates an atomic unit only when the receipt retains the exact same snapshot.
    pub fn try_new(
        workflow: WorkflowAggregate,
        receipt: WorkflowCreateReceipt,
    ) -> Result<Self, WorkflowApplicationError> {
        if receipt.created_workflow != workflow {
            return Err(WorkflowApplicationError::WorkflowPersistenceFailure);
        }
        Ok(Self { workflow, receipt })
    }
    /// Separates the already validated persistence records.
    #[must_use]
    pub fn into_parts(self) -> (WorkflowAggregate, WorkflowCreateReceipt) {
        (self.workflow, self.receipt)
    }
}

/// Canonical Workflow creation command hash.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct WorkflowCreateCommandHash([u8; 32]);

impl WorkflowCreateCommandHash {
    /// Restores exact SHA-256 bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
    /// Returns exact SHA-256 bytes.
    #[must_use]
    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Exact replay evidence for one Workflow creation.
#[derive(Clone, Debug, PartialEq)]
pub struct WorkflowCreateReceipt {
    /// Stable request identity.
    pub(super) request_id: WorkflowCreateRequestId,
    /// Canonical command hash.
    pub(super) command_hash: WorkflowCreateCommandHash,
    /// Exact created snapshot returned by replay.
    pub(super) created_workflow: WorkflowAggregate,
    /// SHA-256 integrity fingerprint of the created snapshot.
    pub(super) result_fingerprint: [u8; 32],
}

/// Atomic Workflow mutation compare-and-swap request.
#[derive(Clone, Debug, PartialEq)]
pub struct WorkflowMutationCommit {
    workflow: WorkflowAggregate,
    expected_revision: WorkflowRevision,
    receipt: WorkflowMutationReceipt,
}

impl WorkflowMutationCommit {
    /// Creates an atomic mutation unit only for the exact receipt snapshot.
    pub fn try_new(
        workflow: WorkflowAggregate,
        expected_revision: WorkflowRevision,
        receipt: WorkflowMutationReceipt,
    ) -> Result<Self, WorkflowApplicationError> {
        if receipt.committed_workflow() != &workflow {
            return Err(WorkflowApplicationError::WorkflowPersistenceFailure);
        }
        Ok(Self { workflow, expected_revision, receipt })
    }
    /// Separates the already validated compare-and-swap records.
    #[must_use]
    pub fn into_parts(self) -> (WorkflowAggregate, WorkflowRevision, WorkflowMutationReceipt) {
        (self.workflow, self.expected_revision, self.receipt)
    }
}

/// Post-commit intent admitted atomically with a queued Run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorkflowExecuteRunEffect {
    /// Admitted Run to coordinate.
    pub workflow_run_id: WorkflowRunId,
}

/// Atomic waiting-node completion and downstream execution intent.
#[derive(Clone, Debug)]
pub struct WorkflowGenerationTaskCompletionCommit {
    run: WorkflowRunAggregate,
    expected_last_event_count: usize,
    completion_id: WorkflowGenerationTaskCompletionId,
    effect: WorkflowExecuteRunEffect,
}

impl WorkflowGenerationTaskCompletionCommit {
    /// Creates one atomic unit only for a newly appended terminal node event and matching Run.
    pub fn try_new(
        run: WorkflowRunAggregate,
        expected_last_event_count: usize,
        completion_id: WorkflowGenerationTaskCompletionId,
        effect: WorkflowExecuteRunEffect,
    ) -> Result<Self, WorkflowApplicationError> {
        if effect.workflow_run_id != run.run_id()
            || run.events().len() != expected_last_event_count.saturating_add(1)
        {
            return Err(WorkflowApplicationError::WorkflowGenerationTaskCompletionConflict);
        }
        Ok(Self { run, expected_last_event_count, completion_id, effect })
    }

    /// Separates the validated Run transition and exact downstream effect identity.
    #[must_use]
    pub fn into_parts(
        self,
    ) -> (WorkflowRunAggregate, usize, WorkflowGenerationTaskCompletionId, WorkflowExecuteRunEffect)
    {
        (self.run, self.expected_last_event_count, self.completion_id, self.effect)
    }
}

/// Run admission evidence kept opaque until W4 constructs its frozen hash contract.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowRunAdmissionReceipt {
    /// Stable request identity.
    pub(super) request_id: WorkflowRunRequestId,
    /// Canonical SHA-256 admission command hash.
    pub(super) command_hash: WorkflowRunCommandHash,
    /// Admitted Run identity.
    pub(super) workflow_run_id: WorkflowRunId,
}

/// Canonical Workflow Run admission command hash.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct WorkflowRunCommandHash([u8; 32]);

impl WorkflowRunCommandHash {
    /// Restores exact SHA-256 bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
    /// Returns exact SHA-256 bytes.
    #[must_use]
    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Atomic Run admission unit.
#[derive(Clone, Debug)]
pub struct WorkflowRunAdmissionCommit {
    run: WorkflowRunAggregate,
    receipt: WorkflowRunAdmissionReceipt,
    effect: WorkflowExecuteRunEffect,
}

impl WorkflowRunAdmissionCommit {
    /// Creates one consistent queued Run, receipt, and execution-effect unit.
    pub fn try_new(
        run: WorkflowRunAggregate,
        receipt: WorkflowRunAdmissionReceipt,
        effect: WorkflowExecuteRunEffect,
    ) -> Result<Self, WorkflowApplicationError> {
        let run_id = run.run_id();
        if receipt.workflow_run_id != run_id
            || effect.workflow_run_id != run_id
            || run.state() != super::WorkflowRunState::Queued
            || run.events().len() != 1
        {
            return Err(WorkflowApplicationError::WorkflowPersistenceFailure);
        }
        Ok(Self { run, receipt, effect })
    }
    /// Separates the already validated admission records.
    #[must_use]
    pub fn into_parts(
        self,
    ) -> (WorkflowRunAggregate, WorkflowRunAdmissionReceipt, WorkflowExecuteRunEffect) {
        (self.run, self.receipt, self.effect)
    }
}

/// Exact typed managed-media value accepted by preview issuance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkflowManagedMediaPreviewSource {
    /// Managed image.
    Image(WorkflowManagedImageRef),
    /// Managed video.
    Video(WorkflowManagedVideoRef),
    /// Managed audio.
    Audio(WorkflowManagedAudioRef),
}

/// Opaque short-lived preview boundary value; Desktop owns URL semantics.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowMediaPreview(String);

impl WorkflowMediaPreview {
    /// Creates a bounded non-empty opaque preview value.
    pub fn try_new(value: impl Into<String>) -> Result<Self, WorkflowApplicationError> {
        let value = value.into();
        if value.is_empty() || value.len() > 2_048 {
            Err(WorkflowApplicationError::WorkflowMediaPreviewIssueFailure)
        } else {
            Ok(Self(value))
        }
    }
    /// Returns the opaque Desktop-issued value.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Exact identity accepted by the single Workflow load operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkflowLoadKey {
    /// Load the one current Workflow associated with a Project.
    Project(ProjectId),
    /// Load one Workflow by aggregate identity.
    Workflow(WorkflowId),
}

/// Persistence boundary consumed by Workflow document use cases.
#[async_trait]
pub trait WorkflowAggregateRepositoryInterface: Send + Sync {
    /// Loads a current Workflow through one exact supported identity.
    async fn load_workflow(
        &self,
        key: WorkflowLoadKey,
    ) -> Result<Option<WorkflowAggregate>, WorkflowApplicationError>;
    /// Loads exact Workflow creation replay evidence.
    async fn load_workflow_creation_receipt(
        &self,
        request_id: WorkflowCreateRequestId,
    ) -> Result<Option<WorkflowCreateReceipt>, WorkflowApplicationError>;
    /// Loads an exact prior mutation receipt for idempotent replay.
    async fn load_workflow_mutation_receipt(
        &self,
        request_id: WorkflowMutationRequestId,
    ) -> Result<Option<WorkflowMutationReceipt>, WorkflowApplicationError>;
    /// Atomically creates the Project's first Workflow and opaque creation receipt.
    async fn commit_workflow_creation(
        &self,
        commit: WorkflowCreationCommit,
    ) -> Result<WorkflowAggregate, WorkflowApplicationError>;
    /// Atomically compare-and-swaps the snapshot and exact mutation receipt.
    async fn commit_workflow_mutation(
        &self,
        commit: WorkflowMutationCommit,
    ) -> Result<WorkflowMutationReceipt, WorkflowApplicationError>;
}

/// Exact identity accepted by the single Run load operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkflowRunLoadKey {
    /// Internal lookup by globally unique Run identity.
    Run(WorkflowRunId),
    /// Project-scoped external query.
    ProjectScoped {
        /// Owning Project.
        project_id: ProjectId,
        /// Requested Run.
        workflow_run_id: WorkflowRunId,
    },
}

/// Persistence boundary consumed by Run admission, coordination, and queries.
#[async_trait]
pub trait WorkflowRunRepositoryInterface: Send + Sync {
    /// Loads one Run through an exact supported identity.
    async fn load_workflow_run(
        &self,
        key: WorkflowRunLoadKey,
    ) -> Result<Option<WorkflowRunAggregate>, WorkflowApplicationError>;
    /// Lists at most `limit` non-terminal Runs for one Project in newest-first order.
    async fn list_active_project_workflow_runs(
        &self,
        project_id: ProjectId,
        limit: usize,
    ) -> Result<Vec<WorkflowRunAggregate>, WorkflowApplicationError>;
    /// Loads prior admission evidence by stable request identity bytes.
    async fn load_workflow_run_admission_receipt(
        &self,
        request_id: WorkflowRunRequestId,
    ) -> Result<Option<WorkflowRunAdmissionReceipt>, WorkflowApplicationError>;
    /// Atomically admits a queued Run, receipt, first event, and execution effect.
    async fn admit_workflow_run(
        &self,
        commit: WorkflowRunAdmissionCommit,
    ) -> Result<WorkflowRunAdmissionReceipt, WorkflowApplicationError>;
    /// Atomically stores one complete aggregate transition and its newly appended events.
    async fn commit_workflow_run_transition(
        &self,
        run: WorkflowRunAggregate,
        expected_last_event_count: usize,
    ) -> Result<WorkflowRunAggregate, WorkflowApplicationError>;
    /// Atomically commits one terminal Task outcome, event, and downstream execution effect.
    async fn commit_workflow_generation_task_completion(
        &self,
        commit: WorkflowGenerationTaskCompletionCommit,
    ) -> Result<WorkflowRunAggregate, WorkflowApplicationError>;
    /// Loads at most `limit` events after an optional exclusive sequence cursor.
    async fn list_workflow_run_events_after(
        &self,
        workflow_run_id: WorkflowRunId,
        after_sequence: Option<super::WorkflowRunEventSequence>,
        limit: usize,
    ) -> Result<Vec<WorkflowRunEvent>, WorkflowApplicationError>;
    /// Loads the latest Run containing one node by `(created_at, Run ID)`.
    async fn load_latest_workflow_run_for_node(
        &self,
        project_id: ProjectId,
        workflow_id: WorkflowId,
        node_id: crate::workflow_graph::WorkflowNodeId,
    ) -> Result<Option<WorkflowRunAggregate>, WorkflowApplicationError>;
}

/// Deterministic Workflow time source.
pub trait WorkflowClockInterface: Send + Sync {
    /// Observes the current non-negative UTC millisecond time.
    fn current_workflow_time(&self) -> Result<WorkflowRunTime, WorkflowApplicationError>;
}

/// Authoritative Workflow identity source.
pub trait WorkflowIdentityGeneratorInterface: Send + Sync {
    /// Generates a Workflow aggregate identity.
    fn generate_workflow_id(&self) -> WorkflowId;
    /// Generates a Run identity.
    fn generate_workflow_run_id(&self) -> WorkflowRunId;
    /// Generates a node-execution identity.
    fn generate_workflow_node_execution_id(&self) -> WorkflowNodeExecutionId;
}

/// Short-lived media preview boundary consumed only by presentation queries.
#[async_trait]
pub trait WorkflowMediaPreviewIssuerInterface: Send + Sync {
    /// Issues an opaque preview scoped to one Project and typed managed-media value.
    async fn issue_workflow_media_preview(
        &self,
        project_id: ProjectId,
        source: WorkflowManagedMediaPreviewSource,
    ) -> Result<WorkflowMediaPreview, WorkflowApplicationError>;
}

/// Delivery boundary for already committed durable Run events.
#[async_trait]
pub trait WorkflowRunEventPublisherInterface: Send + Sync {
    /// Publishes one committed event without changing its durable sequence.
    async fn publish_committed_workflow_run_event(
        &self,
        event: WorkflowRunEvent,
    ) -> Result<(), WorkflowApplicationError>;
}
