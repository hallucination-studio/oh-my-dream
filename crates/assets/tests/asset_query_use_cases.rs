use std::sync::Arc;
use std::time::{Duration, Instant};

#[allow(dead_code)]
#[path = "support/asset_import_fixture.rs"]
mod asset_import_fixture;

use assets::asset::application::{
    AssetApplicationError, AssetGetQuery, AssetGetUseCase, AssetIssuePreviewCommand,
    AssetIssuePreviewUseCase, AssetListQuery, AssetListUseCase, AssetPageLimit,
    AssetResolveContentQuery, AssetResolveContentUseCase,
};
use assets::asset::domain::AssetMediaKind;

use asset_import_fixture::AssetImportFixtureFakeImpl;

#[tokio::test]
async fn get_returns_the_project_visible_asset_without_hiding_state() {
    let fixture = available_asset_fixture().await;
    let expected = fixture.committed_asset().unwrap();
    let use_case = AssetGetUseCase::new(fixture.clone());

    let asset =
        use_case.get_asset(AssetGetQuery::new(expected.project_id(), expected.id())).await.unwrap();

    assert_eq!(asset, expected);
}

#[tokio::test]
async fn list_returns_the_repository_page_without_projection() {
    let fixture = AssetImportFixtureFakeImpl::new();
    let use_case = AssetListUseCase::new(fixture.clone());
    let query = AssetListQuery::new(
        projects::project::domain::ProjectId::from_uuid(uuid::Uuid::from_bytes([
            1, 0, 0, 0, 0, 0, 0x40, 0, 0x80, 0, 0, 0, 0, 0, 0, 1,
        ]))
        .unwrap(),
        Some(AssetMediaKind::Image),
        None,
        AssetPageLimit::from_u16(10).unwrap(),
    );

    let page = use_case.list_assets(query).await.unwrap();

    assert!(page.assets().is_empty());
    assert!(page.next_cursor().is_none());
}

#[tokio::test]
async fn resolve_returns_exact_descriptor_and_managed_content_lease() {
    let fixture = available_asset_fixture().await;
    let expected = fixture.committed_asset().unwrap();
    let use_case = AssetResolveContentUseCase::new(fixture.clone(), fixture.clone());

    let resolved = use_case
        .resolve_asset_content(AssetResolveContentQuery::new(
            expected.project_id(),
            expected.id(),
            AssetMediaKind::Image,
            Instant::now() + Duration::from_secs(60),
        ))
        .await
        .unwrap();

    assert_eq!(resolved.descriptor(), expected.content_state().descriptor());
    assert_eq!(resolved.content_lease().content_id(), resolved.descriptor().content_id());
}

#[tokio::test]
async fn preview_uses_the_available_asset_exact_content_identity() {
    let fixture = available_asset_fixture().await;
    let expected = fixture.committed_asset().unwrap();
    let use_case = AssetIssuePreviewUseCase::new(fixture.clone(), fixture.clone(), fixture.clone());

    let lease = use_case
        .issue_asset_preview(AssetIssuePreviewCommand::new(expected.project_id(), expected.id()))
        .await
        .unwrap();

    assert_eq!(lease.project_id(), expected.project_id());
    assert_eq!(lease.asset_id(), expected.id());
    assert_eq!(lease.content_id(), expected.content_state().descriptor().content_id());
}

#[tokio::test]
async fn get_rejects_an_asset_owned_by_a_different_project() {
    let fixture = available_asset_fixture().await;
    let expected = fixture.committed_asset().unwrap();
    let other_project = projects::project::domain::ProjectId::from_uuid(uuid::Uuid::from_bytes([
        12, 0, 0, 0, 0, 0, 0x40, 0, 0x80, 0, 0, 0, 0, 0, 0, 12,
    ]))
    .unwrap();

    let error = AssetGetUseCase::new(fixture.clone())
        .get_asset(AssetGetQuery::new(other_project, expected.id()))
        .await
        .unwrap_err();

    assert_eq!(error, AssetApplicationError::NotVisible);
}

#[tokio::test]
async fn resolve_rejects_media_kind_before_opening_content() {
    let fixture = available_asset_fixture().await;
    let expected = fixture.committed_asset().unwrap();

    let result = AssetResolveContentUseCase::new(fixture.clone(), fixture.clone())
        .resolve_asset_content(AssetResolveContentQuery::new(
            expected.project_id(),
            expected.id(),
            AssetMediaKind::Video,
            Instant::now() + Duration::from_secs(60),
        ))
        .await;
    let error = match result {
        Ok(_) => panic!("wrong media kind unexpectedly resolved content"),
        Err(error) => error,
    };

    assert_eq!(
        error,
        AssetApplicationError::MediaKindMismatch {
            expected: AssetMediaKind::Video,
            observed: AssetMediaKind::Image,
        }
    );
}

#[tokio::test]
async fn preview_rejects_pending_content_before_clock_and_identity_generation() {
    let fixture = AssetImportFixtureFakeImpl::new();
    fixture.configure_asset_content_publish_failure(true);
    let pending = fixture.import_use_case().import_asset(fixture.import_command()).await.unwrap();
    fixture.clear_events();

    let error = AssetIssuePreviewUseCase::new(fixture.clone(), fixture.clone(), fixture.clone())
        .issue_asset_preview(AssetIssuePreviewCommand::new(pending.project_id(), pending.id()))
        .await
        .unwrap_err();

    assert_eq!(error, AssetApplicationError::ContentPending);
    assert_eq!(fixture.events(), Vec::<&'static str>::new());
}

async fn available_asset_fixture() -> Arc<AssetImportFixtureFakeImpl> {
    let fixture = AssetImportFixtureFakeImpl::new();
    fixture.import_use_case().import_asset(fixture.import_command()).await.unwrap();
    fixture
}
