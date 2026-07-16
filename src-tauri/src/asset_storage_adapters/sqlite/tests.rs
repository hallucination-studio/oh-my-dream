use assets::asset::interfaces::{AssetIngestTransactionInterface, AssetRepositoryInterface};
use projects::project::domain::ProjectId;
use uuid::Uuid;

use super::*;
use crate::post_commit_effect::{
    DesktopApplicationInstanceId, DesktopPostCommitEffect, DesktopPostCommitEffectOutboxInterface,
    SqliteDesktopPostCommitEffectOutboxAdapterImpl,
};

#[tokio::test]
async fn imported_pending_commit_is_atomic_queryable_and_finalizable() {
    let connection = Arc::new(Mutex::new(Connection::open_in_memory().unwrap()));
    let outbox =
        SqliteDesktopPostCommitEffectOutboxAdapterImpl::try_new(Arc::clone(&connection)).unwrap();
    let repository = SqliteAssetRepositoryAdapterImpl::try_new(Arc::clone(&connection)).unwrap();
    let ingest = SqliteAssetIngestTransactionAdapterImpl::try_new(Arc::clone(&connection)).unwrap();
    let pending = imported_pending(1, 2, 3);
    let command = pending_command(pending.clone(), 3);
    ingest.commit_imported_pending_asset(command.clone()).await.unwrap();
    assert_eq!(repository.find_asset_by_id(pending.id()).await.unwrap(), Some(pending.clone()));
    assert_eq!(
        repository.find_asset_content_finalization(finalization_id(3)).await.unwrap(),
        Some(command.finalization().clone())
    );
    let claimed = outbox
        .claim_next_post_commit_effect(instance_id(9), post_commit_time(20))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(claimed.effect(), DesktopPostCommitEffect::Asset(command.effect()));

    let mut available = pending;
    available.mark_pending_content_available(finalization_id(3)).unwrap();
    ingest
        .commit_finalized_asset_content_available(
            AssetCommitFinalizedContentAvailableCommand::try_new(
                available.clone(),
                finalization_id(3),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(repository.find_asset_by_id(available.id()).await.unwrap(), Some(available.clone()));
    assert_eq!(
        repository
            .list_available_assets_for_content_verification(
                None,
                AssetPageLimit::from_u16(10).unwrap(),
            )
            .await
            .unwrap()
            .assets(),
        &[available]
    );
}

#[tokio::test]
async fn node_output_binding_returns_existing_asset_without_partial_second_commit() {
    let connection = Arc::new(Mutex::new(Connection::open_in_memory().unwrap()));
    SqliteDesktopPostCommitEffectOutboxAdapterImpl::try_new(Arc::clone(&connection)).unwrap();
    let repository = SqliteAssetRepositoryAdapterImpl::try_new(Arc::clone(&connection)).unwrap();
    let ingest = SqliteAssetIngestTransactionAdapterImpl::try_new(connection).unwrap();
    let first = node_pending(1, 3);
    assert_eq!(
        ingest
            .commit_workflow_node_output_pending_asset(pending_command(first.clone(), 3))
            .await
            .unwrap(),
        AssetCommitWorkflowNodeOutputPendingResult::Committed
    );
    let second = node_pending(8, 9);
    assert_eq!(
        ingest
            .commit_workflow_node_output_pending_asset(pending_command(second.clone(), 9))
            .await
            .unwrap(),
        AssetCommitWorkflowNodeOutputPendingResult::OutputKeyAlreadyBound {
            asset: Box::new(first.clone())
        }
    );
    let AssetOrigin::WorkflowNodeOutput(origin) = first.origin() else {
        panic!("node origin");
    };
    assert_eq!(
        repository.find_asset_by_node_output_key(origin.output_key().clone()).await.unwrap(),
        Some(first)
    );
    assert!(repository.find_asset_by_id(second.id()).await.unwrap().is_none());
}

#[tokio::test]
async fn repository_pages_stably_and_missing_transitions_complete_only_required_finalizations() {
    let connection = Arc::new(Mutex::new(Connection::open_in_memory().unwrap()));
    SqliteDesktopPostCommitEffectOutboxAdapterImpl::try_new(Arc::clone(&connection)).unwrap();
    let repository = SqliteAssetRepositoryAdapterImpl::try_new(Arc::clone(&connection)).unwrap();
    let ingest = SqliteAssetIngestTransactionAdapterImpl::try_new(connection).unwrap();
    let first = imported_pending(1, 2, 3);
    let second = imported_pending(4, 2, 5);
    ingest.commit_imported_pending_asset(pending_command(first.clone(), 3)).await.unwrap();
    ingest.commit_imported_pending_asset(pending_command(second.clone(), 5)).await.unwrap();

    let first_page = repository
        .list_project_assets(AssetListQuery::new(
            project_id(2),
            None,
            None,
            AssetPageLimit::from_u16(1).unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(first_page.assets(), std::slice::from_ref(&second));
    let second_page = repository
        .list_project_assets(AssetListQuery::new(
            project_id(2),
            None,
            first_page.next_cursor(),
            AssetPageLimit::from_u16(1).unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(second_page.assets(), std::slice::from_ref(&first));

    let finalization_page = repository
        .list_unfinished_asset_content_finalizations(None, AssetPageLimit::from_u16(1).unwrap())
        .await
        .unwrap();
    assert_eq!(finalization_page.finalizations().len(), 1);
    assert!(finalization_page.next_cursor().is_some());
    assert!(
        repository
            .is_asset_staged_content_referenced(
                AssetStagedContentRef::try_from_store_bytes(vec![3]).unwrap(),
            )
            .await
            .unwrap()
    );

    let mut missing = first.clone();
    missing.mark_content_missing(AssetContentMissingReason::FinalizationSourceMissing).unwrap();
    ingest
        .commit_asset_content_missing(
            AssetCommitContentMissingCommand::try_new(missing.clone(), Some(finalization_id(3)))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(repository.find_asset_by_id(first.id()).await.unwrap(), Some(missing));
    assert!(
        !repository
            .is_asset_staged_content_referenced(
                AssetStagedContentRef::try_from_store_bytes(vec![3]).unwrap(),
            )
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn pending_commit_rolls_back_when_shared_outbox_write_fails() {
    let connection = Arc::new(Mutex::new(Connection::open_in_memory().unwrap()));
    let repository = SqliteAssetRepositoryAdapterImpl::try_new(Arc::clone(&connection)).unwrap();
    let ingest = SqliteAssetIngestTransactionAdapterImpl::try_new(connection).unwrap();
    let pending = imported_pending(1, 2, 3);
    assert_eq!(
        ingest.commit_imported_pending_asset(pending_command(pending.clone(), 3)).await,
        Err(AssetApplicationError::ManagedStorageFailed)
    );
    assert!(repository.find_asset_by_id(pending.id()).await.unwrap().is_none());
    assert!(
        repository.find_asset_content_finalization(finalization_id(3)).await.unwrap().is_none()
    );
}

fn pending_command(
    asset: AssetAggregate,
    finalization_seed: u8,
) -> AssetCommitPendingContentCommand {
    let finalization = AssetContentFinalization::new(
        finalization_id(finalization_seed),
        asset.id(),
        asset.content_state().descriptor().clone(),
        AssetStagedContentRef::try_from_store_bytes(vec![finalization_seed]).unwrap(),
        asset.created_at(),
    );
    AssetCommitPendingContentCommand::try_new(
        asset,
        finalization,
        AssetFinalizeContentEffect::new(finalization_id(finalization_seed)),
    )
    .unwrap()
}

fn imported_pending(asset_seed: u8, project_seed: u8, finalization_seed: u8) -> AssetAggregate {
    AssetAggregate::try_new_pending(
        asset_id(asset_seed),
        project_id(project_seed),
        AssetMediaKind::Image,
        descriptor(asset_seed),
        finalization_id(finalization_seed),
        AssetMediaFacts::try_image(32, 32).unwrap(),
        AssetOrigin::imported(
            AssetImportId::from_uuid(uuid(asset_seed + 20)).unwrap(),
            AssetOriginalFileName::try_new("image.png").unwrap(),
        ),
        AssetDisplayName::try_new("image").unwrap(),
        created_at(asset_seed as i64),
    )
    .unwrap()
}

fn node_pending(asset_seed: u8, finalization_seed: u8) -> AssetAggregate {
    let run_id = AssetOriginWorkflowRunId::from_uuid(uuid(30)).unwrap();
    let execution_id = AssetOriginWorkflowNodeExecutionId::from_uuid(uuid(31)).unwrap();
    let output_key = AssetNodeOutputKey::new(
        run_id,
        execution_id,
        AssetOriginNodeOutputKey::try_new("image").unwrap(),
        0,
    );
    let producer = AssetWorkflowNodeOrigin::new(
        AssetOriginWorkflowId::from_uuid(uuid(32)).unwrap(),
        AssetOriginWorkflowRevision::new(1).unwrap(),
        run_id,
        AssetOriginWorkflowNodeId::from_uuid(uuid(33)).unwrap(),
        execution_id,
        AssetOriginNodeCapabilityContractRef::try_new("image.generate_from_text", 1, 0).unwrap(),
    );
    AssetAggregate::try_new_pending(
        asset_id(asset_seed),
        project_id(2),
        AssetMediaKind::Image,
        descriptor(asset_seed),
        finalization_id(finalization_seed),
        AssetMediaFacts::try_image(32, 32).unwrap(),
        AssetOrigin::workflow_node_output(
            producer,
            AssetNodeOutputProduction::ProviderGenerated {
                generation_profile_ref: AssetOriginGenerationProfileRef::try_new(
                    "image.high_quality_general",
                    1,
                )
                .unwrap(),
            },
            output_key,
        )
        .unwrap(),
        AssetDisplayName::try_new("generated image").unwrap(),
        created_at(asset_seed as i64),
    )
    .unwrap()
}

fn descriptor(seed: u8) -> AssetContentDescriptor {
    let digest = AssetContentDigest::from_bytes([seed; 32]);
    AssetContentDescriptor::try_new(
        AssetManagedContentId::from_digest(digest),
        digest,
        10,
        AssetMediaMimeType::ImagePng,
        AssetMediaKind::Image,
    )
    .unwrap()
}

fn asset_id(seed: u8) -> AssetId {
    AssetId::from_uuid(uuid(seed)).unwrap()
}
fn finalization_id(seed: u8) -> AssetContentFinalizationId {
    AssetContentFinalizationId::from_uuid(uuid(seed)).unwrap()
}
fn project_id(seed: u8) -> ProjectId {
    ProjectId::from_uuid(uuid(seed)).unwrap()
}
fn created_at(value: i64) -> AssetCreatedAt {
    AssetCreatedAt::from_utc_milliseconds(value).unwrap()
}
fn instance_id(seed: u8) -> DesktopApplicationInstanceId {
    DesktopApplicationInstanceId::from_uuid(uuid(seed)).unwrap()
}
fn post_commit_time(value: i64) -> DesktopPostCommitTimestamp {
    DesktopPostCommitTimestamp::from_epoch_millis(value).unwrap()
}
fn uuid(seed: u8) -> Uuid {
    Uuid::from_bytes([seed, 0, 0, 0, 0, 0, 0x40, 0, 0x80, 0, 0, 0, 0, 0, 0, seed])
}
