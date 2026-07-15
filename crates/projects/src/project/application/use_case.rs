//! Frozen Project application use cases.

use super::{
    ProjectApplicationError, ProjectListPage, ProjectListQuery, ProjectMutationCommandHash,
    ProjectMutationOperation, ProjectMutationOutcome, ProjectMutationReceipt,
    ProjectMutationRequestId, ProjectWorkflowSummary,
};
use crate::project::domain::{
    ProjectAggregate, ProjectCreatedAt, ProjectId, ProjectName, ProjectRevision,
};
use crate::project::interfaces::{
    ProjectClockInterface, ProjectIdentityGeneratorInterface, ProjectRepositoryInterface,
    ProjectWorkflowSummaryReaderInterface,
};
use std::sync::Arc;

/// Input for creating one durable Project.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectCreateRequest {
    /// Stable idempotency identity.
    pub request_id: ProjectMutationRequestId,
    /// Validated normalized Project name.
    pub name: ProjectName,
}

/// Creates one durable Project or replays its exact prior outcome.
pub struct ProjectCreateUseCase {
    repository: Arc<dyn ProjectRepositoryInterface>,
    clock: Arc<dyn ProjectClockInterface>,
    identity_generator: Arc<dyn ProjectIdentityGeneratorInterface>,
}

impl ProjectCreateUseCase {
    /// Wires the exact dependencies consumed by Project creation.
    #[must_use]
    pub fn new(
        repository: Arc<dyn ProjectRepositoryInterface>,
        clock: Arc<dyn ProjectClockInterface>,
        identity_generator: Arc<dyn ProjectIdentityGeneratorInterface>,
    ) -> Self {
        Self { repository, clock, identity_generator }
    }

    /// Creates one Project or returns the matching stored mutation outcome.
    pub async fn create_project(
        &self,
        request: ProjectCreateRequest,
    ) -> Result<ProjectAggregate, ProjectApplicationError> {
        let command_hash = ProjectMutationCommandHash::for_project_creation(&request.name);
        if let Some(receipt) =
            self.repository.load_project_mutation_receipt(request.request_id).await?
        {
            return project_from_matching_receipt(
                receipt,
                request.request_id,
                command_hash,
                ProjectMutationOperation::Create,
            );
        }
        let observed_at = self.clock.observe_project_time()?;
        let project = ProjectAggregate::create(
            self.identity_generator.generate_project_id(),
            request.name,
            ProjectCreatedAt::from_observed_project_time(observed_at),
        );
        let receipt = ProjectMutationReceipt::new(
            request.request_id,
            command_hash,
            ProjectMutationOperation::Create,
            ProjectMutationOutcome::from_project(project.clone()),
        );
        let committed = self.repository.commit_project_creation(project, receipt).await?;
        project_from_matching_receipt(
            committed,
            request.request_id,
            command_hash,
            ProjectMutationOperation::Create,
        )
    }
}

/// Input for renaming one exact Project revision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectRenameRequest {
    /// Stable idempotency identity.
    pub request_id: ProjectMutationRequestId,
    /// Project to rename.
    pub project_id: ProjectId,
    /// Revision that must still be current.
    pub expected_revision: ProjectRevision,
    /// Validated normalized replacement name.
    pub name: ProjectName,
}

/// Renames one Project through revision compare-and-swap.
pub struct ProjectRenameUseCase {
    repository: Arc<dyn ProjectRepositoryInterface>,
    clock: Arc<dyn ProjectClockInterface>,
}

impl ProjectRenameUseCase {
    /// Wires the exact dependencies consumed by Project rename.
    #[must_use]
    pub fn new(
        repository: Arc<dyn ProjectRepositoryInterface>,
        clock: Arc<dyn ProjectClockInterface>,
    ) -> Self {
        Self { repository, clock }
    }

    /// Renames one Project or returns the matching stored mutation outcome.
    pub async fn rename_project(
        &self,
        request: ProjectRenameRequest,
    ) -> Result<ProjectAggregate, ProjectApplicationError> {
        let command_hash = ProjectMutationCommandHash::for_project_rename(
            request.project_id,
            request.expected_revision,
            &request.name,
        );
        if let Some(receipt) =
            self.repository.load_project_mutation_receipt(request.request_id).await?
        {
            return project_from_matching_receipt(
                receipt,
                request.request_id,
                command_hash,
                ProjectMutationOperation::Rename,
            );
        }
        let mut project =
            load_required_project(self.repository.as_ref(), request.project_id).await?;
        if project.revision() != request.expected_revision {
            return Err(ProjectApplicationError::ProjectRevisionConflict {
                project_id: request.project_id,
                expected_revision: request.expected_revision,
                actual_revision: project.revision(),
            });
        }
        project.rename(request.name, self.clock.observe_project_time()?)?;
        let receipt = ProjectMutationReceipt::new(
            request.request_id,
            command_hash,
            ProjectMutationOperation::Rename,
            ProjectMutationOutcome::from_project(project.clone()),
        );
        let committed = self
            .repository
            .commit_project_rename(project, request.expected_revision, receipt)
            .await?;
        project_from_matching_receipt(
            committed,
            request.request_id,
            command_hash,
            ProjectMutationOperation::Rename,
        )
    }
}

/// Loads one exact Project.
pub struct ProjectGetUseCase {
    repository: Arc<dyn ProjectRepositoryInterface>,
}

impl ProjectGetUseCase {
    /// Wires the Project repository.
    #[must_use]
    pub fn new(repository: Arc<dyn ProjectRepositoryInterface>) -> Self {
        Self { repository }
    }

    /// Returns one Project or `ProjectNotFound`.
    pub async fn get_project(
        &self,
        project_id: ProjectId,
    ) -> Result<ProjectAggregate, ProjectApplicationError> {
        load_required_project(self.repository.as_ref(), project_id).await
    }
}

/// Lists stable bounded Project keyset pages.
pub struct ProjectListUseCase {
    repository: Arc<dyn ProjectRepositoryInterface>,
}

impl ProjectListUseCase {
    /// Wires the Project repository.
    #[must_use]
    pub fn new(repository: Arc<dyn ProjectRepositoryInterface>) -> Self {
        Self { repository }
    }

    /// Returns one stable bounded Project page.
    pub async fn list_projects(
        &self,
        query: ProjectListQuery,
    ) -> Result<ProjectListPage, ProjectApplicationError> {
        self.repository.list_projects(query).await
    }
}

/// Opened Project plus its optional translated current-Workflow summary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectWorkspaceView {
    /// Authoritative opened Project.
    pub project: ProjectAggregate,
    /// Minimal optional current-Workflow projection.
    pub current_workflow_summary: Option<ProjectWorkflowSummary>,
}

/// Opens one Project without creating or selecting global state.
pub struct ProjectOpenUseCase {
    repository: Arc<dyn ProjectRepositoryInterface>,
    workflow_summary_reader: Arc<dyn ProjectWorkflowSummaryReaderInterface>,
}

impl ProjectOpenUseCase {
    /// Wires Project load and current-Workflow summary dependencies.
    #[must_use]
    pub fn new(
        repository: Arc<dyn ProjectRepositoryInterface>,
        workflow_summary_reader: Arc<dyn ProjectWorkflowSummaryReaderInterface>,
    ) -> Self {
        Self { repository, workflow_summary_reader }
    }

    /// Returns the Project and its optional current-Workflow summary.
    pub async fn open_project(
        &self,
        project_id: ProjectId,
    ) -> Result<ProjectWorkspaceView, ProjectApplicationError> {
        let project = load_required_project(self.repository.as_ref(), project_id).await?;
        let current_workflow_summary = self
            .workflow_summary_reader
            .read_current_project_workflow_summary(project_id)
            .await
            .map_err(|_| ProjectApplicationError::ProjectWorkflowSummaryReadFailure)?;
        Ok(ProjectWorkspaceView { project, current_workflow_summary })
    }
}

async fn load_required_project(
    repository: &dyn ProjectRepositoryInterface,
    project_id: ProjectId,
) -> Result<ProjectAggregate, ProjectApplicationError> {
    repository
        .load_project(project_id)
        .await?
        .ok_or(ProjectApplicationError::ProjectNotFound { project_id })
}

fn project_from_matching_receipt(
    receipt: ProjectMutationReceipt,
    request_id: ProjectMutationRequestId,
    command_hash: ProjectMutationCommandHash,
    operation: ProjectMutationOperation,
) -> Result<ProjectAggregate, ProjectApplicationError> {
    if receipt.request_id() != request_id
        || receipt.command_hash() != command_hash
        || receipt.operation() != operation
    {
        return Err(ProjectApplicationError::ProjectMutationIdempotencyConflict { request_id });
    }
    Ok(receipt.outcome().project().clone())
}
