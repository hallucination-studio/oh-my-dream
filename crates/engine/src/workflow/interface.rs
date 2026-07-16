use async_trait::async_trait;
use projects::project::domain::ProjectId;

use crate::node_capability::{
    WorkflowManagedAudioRef, WorkflowManagedImageRef, WorkflowManagedVideoRef,
    WorkflowNodeExecutionId, WorkflowRunId,
};
use crate::workflow_graph::{
    WorkflowAggregate, WorkflowId, WorkflowMutationReceipt, WorkflowMutationRequestId,
    WorkflowRevision,
};

use super::{
    WorkflowCreateRequestId, WorkflowRunAggregate, WorkflowRunEvent, WorkflowRunRequestId,
    WorkflowRunTime,
};

/// Closed application and consumer-boundary failures for Workflow operations.
#[derive(Clone, Debug, thiserror::Error, PartialEq, Eq)]
pub enum WorkflowApplicationError {
    /// No current Workflow exists for the requested Project.
    #[error("Workflow was not found")]
    WorkflowNotFound,
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
    WorkflowNotReady,
    /// A persistence operation failed without exposing implementation details.
    #[error("Workflow persistence failed")]
    WorkflowPersistenceFailure,
    /// A media preview could not be issued.
    #[error("Workflow media preview issue failed")]
    WorkflowMediaPreviewIssueFailure,
    /// A committed event could not be published.
    #[error("Workflow Run event publish failed")]
    WorkflowRunEventPublishFailure,
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
    pub request_id: WorkflowCreateRequestId,
    /// Canonical command hash.
    pub command_hash: WorkflowCreateCommandHash,
    /// Exact created snapshot returned by replay.
    pub created_workflow: WorkflowAggregate,
    /// SHA-256 integrity fingerprint of the created snapshot.
    pub result_fingerprint: [u8; 32],
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

/// Run admission evidence kept opaque until W4 constructs its frozen hash contract.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowRunAdmissionReceipt {
    /// Stable request identity.
    pub request_id: WorkflowRunRequestId,
    /// Canonical SHA-256 admission command hash.
    pub command_hash: WorkflowRunCommandHash,
    /// Admitted Run identity.
    pub workflow_run_id: WorkflowRunId,
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

/// Persistence boundary consumed by Workflow document use cases.
#[async_trait]
pub trait WorkflowAggregateRepositoryInterface: Send + Sync {
    /// Loads the one current Workflow owned by a Project.
    async fn load_workflow(
        &self,
        project_id: ProjectId,
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

/// Persistence boundary consumed by Run admission, coordination, and queries.
#[async_trait]
pub trait WorkflowRunRepositoryInterface: Send + Sync {
    /// Loads one Project-scoped Run.
    async fn load_workflow_run(
        &self,
        project_id: ProjectId,
        workflow_run_id: WorkflowRunId,
    ) -> Result<Option<WorkflowRunAggregate>, WorkflowApplicationError>;
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
