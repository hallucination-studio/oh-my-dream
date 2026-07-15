use assets::asset::application::{
    AssetApplicationError, AssetCommitContentMissingCommand,
    AssetCommitFinalizedContentAvailableCommand, AssetCommitPendingContentCommand,
    AssetContentFinalization, AssetFinalizeContentEffect, AssetInspectedMedia, AssetPageLimit,
    AssetStagedContent, AssetStagedContentRef,
};
use assets::asset::domain::{
    AssetAggregate, AssetContentDescriptor, AssetContentDigest, AssetContentFinalizationId,
    AssetContentMissingReason, AssetCreatedAt, AssetDisplayName, AssetId, AssetImportId,
    AssetManagedContentId, AssetMediaFacts, AssetMediaKind, AssetMediaMimeType, AssetOrigin,
    AssetOriginalFileName,
};
use projects::project::domain::ProjectId;
use uuid::Uuid;

#[test]
fn staged_content_reference_accepts_only_bounded_opaque_bytes() {
    assert!(matches!(
        AssetStagedContentRef::try_from_store_bytes(Vec::new()),
        Err(AssetApplicationError::ManagedStorageFailed)
    ));
    assert!(matches!(
        AssetStagedContentRef::try_from_store_bytes(vec![0; 513]),
        Err(AssetApplicationError::ManagedStorageFailed)
    ));

    let reference = AssetStagedContentRef::try_from_store_bytes(vec![0, 1, 255]).unwrap();
    assert_eq!(reference.as_store_bytes(), &[0, 1, 255]);
}

#[test]
fn staged_content_rejects_an_empty_observation() {
    assert!(matches!(
        AssetStagedContent::try_new(
            AssetStagedContentRef::try_from_store_bytes(vec![1]).unwrap(),
            AssetContentDigest::from_bytes([1; 32]),
            0,
            created_at(),
        ),
        Err(AssetApplicationError::InvalidMedia)
    ));
}

#[test]
fn asset_page_limit_accepts_only_one_through_one_hundred() {
    assert!(AssetPageLimit::from_u16(0).is_none());
    assert_eq!(AssetPageLimit::from_u16(1).unwrap().get(), 1);
    assert_eq!(AssetPageLimit::from_u16(100).unwrap().get(), 100);
    assert!(AssetPageLimit::from_u16(101).is_none());
}

#[test]
fn inspected_media_requires_mime_and_facts_to_have_the_same_kind() {
    let image_facts = AssetMediaFacts::try_image(32, 32).unwrap();
    assert!(AssetInspectedMedia::try_new(AssetMediaMimeType::ImagePng, image_facts).is_ok());
    assert!(matches!(
        AssetInspectedMedia::try_new(AssetMediaMimeType::VideoMp4, image_facts),
        Err(AssetApplicationError::InvalidMedia)
    ));
}

#[test]
fn pending_commit_command_requires_all_durable_identities_to_agree() {
    let asset = pending_asset();
    let finalization = content_finalization(finalization_id(4));

    assert!(
        AssetCommitPendingContentCommand::try_new(
            asset.clone(),
            finalization.clone(),
            AssetFinalizeContentEffect::new(finalization_id(4)),
        )
        .is_ok()
    );
    assert!(matches!(
        AssetCommitPendingContentCommand::try_new(
            asset,
            finalization,
            AssetFinalizeContentEffect::new(finalization_id(9)),
        ),
        Err(AssetApplicationError::IdentityConflict)
    ));
}

#[test]
fn transition_commands_accept_only_the_matching_approved_state() {
    let mut available = pending_asset();
    available.mark_pending_content_available(finalization_id(4)).unwrap();
    assert!(
        AssetCommitFinalizedContentAvailableCommand::try_new(available, finalization_id(4)).is_ok()
    );
    assert!(matches!(
        AssetCommitFinalizedContentAvailableCommand::try_new(pending_asset(), finalization_id(4)),
        Err(AssetApplicationError::IdentityConflict)
    ));

    let mut missing = pending_asset();
    missing.mark_content_missing(AssetContentMissingReason::FinalizationSourceMissing).unwrap();
    assert!(
        AssetCommitContentMissingCommand::try_new(missing.clone(), Some(finalization_id(4)))
            .is_ok()
    );
    assert!(matches!(
        AssetCommitContentMissingCommand::try_new(missing, None),
        Err(AssetApplicationError::IdentityConflict)
    ));
}

fn pending_asset() -> AssetAggregate {
    AssetAggregate::try_new_pending(
        asset_id(1),
        project_id(2),
        AssetMediaKind::Image,
        descriptor(),
        finalization_id(4),
        AssetMediaFacts::try_image(32, 32).unwrap(),
        AssetOrigin::imported(
            AssetImportId::from_uuid(uuid(5)).unwrap(),
            AssetOriginalFileName::try_new("image.png").unwrap(),
        ),
        AssetDisplayName::try_new("image").unwrap(),
        created_at(),
    )
    .unwrap()
}

fn content_finalization(finalization_id: AssetContentFinalizationId) -> AssetContentFinalization {
    AssetContentFinalization::new(
        finalization_id,
        asset_id(1),
        descriptor(),
        AssetStagedContentRef::try_from_store_bytes(vec![1]).unwrap(),
        created_at(),
    )
}

fn descriptor() -> AssetContentDescriptor {
    let digest = AssetContentDigest::from_bytes([3; 32]);
    AssetContentDescriptor::try_new(
        AssetManagedContentId::from_digest(digest),
        digest,
        10,
        AssetMediaMimeType::ImagePng,
        AssetMediaKind::Image,
    )
    .unwrap()
}

fn created_at() -> AssetCreatedAt {
    AssetCreatedAt::from_utc_milliseconds(10).unwrap()
}

fn asset_id(seed: u8) -> AssetId {
    AssetId::from_uuid(uuid(seed)).unwrap()
}

fn project_id(seed: u8) -> ProjectId {
    ProjectId::from_uuid(uuid(seed)).unwrap()
}

fn finalization_id(seed: u8) -> AssetContentFinalizationId {
    AssetContentFinalizationId::from_uuid(uuid(seed)).unwrap()
}

fn uuid(seed: u8) -> Uuid {
    Uuid::from_bytes([seed, 0, 0, 0, 0, 0, 0x40, 0, 0x80, 0, 0, 0, 0, 0, 0, seed])
}
