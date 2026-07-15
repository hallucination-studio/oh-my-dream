#[path = "project_interfaces/support.rs"]
#[allow(dead_code)]
mod project_interfaces_support;

use async_trait::async_trait;
use project_interfaces_support::*;
use projects::project::application::{
    ProjectApplicationError, ProjectCreateRequest, ProjectCreateUseCase, ProjectGetUseCase,
    ProjectListLimit, ProjectListQuery, ProjectListUseCase, ProjectMutationCommandHash,
    ProjectMutationRequestId, ProjectOpenUseCase, ProjectRenameRequest, ProjectRenameUseCase,
    ProjectWorkflowIdBoundaryValue, ProjectWorkflowReadinessSummary,
    ProjectWorkflowRevisionBoundaryValue, ProjectWorkflowSummary,
};
use projects::project::domain::{ProjectId, ProjectName, ProjectRevision, ProjectUpdatedAt};
use projects::project::interfaces::ProjectWorkflowSummaryReaderInterface;
use std::sync::Arc;
use uuid::Uuid;

#[test]
fn mutation_command_hashes_match_the_frozen_binary_vectors() {
    let name = ProjectName::new("First").expect("name is valid");
    assert_eq!(
        ProjectMutationCommandHash::for_project_creation(&name).as_bytes(),
        &[
            0x2d, 0xb3, 0x7c, 0xa9, 0x21, 0x66, 0x3a, 0x42, 0x49, 0xf8, 0x93, 0xb8, 0x9c, 0xa3,
            0x24, 0x92, 0xdf, 0xe5, 0x24, 0x44, 0xf9, 0xb6, 0xc3, 0x45, 0x3a, 0xe1, 0xae, 0x5b,
            0xc9, 0x5b, 0xa4, 0x73,
        ],
    );
    let renamed = ProjectName::new("After").expect("name is valid");
    assert_eq!(
        ProjectMutationCommandHash::for_project_rename(
            project_id("018f47a2-4e12-4f79-8bd8-95d2b26f4418"),
            ProjectRevision::initial(),
            &renamed,
        )
        .as_bytes(),
        &[
            0xaf, 0x9d, 0x26, 0x3e, 0x9b, 0x9d, 0x70, 0x95, 0xb3, 0x3d, 0xd2, 0x0e, 0xd6, 0x2c,
            0x0e, 0x50, 0x0a, 0x07, 0x09, 0xe4, 0x1e, 0x60, 0xf0, 0xe6, 0x44, 0xfc, 0xbd, 0xfb,
            0x87, 0x08, 0xf2, 0x29,
        ],
    );
}

#[tokio::test]
async fn create_replays_the_original_outcome_without_generating_new_values() {
    let repository = Arc::new(InMemoryProjectRepositoryFake::default());
    let request = ProjectCreateRequest {
        request_id: request_id("118f47a2-4e12-4f79-8bd8-95d2b26f4418"),
        name: ProjectName::new("  First  ").expect("name is valid"),
    };
    let use_case = ProjectCreateUseCase::new(
        repository,
        Arc::new(DeterministicProjectClockFake {
            observed_at: ProjectUpdatedAt::new(10).expect("timestamp is valid"),
        }),
        Arc::new(SequenceProjectIdentityGeneratorFake::new([project_id(
            "018f47a2-4e12-4f79-8bd8-95d2b26f4418",
        )])),
    );
    let created = use_case.create_project(request.clone()).await.expect("create succeeds");
    let replayed = use_case.create_project(request).await.expect("replay succeeds");
    assert_eq!(replayed, created);
    assert_eq!(created.name().as_str(), "First");
    assert_eq!(created.revision(), ProjectRevision::initial());
}

#[tokio::test]
async fn rename_uses_revision_cas_and_replays_the_exact_committed_outcome() {
    let repository = Arc::new(InMemoryProjectRepositoryFake::default());
    let create = ProjectCreateUseCase::new(
        repository.clone(),
        Arc::new(DeterministicProjectClockFake {
            observed_at: ProjectUpdatedAt::new(10).expect("timestamp is valid"),
        }),
        Arc::new(SequenceProjectIdentityGeneratorFake::new([project_id(
            "018f47a2-4e12-4f79-8bd8-95d2b26f4418",
        )])),
    );
    let create_request = ProjectCreateRequest {
        request_id: request_id("118f47a2-4e12-4f79-8bd8-95d2b26f4418"),
        name: ProjectName::new("Before").expect("name is valid"),
    };
    let created = create.create_project(create_request.clone()).await.expect("create succeeds");
    let rename = ProjectRenameUseCase::new(
        repository,
        Arc::new(DeterministicProjectClockFake {
            observed_at: ProjectUpdatedAt::new(20).expect("timestamp is valid"),
        }),
    );
    let request = ProjectRenameRequest {
        request_id: request_id("218f47a2-4e12-4f79-8bd8-95d2b26f4418"),
        project_id: created.id(),
        expected_revision: created.revision(),
        name: ProjectName::new("After").expect("name is valid"),
    };
    let renamed = rename.rename_project(request.clone()).await.expect("rename succeeds");
    assert_eq!(renamed.name().as_str(), "After");
    assert_eq!(renamed.revision().get(), 2);
    assert_eq!(rename.rename_project(request).await.expect("replay succeeds"), renamed);
    assert_eq!(
        create.create_project(create_request).await.expect("creation replay succeeds"),
        created,
    );
}

#[tokio::test]
async fn get_returns_not_found_and_list_returns_the_repository_page() {
    let repository = Arc::new(InMemoryProjectRepositoryFake::default());
    let missing = project_id("018f47a2-4e12-4f79-8bd8-95d2b26f4499");
    let get = ProjectGetUseCase::new(repository.clone());
    assert_eq!(
        get.get_project(missing).await,
        Err(ProjectApplicationError::ProjectNotFound { project_id: missing }),
    );
    let list = ProjectListUseCase::new(repository);
    assert_eq!(
        list.list_projects(ProjectListQuery {
            limit: ProjectListLimit::new(10).expect("limit is valid"),
            cursor: None,
        })
        .await
        .expect("list succeeds")
        .projects,
        [],
    );
}

#[tokio::test]
async fn open_returns_the_project_and_only_the_translated_workflow_summary() {
    let repository = Arc::new(InMemoryProjectRepositoryFake::default());
    let create = ProjectCreateUseCase::new(
        repository.clone(),
        Arc::new(DeterministicProjectClockFake {
            observed_at: ProjectUpdatedAt::new(10).expect("timestamp is valid"),
        }),
        Arc::new(SequenceProjectIdentityGeneratorFake::new([project_id(
            "018f47a2-4e12-4f79-8bd8-95d2b26f4418",
        )])),
    );
    let project = create
        .create_project(ProjectCreateRequest {
            request_id: request_id("118f47a2-4e12-4f79-8bd8-95d2b26f4418"),
            name: ProjectName::new("Project").expect("name is valid"),
        })
        .await
        .expect("create succeeds");
    let summary = ProjectWorkflowSummary {
        workflow_id: ProjectWorkflowIdBoundaryValue::new("workflow-1")
            .expect("workflow id is valid"),
        workflow_revision: ProjectWorkflowRevisionBoundaryValue::new(4).expect("revision is valid"),
        readiness: ProjectWorkflowReadinessSummary::Ready,
    };
    let open = ProjectOpenUseCase::new(
        repository.clone(),
        Arc::new(DeterministicProjectWorkflowSummaryReaderFake { summary: Some(summary.clone()) }),
    );
    let workspace = open.open_project(project.id()).await.expect("open succeeds");
    assert_eq!(workspace.project, project);
    assert_eq!(workspace.current_workflow_summary, Some(summary));
    let open_without_workflow = ProjectOpenUseCase::new(
        repository,
        Arc::new(DeterministicProjectWorkflowSummaryReaderFake { summary: None }),
    );
    assert_eq!(
        open_without_workflow
            .open_project(workspace.project.id())
            .await
            .expect("open without Workflow succeeds")
            .current_workflow_summary,
        None,
    );
}

#[tokio::test]
async fn open_translates_workflow_reader_failures_to_the_project_error() {
    let repository = Arc::new(InMemoryProjectRepositoryFake::default());
    let create = ProjectCreateUseCase::new(
        repository.clone(),
        Arc::new(DeterministicProjectClockFake {
            observed_at: ProjectUpdatedAt::new(10).expect("timestamp is valid"),
        }),
        Arc::new(SequenceProjectIdentityGeneratorFake::new([project_id(
            "018f47a2-4e12-4f79-8bd8-95d2b26f4418",
        )])),
    );
    let project = create
        .create_project(ProjectCreateRequest {
            request_id: request_id("118f47a2-4e12-4f79-8bd8-95d2b26f4418"),
            name: ProjectName::new("Project").expect("name is valid"),
        })
        .await
        .expect("create succeeds");
    let open =
        ProjectOpenUseCase::new(repository, Arc::new(FailingProjectWorkflowSummaryReaderFake));
    assert_eq!(
        open.open_project(project.id()).await,
        Err(ProjectApplicationError::ProjectWorkflowSummaryReadFailure),
    );
}

fn request_id(value: &str) -> ProjectMutationRequestId {
    ProjectMutationRequestId::from_uuid(Uuid::parse_str(value).expect("test UUID is valid"))
        .expect("test UUID is version four")
}

struct FailingProjectWorkflowSummaryReaderFake;

#[async_trait]
impl ProjectWorkflowSummaryReaderInterface for FailingProjectWorkflowSummaryReaderFake {
    async fn read_current_project_workflow_summary(
        &self,
        _project_id: ProjectId,
    ) -> Result<Option<ProjectWorkflowSummary>, ProjectApplicationError> {
        Err(ProjectApplicationError::ProjectPersistenceFailure)
    }
}
