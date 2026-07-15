//! Durable, revisioned authority for the optional Project Workflow head.

use engine::Workflow;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;

/// The durable Workflow projection for one Project.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkflowHead {
    /// Project scope for this head.
    pub project_id: String,
    /// Compare-and-swap revision, starting at one.
    pub revision: u64,
    /// Portable creative Workflow document.
    pub workflow: Workflow,
}

/// A caller request to create or update one Workflow head.
#[derive(Clone, Debug)]
pub struct WorkflowCommitRequest {
    /// Project scope supplied by trusted application context.
    pub project_id: String,
    /// Expected current revision; `None` means create only if absent.
    pub expected_revision: Option<u64>,
    /// Stable request identity used for deduplication.
    pub request_id: String,
    /// Caller-computed hash of the complete normalized mutation request.
    pub request_hash: String,
    /// Candidate canonical Workflow after the mutation.
    pub workflow: Workflow,
}

impl WorkflowCommitRequest {
    /// Creates a Workflow head mutation request.
    #[must_use]
    pub fn new(
        project_id: impl Into<String>,
        expected_revision: Option<u64>,
        request_id: impl Into<String>,
        request_hash: impl Into<String>,
        workflow: Workflow,
    ) -> Self {
        Self {
            project_id: project_id.into(),
            expected_revision,
            request_id: request_id.into(),
            request_hash: request_hash.into(),
            workflow,
        }
    }
}

/// Result returned after a Workflow mutation or an idempotent retry.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkflowCommitResult {
    /// Canonical head after the request, or `None` for an absent no-op.
    pub head: Option<WorkflowHead>,
    /// Whether this request changed the Workflow head.
    pub changed: bool,
    /// Whether the result came from an existing request receipt.
    pub deduplicated: bool,
    /// Durable undo journal identity for a changed head.
    pub undo_id: Option<String>,
}

impl WorkflowCommitResult {
    pub(crate) fn mark_deduplicated(mut self) -> Self {
        self.deduplicated = true;
        self
    }
}

/// Errors raised by Workflow authority validation, CAS, or persistence.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum WorkflowAuthorityError {
    /// A Project, request, or hash identifier was empty.
    #[error("{field} must not be empty")]
    EmptyIdentifier { field: &'static str },
    /// The request scope did not match the Workflow document.
    #[error(
        "Workflow project `{workflow_project_id}` does not match request project `{request_project_id}`"
    )]
    ProjectMismatch { request_project_id: String, workflow_project_id: String },
    /// The expected revision did not match the current head.
    #[error("Workflow revision conflict: expected {expected:?}, current {actual:?}")]
    RevisionConflict { expected: Option<u64>, actual: Option<u64> },
    /// A request id was reused with a different request hash.
    #[error("request `{request_id}` was reused with a different request hash")]
    RequestHashMismatch { request_id: String },
    /// The SQLite repository could not complete its transaction.
    #[error("Workflow storage failure: {message}")]
    Storage { message: String },
    /// A persisted row did not represent a valid Workflow authority document.
    #[error("invalid persisted Workflow authority data: {message}")]
    CorruptData { message: String },
    /// The revision counter cannot be incremented safely.
    #[error("Workflow revision overflow")]
    RevisionOverflow,
}

/// Storage boundary consumed by [`WorkflowAuthority`].
pub trait WorkflowRepository: Send + Sync {
    /// Loads the current optional head without creating one.
    fn load_head(&self, project_id: &str) -> Result<Option<WorkflowHead>, WorkflowAuthorityError>;

    /// Loads an exact prior request result without performing a mutation.
    fn load_receipt(
        &self,
        project_id: &str,
        request_id: &str,
        request_hash: &str,
    ) -> Result<Option<WorkflowCommitResult>, WorkflowAuthorityError>;

    /// Atomically applies CAS, dedupe, head, receipt, and undo semantics.
    fn commit(
        &self,
        request: &WorkflowCommitRequest,
    ) -> Result<WorkflowCommitResult, WorkflowAuthorityError>;
}

/// Application service owning Workflow lifecycle and mutation semantics.
pub struct WorkflowAuthority {
    repository: Arc<dyn WorkflowRepository>,
}

impl WorkflowAuthority {
    /// Creates an authority over a selected repository adapter.
    #[must_use]
    pub fn new(repository: Arc<dyn WorkflowRepository>) -> Self {
        Self { repository }
    }

    /// Creates an authority from a concrete repository at the composition root.
    #[must_use]
    pub fn from_repository<R>(repository: R) -> Self
    where
        R: WorkflowRepository + 'static,
    {
        Self::new(Arc::new(repository))
    }

    /// Reads the optional head without creating an empty Workflow.
    pub fn load_head(
        &self,
        project_id: &str,
    ) -> Result<Option<WorkflowHead>, WorkflowAuthorityError> {
        validate_identifier(project_id, "project_id")?;
        self.repository.load_head(project_id)
    }

    /// Reads a prior exact request result for crash-after-commit recovery.
    pub fn load_receipt(
        &self,
        project_id: &str,
        request_id: &str,
        request_hash: &str,
    ) -> Result<Option<WorkflowCommitResult>, WorkflowAuthorityError> {
        validate_identifier(project_id, "project_id")?;
        validate_identifier(request_id, "request_id")?;
        validate_identifier(request_hash, "request_hash")?;
        self.repository
            .load_receipt(project_id, request_id, request_hash)
            .map(|result| result.map(WorkflowCommitResult::mark_deduplicated))
    }

    /// Applies one canonical mutation through the repository transaction.
    pub fn apply(
        &self,
        request: WorkflowCommitRequest,
    ) -> Result<WorkflowCommitResult, WorkflowAuthorityError> {
        validate_request(&request)?;
        self.repository.commit(&request)
    }
}

fn validate_request(request: &WorkflowCommitRequest) -> Result<(), WorkflowAuthorityError> {
    validate_identifier(&request.project_id, "project_id")?;
    validate_identifier(&request.request_id, "request_id")?;
    validate_identifier(&request.request_hash, "request_hash")?;
    if request.workflow.project_id != request.project_id {
        return Err(WorkflowAuthorityError::ProjectMismatch {
            request_project_id: request.project_id.clone(),
            workflow_project_id: request.workflow.project_id.clone(),
        });
    }
    Ok(())
}

fn validate_identifier(value: &str, field: &'static str) -> Result<(), WorkflowAuthorityError> {
    if value.trim().is_empty() {
        return Err(WorkflowAuthorityError::EmptyIdentifier { field });
    }
    Ok(())
}
