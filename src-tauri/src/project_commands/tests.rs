use tempfile::tempdir;
use uuid::Uuid;

use super::*;
use crate::composition::{DesktopApplicationPaths, DesktopCompositionRoot};

#[tokio::test]
async fn activates_create_get_rename_list_and_open_with_canonical_dtos() {
    let directory = tempdir().expect("directory");
    let dependencies = DesktopCompositionRoot::compose_activated_commands(
        DesktopApplicationPaths::from_application_data_root(directory.path()),
    )
    .await
    .expect("dependencies");

    let created = project_create_with_dependencies(
        ProjectCreateRequestDto { request_id: uuid(1), name: "  First Project  ".to_owned() },
        &dependencies,
    )
    .await
    .expect("create");
    assert_eq!(created.name, "First Project");
    assert_eq!(created.revision, "1");
    assert_eq!(
        project_get_with_dependencies(
            ProjectGetRequestDto { project_id: created.id.clone() },
            &dependencies,
        )
        .await
        .expect("get"),
        created
    );

    let renamed = project_rename_with_dependencies(
        ProjectRenameRequestDto {
            request_id: uuid(2),
            project_id: created.id.clone(),
            expected_revision: created.revision.clone(),
            name: "Renamed".to_owned(),
        },
        &dependencies,
    )
    .await
    .expect("rename");
    assert_eq!(renamed.name, "Renamed");
    assert_eq!(renamed.revision, "2");

    let listed = project_list_with_dependencies(
        ProjectListRequestDto { limit: 100, cursor: None },
        &dependencies,
    )
    .await
    .expect("list");
    assert_eq!(listed.projects, vec![renamed.clone()]);
    assert_eq!(listed.next_cursor, None);

    let opened = project_open_with_dependencies(
        ProjectOpenRequestDto { project_id: renamed.id.clone() },
        &dependencies,
    )
    .await
    .expect("open");
    assert_eq!(opened.project, renamed);
    assert_eq!(opened.current_workflow_summary, None);
}

#[tokio::test]
async fn list_cursor_is_canonical_bounded_and_does_not_repeat_project_ids() {
    let directory = tempdir().expect("directory");
    let dependencies = DesktopCompositionRoot::compose_activated_commands(
        DesktopApplicationPaths::from_application_data_root(directory.path()),
    )
    .await
    .expect("dependencies");
    for seed in 10..13 {
        project_create_with_dependencies(
            ProjectCreateRequestDto { request_id: uuid(seed), name: format!("Project {seed}") },
            &dependencies,
        )
        .await
        .expect("create");
    }

    let first = project_list_with_dependencies(
        ProjectListRequestDto { limit: 2, cursor: None },
        &dependencies,
    )
    .await
    .expect("first page");
    let second = project_list_with_dependencies(
        ProjectListRequestDto { limit: 2, cursor: first.next_cursor.clone() },
        &dependencies,
    )
    .await
    .expect("second page");

    assert_eq!(first.projects.len(), 2);
    assert_eq!(second.projects.len(), 1);
    assert!(
        first
            .projects
            .iter()
            .all(|project| second.projects.iter().all(|other| other.id != project.id))
    );
    let cursor = first.next_cursor.expect("cursor");
    assert_eq!(
        project_list_with_dependencies(
            ProjectListRequestDto { limit: 2, cursor: Some(format!("{cursor}=")) },
            &dependencies,
        )
        .await
        .expect_err("padded cursor")
        .code,
        "project.invalid_request"
    );
}

#[tokio::test]
async fn rejects_noncanonical_identity_revision_and_idempotency_conflict_safely() {
    let directory = tempdir().expect("directory");
    let dependencies = DesktopCompositionRoot::compose_activated_commands(
        DesktopApplicationPaths::from_application_data_root(directory.path()),
    )
    .await
    .expect("dependencies");
    let request_id = uuid(30);
    let created = project_create_with_dependencies(
        ProjectCreateRequestDto { request_id: request_id.clone(), name: "One".to_owned() },
        &dependencies,
    )
    .await
    .expect("create");

    assert_eq!(
        project_create_with_dependencies(
            ProjectCreateRequestDto { request_id, name: "Two".to_owned() },
            &dependencies,
        )
        .await
        .expect_err("conflict")
        .code,
        "project.mutation_conflict"
    );
    assert_eq!(
        project_rename_with_dependencies(
            ProjectRenameRequestDto {
                request_id: uuid(31),
                project_id: created.id.to_uppercase(),
                expected_revision: "01".to_owned(),
                name: "Two".to_owned(),
            },
            &dependencies,
        )
        .await
        .expect_err("invalid")
        .code,
        "project.invalid_request"
    );
}

fn uuid(seed: u128) -> String {
    Uuid::from_u128(0x123e_4567_e89b_42d3_a456_4266_0000_0000 | seed).to_string()
}
