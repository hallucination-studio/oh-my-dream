use std::time::{Duration, Instant};

use assets::asset::application::{
    AssetContentFinalization, AssetFinalizeContentCommand, AssetFinalizeContentEffect,
    AssetStagedContentRef,
};
use assets::asset::domain::{
    AssetAggregate, AssetContentDescriptor, AssetContentDigest, AssetContentFinalizationId,
    AssetCreatedAt, AssetDisplayName, AssetId, AssetImportId, AssetManagedContentId,
    AssetMediaFacts, AssetMediaKind, AssetMediaMimeType, AssetOrigin, AssetOriginalFileName,
};
use projects::project::domain::ProjectId;
use uuid::Uuid;

pub fn command() -> AssetFinalizeContentCommand {
    AssetFinalizeContentCommand::new(
        AssetFinalizeContentEffect::new(finalization_id()),
        Instant::now() + Duration::from_secs(60),
    )
}

pub fn pending_asset() -> AssetAggregate {
    AssetAggregate::try_new_pending(
        asset_id(),
        project_id(),
        AssetMediaKind::Image,
        descriptor(),
        finalization_id(),
        AssetMediaFacts::try_image(32, 32).unwrap(),
        AssetOrigin::imported(
            AssetImportId::from_uuid(uuid(4)).unwrap(),
            AssetOriginalFileName::try_new("image.png").unwrap(),
        ),
        AssetDisplayName::try_new("image").unwrap(),
        created_at(),
    )
    .unwrap()
}

pub fn content_finalization() -> AssetContentFinalization {
    AssetContentFinalization::new(
        finalization_id(),
        asset_id(),
        descriptor(),
        staged_ref(),
        created_at(),
    )
}

pub fn descriptor() -> AssetContentDescriptor {
    AssetContentDescriptor::try_new(
        AssetManagedContentId::from_digest(digest()),
        digest(),
        10,
        AssetMediaMimeType::ImagePng,
        AssetMediaKind::Image,
    )
    .unwrap()
}

pub fn digest() -> AssetContentDigest {
    AssetContentDigest::from_bytes([3; 32])
}
pub fn staged_ref() -> AssetStagedContentRef {
    AssetStagedContentRef::try_from_store_bytes(vec![1]).unwrap()
}
pub fn asset_id() -> AssetId {
    AssetId::from_uuid(uuid(1)).unwrap()
}
fn project_id() -> ProjectId {
    ProjectId::from_uuid(uuid(2)).unwrap()
}
pub fn finalization_id() -> AssetContentFinalizationId {
    AssetContentFinalizationId::from_uuid(uuid(3)).unwrap()
}
pub fn created_at() -> AssetCreatedAt {
    AssetCreatedAt::from_utc_milliseconds(10).unwrap()
}
fn uuid(seed: u8) -> Uuid {
    Uuid::from_bytes([seed, 0, 0, 0, 0, 0, 0x40, 0, 0x80, 0, 0, 0, 0, 0, 0, seed])
}
