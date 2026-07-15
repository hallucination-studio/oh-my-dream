#[path = "project_interfaces/support.rs"]
mod project_interfaces_support;

use project_interfaces_support::*;
use projects::project::application::{
    ProjectApplicationError, ProjectListLimit, ProjectListQuery, ProjectMutationOperation,
    ProjectMutationReceipt, ProjectMutationRequestId, ProjectMutationResultFingerprint,
    ProjectWorkflowIdBoundaryValue, ProjectWorkflowReadinessSummary,
    ProjectWorkflowRevisionBoundaryValue, ProjectWorkflowSummary,
};
use projects::project::domain::{ProjectName, ProjectRevision, ProjectUpdatedAt};
use projects::project::interfaces::{
    ProjectClockInterface, ProjectIdentityGeneratorInterface, ProjectRepositoryInterface,
    ProjectWorkflowSummaryReaderInterface,
};
use uuid::Uuid;

#[test]
fn project_list_limit_enforces_the_frozen_bounds() {
    assert_eq!(ProjectListLimit::new(1).expect("minimum is valid").get(), 1);
    assert_eq!(ProjectListLimit::new(100).expect("maximum is valid").get(), 100);
    assert_eq!(
        ProjectListLimit::new(0),
        Err(ProjectApplicationError::ProjectListLimitOutOfBounds { requested_limit: 0 }),
    );
    assert_eq!(
        ProjectListLimit::new(101),
        Err(ProjectApplicationError::ProjectListLimitOutOfBounds { requested_limit: 101 }),
    );
}

#[test]
fn workflow_boundary_id_uses_the_documented_utf8_byte_bound() {
    assert_eq!(ProjectWorkflowIdBoundaryValue::new(""), None);
    assert!(ProjectWorkflowIdBoundaryValue::new("x".repeat(128)).is_some());
    assert_eq!(ProjectWorkflowIdBoundaryValue::new("界".repeat(43)), None);
}

#[test]
fn mutation_request_id_accepts_only_rfc_uuid_version_four() {
    let valid =
        Uuid::parse_str("118f47a2-4e12-4f79-8bd8-95d2b26f4418").expect("test UUID is valid");
    assert!(ProjectMutationRequestId::from_uuid(valid).is_some());
    let version_one =
        Uuid::parse_str("6ba7b810-9dad-11d1-80b4-00c04fd430c8").expect("test UUID is valid");
    assert_eq!(ProjectMutationRequestId::from_uuid(version_one), None);
}

#[test]
fn mutation_receipt_restore_rejects_a_corrupt_result_fingerprint() {
    let receipt = receipt(
        "118f47a2-4e12-4f79-8bd8-95d2b26f4418",
        1,
        ProjectMutationOperation::Create,
        project(project_id("018f47a2-4e12-4f79-8bd8-95d2b26f4418"), "Project", 10),
    );
    assert_eq!(
        receipt.result_fingerprint().as_bytes(),
        &[
            0x42, 0x5a, 0xd0, 0x78, 0xe2, 0xfd, 0x3c, 0xfd, 0x89, 0xad, 0x8e, 0xaa, 0x58, 0x14,
            0x96, 0x4a, 0x38, 0x55, 0x04, 0x11, 0x31, 0x1f, 0xad, 0xe0, 0xe6, 0x40, 0xa1, 0xce,
            0xba, 0x55, 0x86, 0x0d,
        ],
    );
    assert_eq!(
        ProjectMutationReceipt::restore(
            receipt.request_id(),
            receipt.command_hash(),
            receipt.operation(),
            receipt.outcome().clone(),
            ProjectMutationResultFingerprint::from_bytes([0xff; 32]),
        ),
        Err(ProjectApplicationError::ProjectPersistenceFailure),
    );
}

#[test]
fn deterministic_clock_and_identity_fakes_return_configured_values() {
    let clock = DeterministicProjectClockFake {
        observed_at: ProjectUpdatedAt::new(42).expect("timestamp is valid"),
    };
    assert_eq!(clock.observe_project_time().expect("clock succeeds").get(), 42);
    let first = project_id("018f47a2-4e12-4f79-8bd8-95d2b26f4418");
    let second = project_id("018f47a2-4e12-4f79-8bd8-95d2b26f4419");
    let generator = SequenceProjectIdentityGeneratorFake::new([first, second]);
    assert_eq!(generator.generate_project_id(), first);
    assert_eq!(generator.generate_project_id(), second);
}

#[test]
fn all_project_interfaces_are_object_safe_substitution_boundaries() {
    fn accepts_repository(_value: &dyn ProjectRepositoryInterface) {}
    fn accepts_reader(_value: &dyn ProjectWorkflowSummaryReaderInterface) {}
    fn accepts_clock(_value: &dyn ProjectClockInterface) {}
    fn accepts_generator(_value: &dyn ProjectIdentityGeneratorInterface) {}
    accepts_repository(&InMemoryProjectRepositoryFake::default());
    accepts_reader(&DeterministicProjectWorkflowSummaryReaderFake { summary: None });
    accepts_clock(&DeterministicProjectClockFake {
        observed_at: ProjectUpdatedAt::new(1).expect("timestamp is valid"),
    });
    accepts_generator(&SequenceProjectIdentityGeneratorFake::new([project_id(
        "018f47a2-4e12-4f79-8bd8-95d2b26f4418",
    )]));
}

#[tokio::test]
async fn workflow_summary_reader_returns_only_the_project_owned_projection() {
    let summary = ProjectWorkflowSummary {
        workflow_id: ProjectWorkflowIdBoundaryValue::new("workflow-1")
            .expect("boundary id is valid"),
        workflow_revision: ProjectWorkflowRevisionBoundaryValue::new(3)
            .expect("revision is non-zero"),
        readiness: ProjectWorkflowReadinessSummary::Blocked,
    };
    let reader = DeterministicProjectWorkflowSummaryReaderFake { summary: Some(summary.clone()) };
    assert_eq!(
        reader
            .read_current_project_workflow_summary(project_id(
                "018f47a2-4e12-4f79-8bd8-95d2b26f4418",
            ))
            .await
            .expect("reader succeeds"),
        Some(summary),
    );
}

#[tokio::test]
async fn repository_fake_commits_creation_and_replays_the_exact_receipt() {
    let repository = InMemoryProjectRepositoryFake::default();
    let id = project_id("018f47a2-4e12-4f79-8bd8-95d2b26f4418");
    let project = project(id, "Created", 10);
    let receipt = receipt(
        "118f47a2-4e12-4f79-8bd8-95d2b26f4418",
        1,
        ProjectMutationOperation::Create,
        project.clone(),
    );
    assert_eq!(
        repository
            .commit_project_creation(project.clone(), receipt.clone())
            .await
            .expect("create succeeds"),
        receipt,
    );
    assert_eq!(repository.load_project(id).await.expect("load succeeds"), Some(project.clone()));
    assert_eq!(
        repository
            .commit_project_creation(project, receipt.clone())
            .await
            .expect("matching replay succeeds"),
        receipt,
    );
    assert_eq!(
        repository
            .load_project_mutation_receipt(receipt.request_id())
            .await
            .expect("receipt load succeeds"),
        Some(receipt),
    );
}

#[tokio::test]
async fn repository_fake_rejects_mismatched_replay_and_revision_conflict() {
    let repository = InMemoryProjectRepositoryFake::default();
    let id = project_id("018f47a2-4e12-4f79-8bd8-95d2b26f4418");
    let project = project(id, "Before", 10);
    let create_receipt = receipt(
        "118f47a2-4e12-4f79-8bd8-95d2b26f4418",
        1,
        ProjectMutationOperation::Create,
        project.clone(),
    );
    repository
        .commit_project_creation(project.clone(), create_receipt.clone())
        .await
        .expect("create succeeds");
    let mismatched = receipt(
        "118f47a2-4e12-4f79-8bd8-95d2b26f4418",
        2,
        ProjectMutationOperation::Create,
        project.clone(),
    );
    assert_eq!(
        repository.commit_project_creation(project.clone(), mismatched).await,
        Err(ProjectApplicationError::ProjectMutationIdempotencyConflict {
            request_id: create_receipt.request_id(),
        }),
    );
    let mut renamed = project;
    renamed
        .rename(
            ProjectName::new("After").expect("name is valid"),
            ProjectUpdatedAt::new(11).expect("timestamp is valid"),
        )
        .expect("rename succeeds");
    let expected_revision = ProjectRevision::from_non_zero(2).expect("revision is non-zero");
    let rename_receipt = receipt(
        "218f47a2-4e12-4f79-8bd8-95d2b26f4418",
        3,
        ProjectMutationOperation::Rename,
        renamed.clone(),
    );
    assert_eq!(
        repository.commit_project_rename(renamed, expected_revision, rename_receipt).await,
        Err(ProjectApplicationError::ProjectRevisionConflict {
            project_id: id,
            expected_revision,
            actual_revision: ProjectRevision::initial(),
        }),
    );
}

#[tokio::test]
async fn repository_fake_returns_stable_descending_keyset_pages() {
    let repository = InMemoryProjectRepositoryFake::default();
    let fixtures = [
        (
            "018f47a2-4e12-4f79-8bd8-95d2b26f4418",
            "Old",
            10,
            "118f47a2-4e12-4f79-8bd8-95d2b26f4418",
            1,
        ),
        (
            "018f47a2-4e12-4f79-8bd8-95d2b26f4419",
            "New A",
            20,
            "118f47a2-4e12-4f79-8bd8-95d2b26f4419",
            2,
        ),
        (
            "018f47a2-4e12-4f79-8bd8-95d2b26f4420",
            "New B",
            20,
            "118f47a2-4e12-4f79-8bd8-95d2b26f4420",
            3,
        ),
    ];
    for (uuid, name, updated, request, hash) in fixtures {
        let project = project(project_id(uuid), name, updated);
        let receipt = receipt(request, hash, ProjectMutationOperation::Create, project.clone());
        repository.commit_project_creation(project, receipt).await.expect("create succeeds");
    }
    let query =
        ProjectListQuery { limit: ProjectListLimit::new(2).expect("limit is valid"), cursor: None };
    let first = repository.list_projects(query).await.expect("first page succeeds");
    assert_eq!(project_names(&first), ["New B", "New A"]);
    let second = repository
        .list_projects(ProjectListQuery { limit: query.limit, cursor: first.next_cursor })
        .await
        .expect("second page succeeds");
    assert_eq!(project_names(&second), ["Old"]);
    assert_eq!(second.next_cursor, None);
}
