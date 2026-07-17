use assets::asset::domain::{
    AssetContentDescriptor, AssetContentDigest, AssetCreatedAt, AssetDisplayName, AssetId,
    AssetManagedContentId, AssetMediaFacts, AssetMediaKind, AssetMediaMimeType,
    AssetOriginalFileName,
};
use uuid::Uuid;

#[test]
fn asset_identity_names_and_time_enforce_the_frozen_boundaries() {
    assert!(AssetId::from_uuid(Uuid::nil()).is_err());
    assert!(AssetCreatedAt::from_utc_milliseconds(-1).is_err());
    assert_eq!(AssetDisplayName::try_new("  cover image  ").unwrap().as_str(), "cover image");
    assert!(AssetDisplayName::try_new("\n").is_err());
    assert!(AssetOriginalFileName::try_new("folder/image.png").is_err());
    assert_eq!(AssetOriginalFileName::try_new("image.png").unwrap().as_str(), "image.png");
    assert_eq!(
        AssetId::from_uuid(uuid(7)).unwrap().to_string(),
        "07000000-0000-4000-8000-000000000007"
    );
}

fn uuid(seed: u8) -> Uuid {
    Uuid::from_bytes([seed, 0, 0, 0, 0, 0, 0x40, 0, 0x80, 0, 0, 0, 0, 0, 0, seed])
}

#[test]
fn managed_content_identity_is_derived_from_the_exact_digest() {
    let digest = AssetContentDigest::from_bytes([0xab; 32]);
    let content_id = AssetManagedContentId::from_digest(digest);
    assert_eq!(content_id.canonical_bytes()[0], 1);
    assert_eq!(content_id.digest(), digest);
    assert_eq!(content_id.to_string(), format!("sha256-v1:{}", "ab".repeat(32)));
}

#[test]
fn descriptor_rejects_kind_mime_size_and_content_identity_mismatches() {
    let digest = AssetContentDigest::from_bytes([3; 32]);
    let content_id = AssetManagedContentId::from_digest(digest);
    assert!(
        AssetContentDescriptor::try_new(
            content_id,
            digest,
            32 * 1024 * 1024,
            AssetMediaMimeType::ImagePng,
            AssetMediaKind::Image,
        )
        .is_ok()
    );
    assert!(
        AssetContentDescriptor::try_new(
            content_id,
            AssetContentDigest::from_bytes([4; 32]),
            1,
            AssetMediaMimeType::ImagePng,
            AssetMediaKind::Image,
        )
        .is_err()
    );
    assert!(
        AssetContentDescriptor::try_new(
            content_id,
            digest,
            32 * 1024 * 1024 + 1,
            AssetMediaMimeType::ImagePng,
            AssetMediaKind::Image,
        )
        .is_err()
    );
    assert!(
        AssetContentDescriptor::try_new(
            content_id,
            digest,
            1,
            AssetMediaMimeType::VideoMp4,
            AssetMediaKind::Image,
        )
        .is_err()
    );
}

#[test]
fn media_facts_enforce_exact_kind_specific_bounds() {
    assert!(AssetMediaFacts::try_image(1, 16_384).is_ok());
    assert!(AssetMediaFacts::try_image(0, 1).is_err());
    assert!(AssetMediaFacts::try_video(1920, 1080, 86_400_000, true).is_ok());
    assert!(AssetMediaFacts::try_video(1920, 1080, 86_400_001, false).is_err());
    assert!(AssetMediaFacts::try_audio(1, 8_000, 1).is_ok());
    assert!(AssetMediaFacts::try_audio(1, 7_999, 1).is_err());
    assert!(AssetMediaFacts::try_audio(1, 192_000, 9).is_err());
}
