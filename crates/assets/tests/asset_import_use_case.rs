#[path = "support/asset_import_fixture.rs"]
mod asset_import_fixture;

use assets::asset::application::AssetApplicationError;
use assets::asset::application::{AssetFinalizeContentCommand, AssetFinalizeContentEffect};
use assets::asset::domain::AssetManagedContentState;

use asset_import_fixture::AssetImportFixtureFakeImpl;

#[tokio::test]
async fn imported_content_commits_pending_before_inline_publication_and_returns_available() {
    let fixture = AssetImportFixtureFakeImpl::new();

    let asset = fixture.import_use_case().import_asset(fixture.import_command()).await.unwrap();

    assert!(matches!(asset.content_state(), AssetManagedContentState::Available { .. }));
    assert_eq!(
        fixture.events(),
        vec![
            "clock",
            "generate_asset_id",
            "generate_import_id",
            "generate_finalization_id",
            "stage",
            "open_staged_for_inspection",
            "inspect",
            "commit_pending",
            "open_staged_for_finalization",
            "publish",
            "commit_available",
            "remove_staging",
        ]
    );
}

#[tokio::test]
async fn inspection_failure_removes_staging_without_committing_an_asset() {
    let fixture = AssetImportFixtureFakeImpl::new();
    fixture.fail_inspection();

    let result = fixture.import_use_case().import_asset(fixture.import_command()).await;

    assert_eq!(result.unwrap_err(), AssetApplicationError::InspectionFailed);
    assert_eq!(
        fixture.events(),
        vec![
            "clock",
            "generate_asset_id",
            "generate_import_id",
            "generate_finalization_id",
            "stage",
            "open_staged_for_inspection",
            "inspect",
            "remove_staging",
        ]
    );
    assert!(fixture.committed_asset().is_none());
}

#[tokio::test]
async fn inline_publish_failure_returns_the_durable_pending_asset() {
    let fixture = AssetImportFixtureFakeImpl::new();
    fixture.configure_asset_content_publish_failure(true);

    let asset = fixture.import_use_case().import_asset(fixture.import_command()).await.unwrap();

    assert!(matches!(asset.content_state(), AssetManagedContentState::Pending { .. }));
    assert!(matches!(
        fixture.committed_asset().unwrap().content_state(),
        AssetManagedContentState::Pending { .. }
    ));
    assert_eq!(
        fixture.events(),
        vec![
            "clock",
            "generate_asset_id",
            "generate_import_id",
            "generate_finalization_id",
            "stage",
            "open_staged_for_inspection",
            "inspect",
            "commit_pending",
            "open_staged_for_finalization",
            "publish",
        ]
    );
}

#[tokio::test]
async fn inline_contract_failure_propagates_while_pending_remains_durable() {
    let fixture = AssetImportFixtureFakeImpl::new();
    fixture.hide_committed_finalization();

    let result = fixture.import_use_case().import_asset(fixture.import_command()).await;

    assert_eq!(result.unwrap_err(), AssetApplicationError::NotFound);
    assert!(matches!(
        fixture.committed_asset().unwrap().content_state(),
        AssetManagedContentState::Pending { .. }
    ));
    assert_eq!(
        fixture.events(),
        vec![
            "clock",
            "generate_asset_id",
            "generate_import_id",
            "generate_finalization_id",
            "stage",
            "open_staged_for_inspection",
            "inspect",
            "commit_pending",
        ]
    );
}

#[tokio::test]
async fn equal_byte_imports_create_different_logical_asset_identities() {
    let fixture = AssetImportFixtureFakeImpl::new();

    let first = fixture.import_use_case().import_asset(fixture.import_command()).await.unwrap();
    let second = fixture.import_use_case().import_asset(fixture.import_command()).await.unwrap();

    assert_eq!(
        first.content_state().descriptor().digest(),
        second.content_state().descriptor().digest()
    );
    assert_ne!(first.id(), second.id());
}

#[tokio::test]
async fn pending_import_becomes_available_when_its_exact_publication_is_replayed() {
    let fixture = AssetImportFixtureFakeImpl::new();
    fixture.configure_asset_content_publish_failure(true);
    let pending = fixture.import_use_case().import_asset(fixture.import_command()).await.unwrap();
    let finalization_id = match pending.content_state() {
        AssetManagedContentState::Pending { finalization_id, .. } => *finalization_id,
        _ => panic!("failed publication did not leave Pending content"),
    };
    fixture.configure_asset_content_publish_failure(false);

    let available = fixture
        .finalize_content_use_case()
        .finalize_asset_content(AssetFinalizeContentCommand::new(
            AssetFinalizeContentEffect::new(finalization_id),
            std::time::Instant::now() + std::time::Duration::from_secs(60),
        ))
        .await
        .unwrap();

    assert!(matches!(available.content_state(), AssetManagedContentState::Available { .. }));
}
