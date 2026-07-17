use projects::project::domain::{
    ProjectAggregate, ProjectCreatedAt, ProjectDomainError, ProjectId, ProjectName,
    ProjectRevision, ProjectUpdatedAt,
};
use uuid::Uuid;

const PROJECT_UUID: &str = "018f47a2-4e12-4f79-8bd8-95d2b26f4418";

#[test]
fn project_id_accepts_only_uuid_version_four() {
    let version_four = Uuid::parse_str(PROJECT_UUID).expect("test UUID is valid");
    let id = ProjectId::from_uuid(version_four).expect("test UUID is version four");
    assert_eq!(id.as_uuid(), version_four);

    let version_one =
        Uuid::parse_str("6ba7b810-9dad-11d1-80b4-00c04fd430c8").expect("test UUID is valid");
    assert_eq!(ProjectId::from_uuid(version_one), None);

    let mut invalid_variant_bytes = *version_four.as_bytes();
    invalid_variant_bytes[8] = (invalid_variant_bytes[8] & 0x3f) | 0xc0;
    assert_eq!(ProjectId::from_uuid(Uuid::from_bytes(invalid_variant_bytes)), None);
}

#[test]
fn project_name_trims_edges_and_preserves_interior_text() {
    let name = ProjectName::new("  A  Dream  ").expect("name is valid");
    assert_eq!(name.as_str(), "A  Dream");
}

#[test]
fn project_name_rejects_empty_control_and_overlong_values() {
    assert_eq!(ProjectName::new(" \n "), Err(ProjectDomainError::ProjectNameEmpty));
    assert_eq!(
        ProjectName::new("bad\u{0000}name"),
        Err(ProjectDomainError::ProjectNameContainsControl),
    );
    assert_eq!(ProjectName::new("x".repeat(121)), Err(ProjectDomainError::ProjectNameTooLong),);
}

#[test]
fn rename_advances_revision_and_monotonic_update_time() {
    let mut project = ProjectAggregate::create(
        ProjectId::from_uuid(Uuid::parse_str(PROJECT_UUID).expect("test UUID is valid"))
            .expect("test UUID is version four"),
        ProjectName::new("First").expect("name is valid"),
        ProjectCreatedAt::new(10).expect("timestamp is valid"),
    );

    project
        .rename(
            ProjectName::new("Second").expect("name is valid"),
            ProjectUpdatedAt::new(9).expect("timestamp is valid"),
        )
        .expect("rename succeeds");

    assert_eq!(project.revision().get(), 2);
    assert_eq!(project.updated_at().get(), 11);
}

#[test]
fn rename_rejects_the_current_normalized_name_without_mutation() {
    let mut project = ProjectAggregate::create(
        ProjectId::from_uuid(Uuid::parse_str(PROJECT_UUID).expect("test UUID is valid"))
            .expect("test UUID is version four"),
        ProjectName::new("Same").expect("name is valid"),
        ProjectCreatedAt::new(10).expect("timestamp is valid"),
    );
    let before = project.clone();

    assert_eq!(
        project.rename(
            ProjectName::new(" Same ").expect("name is valid"),
            ProjectUpdatedAt::new(20).expect("timestamp is valid"),
        ),
        Err(ProjectDomainError::ProjectNameUnchanged),
    );
    assert_eq!(project, before);
}

#[test]
fn restore_rejects_update_time_before_creation() {
    assert_eq!(
        ProjectAggregate::restore(
            project_id(),
            ProjectName::new("Project").expect("name is valid"),
            ProjectRevision::initial(),
            ProjectCreatedAt::new(11).expect("timestamp is valid"),
            ProjectUpdatedAt::new(10).expect("timestamp is valid"),
        ),
        Err(ProjectDomainError::ProjectTimestampOutOfRange),
    );
}

#[test]
fn rename_overflow_leaves_the_aggregate_unchanged() {
    let mut revision_overflow = ProjectAggregate::restore(
        project_id(),
        ProjectName::new("Before").expect("name is valid"),
        ProjectRevision::from_non_zero(u64::MAX).expect("maximum is non-zero"),
        ProjectCreatedAt::new(0).expect("timestamp is valid"),
        ProjectUpdatedAt::new(0).expect("timestamp is valid"),
    )
    .expect("aggregate is valid");
    let before_revision_overflow = revision_overflow.clone();
    assert_eq!(
        revision_overflow.rename(
            ProjectName::new("After").expect("name is valid"),
            ProjectUpdatedAt::new(1).expect("timestamp is valid"),
        ),
        Err(ProjectDomainError::ProjectRevisionOverflow),
    );
    assert_eq!(revision_overflow, before_revision_overflow);

    let mut timestamp_overflow = ProjectAggregate::restore(
        project_id(),
        ProjectName::new("Before").expect("name is valid"),
        ProjectRevision::initial(),
        ProjectCreatedAt::new(i64::MAX).expect("timestamp is valid"),
        ProjectUpdatedAt::new(i64::MAX).expect("timestamp is valid"),
    )
    .expect("aggregate is valid");
    let before_timestamp_overflow = timestamp_overflow.clone();
    assert_eq!(
        timestamp_overflow.rename(
            ProjectName::new("After").expect("name is valid"),
            ProjectUpdatedAt::new(i64::MAX).expect("timestamp is valid"),
        ),
        Err(ProjectDomainError::ProjectTimestampOverflow),
    );
    assert_eq!(timestamp_overflow, before_timestamp_overflow);
}

fn project_id() -> ProjectId {
    ProjectId::from_uuid(Uuid::parse_str(PROJECT_UUID).expect("test UUID is valid"))
        .expect("test UUID is version four")
}
