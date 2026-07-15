use assets::asset::domain::{
    AssetAggregate, AssetContentDescriptor, AssetContentDigest, AssetContentFinalizationId,
    AssetContentMissingReason, AssetCreatedAt, AssetDisplayName, AssetDomainError, AssetId,
    AssetImportId, AssetManagedContentId, AssetManagedContentState, AssetMediaFacts,
    AssetMediaKind, AssetMediaMimeType, AssetOrigin, AssetOriginalFileName,
};
use projects::project::domain::ProjectId;
use uuid::Uuid;

#[test]
fn pending_asset_requires_descriptor_facts_and_outer_kind_to_agree() {
    let result = AssetAggregate::try_new_pending(
        asset_id(1),
        project_id(2),
        AssetMediaKind::Image,
        descriptor(3, AssetMediaKind::Image),
        finalization_id(4),
        AssetMediaFacts::try_video(1920, 1080, 1000, false).unwrap(),
        imported_origin(),
        AssetDisplayName::try_new("image").unwrap(),
        AssetCreatedAt::from_utc_milliseconds(10).unwrap(),
    );
    assert_eq!(result.unwrap_err(), AssetDomainError::InvalidMediaFacts);
}

#[test]
fn pending_to_available_requires_the_exact_finalization_identity() {
    let mut aggregate = pending_asset();
    let before = aggregate.clone();
    assert_eq!(
        aggregate.mark_pending_content_available(finalization_id(9)).unwrap_err(),
        AssetDomainError::FinalizationIdentityMismatch
    );
    assert_eq!(aggregate, before);

    aggregate.mark_pending_content_available(finalization_id(4)).unwrap();
    assert!(matches!(aggregate.content_state(), AssetManagedContentState::Available { .. }));
}

#[test]
fn available_can_become_missing_and_only_the_exact_expected_content_can_return() {
    let mut aggregate = pending_asset();
    aggregate.mark_pending_content_available(finalization_id(4)).unwrap();
    aggregate.mark_content_missing(AssetContentMissingReason::ManagedContentMissing).unwrap();
    assert!(matches!(aggregate.content_state(), AssetManagedContentState::Missing { .. }));

    assert_eq!(
        aggregate.restore_missing_content(descriptor(8, AssetMediaKind::Image)).unwrap_err(),
        AssetDomainError::InvalidTransition
    );
    aggregate.restore_missing_content(descriptor(3, AssetMediaKind::Image)).unwrap();
    assert!(matches!(aggregate.content_state(), AssetManagedContentState::Available { .. }));
}

#[test]
fn pending_can_become_missing_but_missing_cannot_transition_to_missing_again() {
    let mut aggregate = pending_asset();
    aggregate.mark_content_missing(AssetContentMissingReason::FinalizationSourceMissing).unwrap();
    assert!(matches!(
        aggregate.content_state(),
        AssetManagedContentState::Missing {
            reason: AssetContentMissingReason::FinalizationSourceMissing,
            ..
        }
    ));
    assert_eq!(
        aggregate
            .mark_content_missing(AssetContentMissingReason::ManagedContentMissing)
            .unwrap_err(),
        AssetDomainError::InvalidTransition
    );
}

fn pending_asset() -> AssetAggregate {
    AssetAggregate::try_new_pending(
        asset_id(1),
        project_id(2),
        AssetMediaKind::Image,
        descriptor(3, AssetMediaKind::Image),
        finalization_id(4),
        AssetMediaFacts::try_image(1024, 768).unwrap(),
        imported_origin(),
        AssetDisplayName::try_new("image").unwrap(),
        AssetCreatedAt::from_utc_milliseconds(10).unwrap(),
    )
    .unwrap()
}

fn descriptor(seed: u8, kind: AssetMediaKind) -> AssetContentDescriptor {
    let digest = AssetContentDigest::from_bytes([seed; 32]);
    let mime = match kind {
        AssetMediaKind::Image => AssetMediaMimeType::ImagePng,
        AssetMediaKind::Video => AssetMediaMimeType::VideoMp4,
        AssetMediaKind::Audio => AssetMediaMimeType::AudioWav,
    };
    AssetContentDescriptor::try_new(
        AssetManagedContentId::from_digest(digest),
        digest,
        10,
        mime,
        kind,
    )
    .unwrap()
}

fn imported_origin() -> AssetOrigin {
    AssetOrigin::imported(
        AssetImportId::from_uuid(uuid(5)).unwrap(),
        AssetOriginalFileName::try_new("image.png").unwrap(),
    )
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
