use assets::asset::{
    application::AssetPreviewLease,
    domain::{
        AssetContentDescriptor, AssetContentDigest, AssetId, AssetManagedContentId, AssetMediaKind,
        AssetMediaMimeType, AssetPreviewLeaseId,
    },
};
use projects::project::domain::ProjectId;
use uuid::Uuid;

use super::{
    PreviewTokenCodec,
    handler::{build_success, parse_range},
    token::TokenError,
};
use crate::asset_preview_protocol::DesktopAssetPreviewMethod;

#[test]
fn token_is_canonical_signed_restart_scoped_and_time_bounded() {
    let lease = lease();
    let codec = PreviewTokenCodec::from_test_key([9; 32]);
    let uri = codec.issue(&lease);
    let decoded = codec.decode(&uri, 10).unwrap();
    assert_eq!(decoded.project_id, lease.project_id());
    assert_eq!(decoded.asset_id, lease.asset_id());
    assert_eq!(decoded.content_id, lease.content_id());
    assert_eq!(decoded.issued_at_epoch_ms, 10);
    assert_eq!(decoded.expires_at_epoch_ms, 300_010);

    let mut tampered = uri.into_bytes();
    *tampered.last_mut().unwrap() = if *tampered.last().unwrap() == b'A' { b'B' } else { b'A' };
    assert_eq!(codec.decode(std::str::from_utf8(&tampered).unwrap(), 10), Err(TokenError::Invalid));
    let restarted = PreviewTokenCodec::from_test_key([8; 32]);
    assert_eq!(restarted.decode(codec.issue(&lease).as_str(), 10), Err(TokenError::Invalid));
    assert_eq!(codec.decode(codec.issue(&lease).as_str(), 300_010), Err(TokenError::Expired));
}

#[test]
fn range_and_head_responses_preserve_verified_headers_without_paths() {
    assert_eq!(parse_range("bytes=2-4", 8), Some((2, 4)));
    assert_eq!(parse_range("bytes=-3", 8), Some((5, 7)));
    assert_eq!(parse_range("bytes=8-", 8), None);
    assert_eq!(parse_range("bytes=1-2,4-5", 8), None);

    let descriptor = AssetContentDescriptor::try_new(
        AssetManagedContentId::from_digest(AssetContentDigest::from_bytes([7; 32])),
        AssetContentDigest::from_bytes([7; 32]),
        8,
        AssetMediaMimeType::VideoMp4,
        AssetMediaKind::Video,
    )
    .unwrap();
    let response = build_success(
        DesktopAssetPreviewMethod::Get,
        Some("bytes=2-4"),
        AssetMediaKind::Video,
        descriptor.clone(),
        b"12345678".to_vec(),
    )
    .unwrap();
    assert_eq!(response.status, 206);
    assert_eq!(response.body, b"345");
    assert_eq!(response.headers.get("Content-Range").unwrap(), "bytes 2-4/8");
    assert_eq!(response.headers.get("Accept-Ranges").unwrap(), "bytes");
    assert!(!format!("{response:?}").contains("/Users/"));

    let head = build_success(
        DesktopAssetPreviewMethod::Head,
        None,
        AssetMediaKind::Video,
        descriptor,
        b"12345678".to_vec(),
    )
    .unwrap();
    assert_eq!(head.status, 200);
    assert_eq!(head.headers.get("Content-Length").unwrap(), "8");
    assert!(head.body.is_empty());
}

fn lease() -> AssetPreviewLease {
    AssetPreviewLease::try_new(
        AssetPreviewLeaseId::from_uuid(uuid(1)).unwrap(),
        ProjectId::from_uuid(uuid(2)).unwrap(),
        AssetId::from_uuid(uuid(3)).unwrap(),
        AssetManagedContentId::from_digest(AssetContentDigest::from_bytes([7; 32])),
        10,
    )
    .unwrap()
}

fn uuid(seed: u8) -> Uuid {
    Uuid::from_bytes([seed, 0, 0, 0, 0, 0, 0x40, 0, 0x80, 0, 0, 0, 0, 0, 0, seed])
}
