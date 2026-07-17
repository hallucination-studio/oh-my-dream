use assets::asset::application::AssetApplicationError;
use assets::asset::domain::AssetManagedContentState;

use super::{FinalizationFailurePoint, FinalizationFixtureFakeImpl, command};

#[tokio::test]
async fn staged_open_failure_propagates_and_keeps_pending() {
    assert_pending_failure(
        FinalizationFixtureFakeImpl::new(true, false),
        FinalizationFailurePoint::OpenStaged,
        AssetApplicationError::ManagedStorageFailed,
    )
    .await;
}

#[tokio::test]
async fn availability_commit_failure_propagates_and_keeps_pending() {
    assert_pending_failure(
        FinalizationFixtureFakeImpl::new(true, false),
        FinalizationFailurePoint::CommitAvailable,
        AssetApplicationError::IdentityConflict,
    )
    .await;
}

#[tokio::test]
async fn managed_verification_failure_propagates_and_keeps_pending() {
    assert_pending_failure(
        FinalizationFixtureFakeImpl::new(false, false),
        FinalizationFailurePoint::VerifyManaged,
        AssetApplicationError::ManagedStorageFailed,
    )
    .await;
}

#[tokio::test]
async fn missing_commit_failure_propagates_and_keeps_pending() {
    assert_pending_failure(
        FinalizationFixtureFakeImpl::new(false, false),
        FinalizationFailurePoint::CommitMissing,
        AssetApplicationError::IdentityConflict,
    )
    .await;
}

#[tokio::test]
async fn cleanup_failure_after_available_commit_does_not_replace_success() {
    let fixture = FinalizationFixtureFakeImpl::new(true, false);
    fixture.inject_finalization_failure_at(FinalizationFailurePoint::RemoveStaging);

    let asset = fixture.use_case().finalize_asset_content(command()).await.unwrap();

    assert!(matches!(asset.content_state(), AssetManagedContentState::Available { .. }));
}

async fn assert_pending_failure(
    fixture: std::sync::Arc<FinalizationFixtureFakeImpl>,
    failure_point: FinalizationFailurePoint,
    expected_error: AssetApplicationError,
) {
    fixture.inject_finalization_failure_at(failure_point);
    let error = fixture.use_case().finalize_asset_content(command()).await.unwrap_err();
    assert_eq!(error, expected_error);
    assert!(fixture.pending_content_remains());
}
