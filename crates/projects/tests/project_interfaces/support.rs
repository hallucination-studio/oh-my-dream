use async_trait::async_trait;
use projects::project::application::{
    ProjectApplicationError, ProjectListCursor, ProjectListPage, ProjectListQuery,
    ProjectMutationCommandHash, ProjectMutationOperation, ProjectMutationOutcome,
    ProjectMutationReceipt, ProjectMutationRequestId, ProjectWorkflowSummary,
};
use projects::project::domain::{
    ProjectAggregate, ProjectCreatedAt, ProjectId, ProjectName, ProjectRevision, ProjectUpdatedAt,
};
use projects::project::interfaces::{
    ProjectClockInterface, ProjectIdentityGeneratorInterface, ProjectRepositoryInterface,
    ProjectWorkflowSummaryReaderInterface,
};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::{Mutex, MutexGuard};
use uuid::Uuid;

pub struct DeterministicProjectClockFakeImpl {
    pub observed_at: ProjectUpdatedAt,
}

impl ProjectClockInterface for DeterministicProjectClockFakeImpl {
    fn observe_project_time(&self) -> Result<ProjectUpdatedAt, ProjectApplicationError> {
        Ok(self.observed_at)
    }
}

pub struct SequenceProjectIdentityGeneratorFakeImpl {
    ids: Mutex<VecDeque<ProjectId>>,
}

impl SequenceProjectIdentityGeneratorFakeImpl {
    pub fn new(ids: impl IntoIterator<Item = ProjectId>) -> Self {
        Self { ids: Mutex::new(ids.into_iter().collect()) }
    }
}

impl ProjectIdentityGeneratorInterface for SequenceProjectIdentityGeneratorFakeImpl {
    fn generate_project_id(&self) -> ProjectId {
        self.ids.lock().expect("healthy fake lock").pop_front().expect("configured fake identity")
    }
}

pub struct DeterministicProjectWorkflowSummaryReaderFakeImpl {
    pub summary: Option<ProjectWorkflowSummary>,
}

#[async_trait]
impl ProjectWorkflowSummaryReaderInterface for DeterministicProjectWorkflowSummaryReaderFakeImpl {
    async fn read_current_project_workflow_summary(
        &self,
        _project_id: ProjectId,
    ) -> Result<Option<ProjectWorkflowSummary>, ProjectApplicationError> {
        Ok(self.summary.clone())
    }
}

#[derive(Default)]
pub struct InMemoryProjectRepositoryFakeImpl {
    state: Mutex<InMemoryProjectRepositoryState>,
}

#[derive(Default)]
struct InMemoryProjectRepositoryState {
    projects: BTreeMap<ProjectId, ProjectAggregate>,
    receipts: HashMap<ProjectMutationRequestId, ProjectMutationReceipt>,
}

#[async_trait]
impl ProjectRepositoryInterface for InMemoryProjectRepositoryFakeImpl {
    async fn load_project(
        &self,
        project_id: ProjectId,
    ) -> Result<Option<ProjectAggregate>, ProjectApplicationError> {
        Ok(self.lock_state()?.projects.get(&project_id).cloned())
    }

    async fn list_projects(
        &self,
        query: ProjectListQuery,
    ) -> Result<ProjectListPage, ProjectApplicationError> {
        let state = self.lock_state()?;
        let mut projects: Vec<_> = state.projects.values().cloned().collect();
        projects.sort_by_key(|project| (project.updated_at(), project.id()));
        projects.reverse();
        if let Some(cursor) = query.cursor {
            projects.retain(|project| {
                (project.updated_at(), project.id()) < (cursor.updated_at, cursor.project_id)
            });
        }
        let has_more = projects.len() > usize::from(query.limit.get());
        projects.truncate(usize::from(query.limit.get()));
        let next_cursor = if has_more {
            projects.last().map(|last| ProjectListCursor {
                updated_at: last.updated_at(),
                project_id: last.id(),
            })
        } else {
            None
        };
        Ok(ProjectListPage { projects, next_cursor })
    }

    async fn load_project_mutation_receipt(
        &self,
        request_id: ProjectMutationRequestId,
    ) -> Result<Option<ProjectMutationReceipt>, ProjectApplicationError> {
        Ok(self.lock_state()?.receipts.get(&request_id).cloned())
    }

    async fn commit_project_creation(
        &self,
        project: ProjectAggregate,
        receipt: ProjectMutationReceipt,
    ) -> Result<ProjectMutationReceipt, ProjectApplicationError> {
        let mut state = self.lock_state()?;
        if let Some(replayed) = replay_receipt(&state, &receipt)? {
            return Ok(replayed);
        }
        if receipt.operation() != ProjectMutationOperation::Create
            || receipt.outcome().project() != &project
            || state.projects.contains_key(&project.id())
        {
            return Err(ProjectApplicationError::ProjectPersistenceFailure);
        }
        state.projects.insert(project.id(), project);
        state.receipts.insert(receipt.request_id(), receipt.clone());
        Ok(receipt)
    }

    async fn commit_project_rename(
        &self,
        project: ProjectAggregate,
        expected_revision: ProjectRevision,
        receipt: ProjectMutationReceipt,
    ) -> Result<ProjectMutationReceipt, ProjectApplicationError> {
        let mut state = self.lock_state()?;
        if let Some(replayed) = replay_receipt(&state, &receipt)? {
            return Ok(replayed);
        }
        let current = state
            .projects
            .get(&project.id())
            .ok_or(ProjectApplicationError::ProjectNotFound { project_id: project.id() })?;
        if current.revision() != expected_revision {
            return Err(ProjectApplicationError::ProjectRevisionConflict {
                project_id: project.id(),
                expected_revision,
                actual_revision: current.revision(),
            });
        }
        if receipt.operation() != ProjectMutationOperation::Rename
            || receipt.outcome().project() != &project
        {
            return Err(ProjectApplicationError::ProjectPersistenceFailure);
        }
        state.projects.insert(project.id(), project);
        state.receipts.insert(receipt.request_id(), receipt.clone());
        Ok(receipt)
    }
}

impl InMemoryProjectRepositoryFakeImpl {
    fn lock_state(
        &self,
    ) -> Result<MutexGuard<'_, InMemoryProjectRepositoryState>, ProjectApplicationError> {
        self.state.lock().map_err(|_| ProjectApplicationError::ProjectPersistenceFailure)
    }
}

fn replay_receipt(
    state: &InMemoryProjectRepositoryState,
    proposed: &ProjectMutationReceipt,
) -> Result<Option<ProjectMutationReceipt>, ProjectApplicationError> {
    let Some(existing) = state.receipts.get(&proposed.request_id()) else {
        return Ok(None);
    };
    if existing.command_hash() != proposed.command_hash() {
        return Err(ProjectApplicationError::ProjectMutationIdempotencyConflict {
            request_id: proposed.request_id(),
        });
    }
    Ok(Some(existing.clone()))
}

pub fn project_id(value: &str) -> ProjectId {
    ProjectId::from_uuid(Uuid::parse_str(value).expect("valid test UUID"))
        .expect("version-four test UUID")
}

pub fn project(id: ProjectId, name: &str, timestamp: i64) -> ProjectAggregate {
    ProjectAggregate::create(
        id,
        ProjectName::new(name).expect("valid name"),
        ProjectCreatedAt::new(timestamp).expect("valid timestamp"),
    )
}

pub fn receipt(
    request_id: &str,
    hash_byte: u8,
    operation: ProjectMutationOperation,
    project: ProjectAggregate,
) -> ProjectMutationReceipt {
    let request_id = ProjectMutationRequestId::from_uuid(
        Uuid::parse_str(request_id).expect("valid test request UUID"),
    )
    .expect("version-four test request UUID");
    ProjectMutationReceipt::new(
        request_id,
        ProjectMutationCommandHash::from_bytes([hash_byte; 32]),
        operation,
        ProjectMutationOutcome::from_project(project),
    )
}

pub fn project_names(page: &ProjectListPage) -> Vec<&str> {
    page.projects.iter().map(ProjectAggregate::name).map(ProjectName::as_str).collect()
}
