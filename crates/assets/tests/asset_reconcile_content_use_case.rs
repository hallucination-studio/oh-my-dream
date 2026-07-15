#[path = "support/asset_reconcile_fixture.rs"]
mod asset_reconcile_fixture;

use assets::asset::application::AssetApplicationError;
use assets::asset::domain::AssetManagedContentState;

use asset_reconcile_fixture::AssetReconcileFixtureFakeImpl;

#[tokio::test]
async fn reconciliation_processes_finalization_available_and_staging_in_order() {
    let fixture = AssetReconcileFixtureFakeImpl::new();

    let result = fixture
        .reconcile_use_case()
        .reconcile_asset_content(fixture.reconcile_command())
        .await
        .unwrap();

    assert!(result.finalization_cursor().is_none());
    assert!(result.available_content_cursor().is_none());
    assert!(result.staged_content_cursor().is_none());
    assert!(matches!(
        fixture.pending_asset().content_state(),
        AssetManagedContentState::Missing { .. }
    ));
    assert!(matches!(
        fixture.available_asset().content_state(),
        AssetManagedContentState::Missing { .. }
    ));
    assert_eq!(fixture.stale_cutoff_utc_milliseconds(), Some(13_600_000));
    assert_eq!(
        fixture.events(),
        vec![
            "clock",
            "list_finalizations",
            "find_finalization",
            "find_finalization_asset",
            "open_finalization_staging",
            "verify_finalization_managed",
            "commit_finalization_missing",
            "list_available",
            "verify_available_managed",
            "commit_available_missing",
            "list_stale_staging",
            "check_staging_reference",
            "remove_stale_staging",
        ]
    );
}

#[tokio::test]
async fn referenced_stale_staging_is_not_removed() {
    let fixture = AssetReconcileFixtureFakeImpl::new();
    fixture.mark_staging_referenced();

    fixture
        .reconcile_use_case()
        .reconcile_asset_content(fixture.reconcile_command())
        .await
        .unwrap();

    assert!(!fixture.events().contains(&"remove_stale_staging"));
}

#[tokio::test]
async fn available_verification_failure_stops_before_stale_staging() {
    let fixture = AssetReconcileFixtureFakeImpl::new();
    fixture.fail_available_verification();

    let result =
        fixture.reconcile_use_case().reconcile_asset_content(fixture.reconcile_command()).await;

    assert_eq!(result.unwrap_err(), AssetApplicationError::ManagedStorageFailed);
    assert!(!fixture.events().contains(&"list_stale_staging"));
}
