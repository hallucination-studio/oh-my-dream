//! Project application failures.

use super::ProjectMutationRequestId;
use crate::project::domain::{ProjectDomainError, ProjectId, ProjectRevision};

/// A Project use-case or consumer-interface failure.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ProjectApplicationError {
    /// The Project domain rejected a value or transition.
    #[error(transparent)]
    ProjectDomain(#[from] ProjectDomainError),
    /// No Project exists for the requested identity.
    #[error("Project was not found")]
    ProjectNotFound { project_id: ProjectId },
    /// A rename did not target the current Project revision.
    #[error("Project revision conflict")]
    ProjectRevisionConflict {
        project_id: ProjectId,
        expected_revision: ProjectRevision,
        actual_revision: ProjectRevision,
    },
    /// One mutation request identity was reused for different content.
    #[error("Project mutation request was reused for different content")]
    ProjectMutationIdempotencyConflict { request_id: ProjectMutationRequestId },
    /// A Project list limit was outside `1..=100`.
    #[error("Project list limit must be between 1 and 100")]
    ProjectListLimitOutOfBounds { requested_limit: u16 },
    /// Project persistence could not complete its operation.
    #[error("Project persistence failed")]
    ProjectPersistenceFailure,
    /// The optional current Workflow summary could not be read.
    #[error("Project Workflow summary read failed")]
    ProjectWorkflowSummaryReadFailure,
}
