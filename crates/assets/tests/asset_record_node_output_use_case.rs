use std::io::Cursor;
use std::time::{Duration, Instant};

#[allow(dead_code)]
#[path = "support/asset_import_fixture.rs"]
mod asset_import_fixture;

use assets::asset::application::{
    AssetNodeOutputRecovery, AssetNodeOutputSourceLease, AssetRecordNodeOutputCommand,
};
use assets::asset::domain::{
    AssetDisplayName, AssetId, AssetManagedContentState, AssetMediaKind, AssetNodeOutputKey,
    AssetNodeOutputProduction, AssetOriginNodeCapabilityContractRef, AssetOriginNodeOutputKey,
    AssetOriginSourceAssetId, AssetOriginSourceAssetIds, AssetOriginWorkflowId,
    AssetOriginWorkflowNodeExecutionId, AssetOriginWorkflowNodeId, AssetOriginWorkflowRevision,
    AssetOriginWorkflowRunId, AssetWorkflowNodeOrigin,
};
use projects::project::domain::ProjectId;
use uuid::Uuid;

use asset_import_fixture::AssetImportFixtureFakeImpl;

#[tokio::test]
async fn new_node_output_commits_pending_then_finalizes_and_returns_available() {
    let fixture = AssetImportFixtureFakeImpl::new();

    let asset = fixture
        .record_node_output_use_case()
        .record_asset_node_output(record_node_output_command(10))
        .await
        .unwrap();

    assert!(matches!(asset.content_state(), AssetManagedContentState::Available { .. }));
    assert_eq!(
        fixture.events(),
        vec![
            "clock",
            "stage_node_output",
            "find_by_output_key",
            "open_staged_for_inspection",
            "inspect",
            "generate_asset_id",
            "generate_finalization_id",
            "commit_pending",
            "open_staged_for_finalization",
            "publish",
            "commit_available",
            "remove_staging",
        ]
    );
}

#[tokio::test]
async fn exact_same_key_replay_returns_existing_available_asset_without_new_identity() {
    let fixture = AssetImportFixtureFakeImpl::new();
    let first = fixture
        .record_node_output_use_case()
        .record_asset_node_output(record_node_output_command(10))
        .await
        .unwrap();
    fixture.clear_events();

    let replayed = fixture
        .record_node_output_use_case()
        .record_asset_node_output(record_node_output_command(10))
        .await
        .unwrap();

    assert_eq!(replayed.id(), first.id());
    assert_eq!(
        fixture.events(),
        vec!["clock", "stage_node_output", "find_by_output_key", "remove_staging"]
    );
}

#[tokio::test]
async fn same_key_with_different_production_returns_node_output_conflict() {
    let fixture = AssetImportFixtureFakeImpl::new();
    fixture
        .record_node_output_use_case()
        .record_asset_node_output(record_node_output_command(10))
        .await
        .unwrap();
    fixture.clear_events();

    let error = fixture
        .record_node_output_use_case()
        .record_asset_node_output(record_node_output_command(11))
        .await
        .unwrap_err();

    assert_eq!(error, assets::asset::application::AssetApplicationError::NodeOutputConflict);
    assert_eq!(
        fixture.events(),
        vec!["clock", "stage_node_output", "find_by_output_key", "remove_staging"]
    );
}

#[tokio::test]
async fn recovery_returns_available_without_staging_source_bytes() {
    let fixture = AssetImportFixtureFakeImpl::new();
    let asset = fixture
        .record_node_output_use_case()
        .record_asset_node_output(record_node_output_command(10))
        .await
        .unwrap();
    fixture.clear_events();

    let recovery = fixture
        .recover_node_output_use_case()
        .recover_asset_node_output(node_output_key())
        .await
        .unwrap();

    assert!(
        matches!(recovery, AssetNodeOutputRecovery::Available(value) if value.id() == asset.id())
    );
    assert_eq!(fixture.events(), vec!["find_by_output_key"]);
}

#[tokio::test]
async fn recovery_returns_pending_only_with_its_durable_finalization() {
    let fixture = AssetImportFixtureFakeImpl::new();
    fixture.configure_asset_content_publish_failure(true);
    let error = fixture
        .record_node_output_use_case()
        .record_asset_node_output(record_node_output_command(10))
        .await
        .unwrap_err();
    assert_eq!(error, assets::asset::application::AssetApplicationError::ManagedStorageFailed);
    fixture.clear_events();

    let recovery = fixture
        .recover_node_output_use_case()
        .recover_asset_node_output(node_output_key())
        .await
        .unwrap();

    assert!(matches!(recovery, AssetNodeOutputRecovery::Pending { .. }));
    assert_eq!(fixture.events(), vec!["find_by_output_key"]);
}

#[tokio::test]
async fn recovery_requires_source_when_key_or_durable_finalization_is_absent() {
    let empty_fixture = AssetImportFixtureFakeImpl::new();
    assert_eq!(
        empty_fixture
            .recover_node_output_use_case()
            .recover_asset_node_output(node_output_key())
            .await
            .unwrap(),
        AssetNodeOutputRecovery::SourceRequired
    );

    let pending_fixture = AssetImportFixtureFakeImpl::new();
    pending_fixture.configure_asset_content_publish_failure(true);
    let _ = pending_fixture
        .record_node_output_use_case()
        .record_asset_node_output(record_node_output_command(10))
        .await;
    pending_fixture.hide_committed_finalization();
    assert_eq!(
        pending_fixture
            .recover_node_output_use_case()
            .recover_asset_node_output(node_output_key())
            .await
            .unwrap(),
        AssetNodeOutputRecovery::SourceRequired
    );
}

fn record_node_output_command(source_asset_seed: u8) -> AssetRecordNodeOutputCommand {
    let producer = AssetWorkflowNodeOrigin::new(
        AssetOriginWorkflowId::from_uuid(uuid(6)).unwrap(),
        AssetOriginWorkflowRevision::new(1).unwrap(),
        AssetOriginWorkflowRunId::from_uuid(uuid(7)).unwrap(),
        AssetOriginWorkflowNodeId::from_uuid(uuid(8)).unwrap(),
        AssetOriginWorkflowNodeExecutionId::from_uuid(uuid(9)).unwrap(),
        AssetOriginNodeCapabilityContractRef::try_new("image.generate", 1, 0).unwrap(),
    );
    let output_key = node_output_key();
    let production = AssetNodeOutputProduction::DeterministicDerived {
        source_asset_ids: AssetOriginSourceAssetIds::try_new(vec![
            AssetOriginSourceAssetId::from_asset_id(
                AssetId::from_uuid(uuid(source_asset_seed)).unwrap(),
            ),
        ])
        .unwrap(),
    };
    AssetRecordNodeOutputCommand::try_new(
        ProjectId::from_uuid(uuid(2)).unwrap(),
        AssetMediaKind::Image,
        AssetDisplayName::try_new("generated image").unwrap(),
        producer,
        production,
        output_key,
        AssetNodeOutputSourceLease::new(
            Instant::now() + Duration::from_secs(60),
            Box::pin(Cursor::new(vec![1; 10])),
        ),
    )
    .unwrap()
}

fn node_output_key() -> AssetNodeOutputKey {
    AssetNodeOutputKey::new(
        AssetOriginWorkflowRunId::from_uuid(uuid(7)).unwrap(),
        AssetOriginWorkflowNodeExecutionId::from_uuid(uuid(9)).unwrap(),
        AssetOriginNodeOutputKey::try_new("image").unwrap(),
        0,
    )
}

fn uuid(seed: u8) -> Uuid {
    Uuid::from_bytes([seed, 0, 0, 0, 0, 0, 0x40, 0, 0x80, 0, 0, 0, 0, 0, 0, seed])
}
