use super::*;
use projects::project::application::{
    ProjectListLimit, ProjectMutationCommandHash, ProjectMutationOutcome,
};
use projects::project::domain::{ProjectCreatedAt, ProjectName};

fn repository() -> SqliteProjectRepositoryAdapterImpl {
    SqliteProjectRepositoryAdapterImpl::try_new(Arc::new(Mutex::new(
        Connection::open_in_memory().expect("in-memory database"),
    )))
    .expect("adapter initializes")
}

fn id(seed: u8) -> ProjectId {
    let mut bytes = [seed; 16];
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    ProjectId::from_uuid(uuid::Uuid::from_bytes(bytes)).expect("version four")
}

fn request_id(seed: u8) -> ProjectMutationRequestId {
    ProjectMutationRequestId::from_uuid(id(seed).as_uuid()).expect("version four")
}

fn project(seed: u8, name: &str, timestamp: i64) -> ProjectAggregate {
    ProjectAggregate::create(
        id(seed),
        ProjectName::new(name).expect("valid name"),
        ProjectCreatedAt::new(timestamp).expect("valid timestamp"),
    )
}

fn receipt(
    seed: u8,
    hash: u8,
    operation: ProjectMutationOperation,
    project: ProjectAggregate,
) -> ProjectMutationReceipt {
    ProjectMutationReceipt::new(
        request_id(seed),
        ProjectMutationCommandHash::from_bytes([hash; 32]),
        operation,
        ProjectMutationOutcome::from_project(project),
    )
}

#[tokio::test]
async fn creation_is_atomic_and_matching_request_replays_exact_receipt() {
    let repository = repository();
    let project = project(1, "Project", 10);
    let receipt = receipt(11, 7, ProjectMutationOperation::Create, project.clone());

    assert_eq!(
        repository
            .commit_project_creation(project.clone(), receipt.clone())
            .await
            .expect("creation succeeds"),
        receipt
    );
    assert_eq!(repository.load_project(project.id()).await.unwrap(), Some(project.clone()));
    assert_eq!(
        repository.commit_project_creation(project, receipt.clone()).await.unwrap(),
        receipt
    );
    assert_eq!(
        repository.load_project_mutation_receipt(receipt.request_id()).await.unwrap(),
        Some(receipt)
    );
}

#[tokio::test]
async fn request_reuse_and_stale_rename_fail_without_partial_write() {
    let repository = repository();
    let before = project(2, "Before", 20);
    let created = receipt(12, 1, ProjectMutationOperation::Create, before.clone());
    repository.commit_project_creation(before.clone(), created.clone()).await.unwrap();
    let mismatch = receipt(12, 2, ProjectMutationOperation::Create, before.clone());
    assert!(matches!(
        repository.commit_project_creation(before.clone(), mismatch).await,
        Err(ProjectApplicationError::ProjectMutationIdempotencyConflict { .. })
    ));

    let mut after = before.clone();
    after.rename(ProjectName::new("After").unwrap(), ProjectUpdatedAt::new(21).unwrap()).unwrap();
    let stale = ProjectRevision::from_non_zero(2).unwrap();
    let renamed = receipt(13, 3, ProjectMutationOperation::Rename, after);
    assert!(matches!(
        repository.commit_project_rename(renamed.outcome().project().clone(), stale, renamed).await,
        Err(ProjectApplicationError::ProjectRevisionConflict { .. })
    ));
    assert_eq!(repository.load_project(before.id()).await.unwrap(), Some(before));
    assert_eq!(repository.load_project_mutation_receipt(request_id(13)).await.unwrap(), None);
}

#[tokio::test]
async fn successful_rename_preserves_the_original_creation_receipt_snapshot() {
    let repository = repository();
    let before = project(8, "Before", 30);
    let created = receipt(31, 4, ProjectMutationOperation::Create, before.clone());
    repository.commit_project_creation(before.clone(), created.clone()).await.unwrap();
    let mut after = before.clone();
    after.rename(ProjectName::new("After").unwrap(), ProjectUpdatedAt::new(31).unwrap()).unwrap();
    let renamed = receipt(32, 5, ProjectMutationOperation::Rename, after.clone());

    assert_eq!(
        repository
            .commit_project_rename(after.clone(), before.revision(), renamed.clone())
            .await
            .unwrap(),
        renamed
    );
    assert_eq!(repository.load_project(before.id()).await.unwrap(), Some(after));
    assert_eq!(repository.commit_project_creation(before, created.clone()).await.unwrap(), created);
}

#[tokio::test]
async fn list_uses_stable_descending_exclusive_keyset_pages() {
    let repository = repository();
    for (seed, name, timestamp) in [(3, "Old", 10), (4, "New A", 20), (5, "New B", 20)] {
        let project = project(seed, name, timestamp);
        repository
            .commit_project_creation(
                project.clone(),
                receipt(seed + 20, seed, ProjectMutationOperation::Create, project),
            )
            .await
            .unwrap();
    }
    let limit = ProjectListLimit::new(2).unwrap();
    let first = repository.list_projects(ProjectListQuery { limit, cursor: None }).await.unwrap();
    assert_eq!(names(&first), ["New B", "New A"]);
    let second = repository
        .list_projects(ProjectListQuery { limit, cursor: first.next_cursor })
        .await
        .unwrap();
    assert_eq!(names(&second), ["Old"]);
    assert_eq!(second.next_cursor, None);
}

#[test]
fn system_clock_and_uuid_generator_satisfy_domain_values() {
    let observed = SystemProjectClockAdapterImpl.observe_project_time().unwrap();
    assert!(observed.get() > 0);
    let generated = UuidProjectIdentityGeneratorAdapterImpl.generate_project_id();
    assert_eq!(generated.as_uuid().get_version(), Some(uuid::Version::Random));
}

fn names(page: &ProjectListPage) -> Vec<&str> {
    page.projects.iter().map(|project| project.name().as_str()).collect()
}
