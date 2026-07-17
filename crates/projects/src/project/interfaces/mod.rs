//! Consumer-owned Project substitution boundaries.

use crate::project::application::{
    ProjectApplicationError, ProjectListPage, ProjectListQuery, ProjectMutationReceipt,
    ProjectMutationRequestId, ProjectWorkflowSummary,
};
use crate::project::domain::{ProjectAggregate, ProjectId, ProjectRevision, ProjectUpdatedAt};
use async_trait::async_trait;

/// Persistence boundary consumed by Project use cases.
#[async_trait]
pub trait ProjectRepositoryInterface: Send + Sync {
    /// Loads one exact Project, or `None` when it does not exist.
    async fn load_project(
        &self,
        project_id: ProjectId,
    ) -> Result<Option<ProjectAggregate>, ProjectApplicationError>;

    /// Lists one stable bounded Project keyset page.
    async fn list_projects(
        &self,
        query: ProjectListQuery,
    ) -> Result<ProjectListPage, ProjectApplicationError>;

    /// Loads a prior mutation result before generating new identity or time values.
    async fn load_project_mutation_receipt(
        &self,
        request_id: ProjectMutationRequestId,
    ) -> Result<Option<ProjectMutationReceipt>, ProjectApplicationError>;

    /// Atomically commits Project creation and its mutation receipt.
    async fn commit_project_creation(
        &self,
        project: ProjectAggregate,
        receipt: ProjectMutationReceipt,
    ) -> Result<ProjectMutationReceipt, ProjectApplicationError>;

    /// Atomically compare-and-swaps a rename and commits its mutation receipt.
    async fn commit_project_rename(
        &self,
        project: ProjectAggregate,
        expected_revision: ProjectRevision,
        receipt: ProjectMutationReceipt,
    ) -> Result<ProjectMutationReceipt, ProjectApplicationError>;
}

/// Optional current-Workflow projection boundary consumed by Project open.
#[async_trait]
pub trait ProjectWorkflowSummaryReaderInterface: Send + Sync {
    /// Reads the minimal translated current-Workflow summary for one Project.
    async fn read_current_project_workflow_summary(
        &self,
        project_id: ProjectId,
    ) -> Result<Option<ProjectWorkflowSummary>, ProjectApplicationError>;
}

/// Deterministic Project time source.
pub trait ProjectClockInterface: Send + Sync {
    /// Observes the current non-negative UTC millisecond value.
    fn observe_project_time(&self) -> Result<ProjectUpdatedAt, ProjectApplicationError>;
}

/// Authoritative Project identity source.
pub trait ProjectIdentityGeneratorInterface: Send + Sync {
    /// Generates one RFC-compatible UUIDv4 Project identity.
    fn generate_project_id(&self) -> ProjectId;
}
