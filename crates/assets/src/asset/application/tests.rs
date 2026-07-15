use std::io::Cursor;
use std::time::{Duration, Instant};

use projects::project::domain::ProjectId;
use tokio::io::AsyncReadExt;
use uuid::Uuid;

use super::*;
use crate::asset::domain::{
    AssetContentDigest, AssetId, AssetManagedContentId, AssetMediaKind, AssetPreviewLeaseId,
};

fn content_id() -> AssetManagedContentId {
    AssetManagedContentId::from_digest(AssetContentDigest::from_bytes([7; 32]))
}

fn asset_id() -> AssetId {
    AssetId::from_uuid(Uuid::from_u128(0x12345678_1234_4234_8234_123456789012)).unwrap()
}

fn project_id() -> ProjectId {
    ProjectId::from_uuid(Uuid::from_u128(0x22345678_1234_4234_8234_123456789012)).unwrap()
}

fn preview_lease_id() -> AssetPreviewLeaseId {
    AssetPreviewLeaseId::from_uuid(Uuid::from_u128(0x32345678_1234_4234_8234_123456789012)).unwrap()
}

#[tokio::test]
async fn managed_content_lease_exposes_exact_metadata_and_one_stream() {
    let deadline = Instant::now() + Duration::from_secs(60);
    let lease = AssetManagedContentLease::new(
        content_id(),
        3,
        deadline,
        Box::pin(Cursor::new(vec![1, 2, 3])),
    );

    assert_eq!(lease.content_id(), content_id());
    assert_eq!(lease.byte_length(), 3);
    assert_eq!(lease.deadline(), deadline);
    let mut stream = lease.try_take_stream().unwrap();
    let mut bytes = Vec::new();
    stream.read_to_end(&mut bytes).await.unwrap();
    assert_eq!(bytes, vec![1, 2, 3]);
}

#[test]
fn stream_leases_reject_handoff_at_expired_deadline() {
    let expired = Instant::now() - Duration::from_secs(1);
    let managed =
        AssetManagedContentLease::new(content_id(), 1, expired, Box::pin(Cursor::new(vec![1])));
    let imported = AssetImportSourceLease::new(expired, Box::pin(Cursor::new(vec![1])));

    assert!(matches!(managed.try_take_stream(), Err(AssetApplicationError::DeadlineExceeded)));
    assert!(matches!(imported.try_take_stream(), Err(AssetApplicationError::DeadlineExceeded)));
}

#[test]
fn preview_lease_derives_exact_five_minute_expiry() {
    let lease = AssetPreviewLease::try_new(
        preview_lease_id(),
        project_id(),
        asset_id(),
        content_id(),
        1_000,
    )
    .unwrap();

    assert_eq!(lease.issued_at_utc_milliseconds(), 1_000);
    assert_eq!(lease.expires_at_utc_milliseconds(), 301_000);
}

#[test]
fn preview_lease_rejects_invalid_issue_time() {
    for issued_at in [-1, i64::MAX] {
        assert!(matches!(
            AssetPreviewLease::try_new(
                preview_lease_id(),
                project_id(),
                asset_id(),
                content_id(),
                issued_at,
            ),
            Err(AssetApplicationError::PreviewLeaseInvalid)
        ));
    }
}

#[test]
fn application_error_has_exact_frozen_categories() {
    let errors = [
        AssetApplicationError::NotFound,
        AssetApplicationError::NotVisible,
        AssetApplicationError::MediaKindMismatch {
            expected: AssetMediaKind::Image,
            observed: AssetMediaKind::Video,
        },
        AssetApplicationError::ContentPending,
        AssetApplicationError::ContentMissing,
        AssetApplicationError::InvalidMedia,
        AssetApplicationError::MediaSizeLimitExceeded,
        AssetApplicationError::ContentDigestMismatch,
        AssetApplicationError::NodeOutputConflict,
        AssetApplicationError::ManagedStorageFailed,
        AssetApplicationError::IdentityConflict,
        AssetApplicationError::InspectionFailed,
        AssetApplicationError::FinalizationFailed,
        AssetApplicationError::PreviewLeaseInvalid,
        AssetApplicationError::PreviewLeaseExpired,
        AssetApplicationError::PreviewRangeInvalid,
        AssetApplicationError::Cancelled,
        AssetApplicationError::DeadlineExceeded,
    ];

    for error in errors {
        assert_eq!(frozen_error_category(error), error);
    }
}

const fn frozen_error_category(error: AssetApplicationError) -> AssetApplicationError {
    match error {
        AssetApplicationError::NotFound => error,
        AssetApplicationError::NotVisible => error,
        AssetApplicationError::MediaKindMismatch { .. } => error,
        AssetApplicationError::ContentPending => error,
        AssetApplicationError::ContentMissing => error,
        AssetApplicationError::InvalidMedia => error,
        AssetApplicationError::MediaSizeLimitExceeded => error,
        AssetApplicationError::ContentDigestMismatch => error,
        AssetApplicationError::NodeOutputConflict => error,
        AssetApplicationError::ManagedStorageFailed => error,
        AssetApplicationError::IdentityConflict => error,
        AssetApplicationError::InspectionFailed => error,
        AssetApplicationError::FinalizationFailed => error,
        AssetApplicationError::PreviewLeaseInvalid => error,
        AssetApplicationError::PreviewLeaseExpired => error,
        AssetApplicationError::PreviewRangeInvalid => error,
        AssetApplicationError::Cancelled => error,
        AssetApplicationError::DeadlineExceeded => error,
    }
}
