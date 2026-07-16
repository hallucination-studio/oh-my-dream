use std::{io::Cursor, time::Duration};

use assets::asset::domain::{AssetManagedContentId, AssetMediaMimeType};
use tokio::io::AsyncReadExt;

use super::*;

#[tokio::test]
async fn store_stages_publishes_opens_lists_and_removes_exact_bytes() {
    let directory = tempfile::tempdir().unwrap();
    let store =
        LocalFilesystemAssetManagedContentStoreAdapterImpl::try_new(directory.path().to_owned())
            .unwrap();
    let bytes = b"exact image bytes".to_vec();
    let deadline = Instant::now() + Duration::from_secs(10);
    let staged = store
        .stage_imported_asset_content(
            AssetImportSourceLease::new(deadline, Box::pin(Cursor::new(bytes.clone()))),
            AssetMediaKind::Image,
            created_at(1),
        )
        .await
        .unwrap();
    let descriptor = AssetContentDescriptor::try_new(
        AssetManagedContentId::from_digest(staged.digest()),
        staged.digest(),
        staged.byte_length(),
        AssetMediaMimeType::ImagePng,
        AssetMediaKind::Image,
    )
    .unwrap();
    let wrong_digest = AssetContentDigest::from_bytes([9; 32]);
    let wrong_descriptor = AssetContentDescriptor::try_new(
        AssetManagedContentId::from_digest(wrong_digest),
        wrong_digest,
        staged.byte_length(),
        AssetMediaMimeType::ImagePng,
        AssetMediaKind::Image,
    )
    .unwrap();
    assert_eq!(
        store
            .publish_staged_asset_content(
                staged.staged_content_ref().clone(),
                wrong_descriptor,
                deadline,
            )
            .await,
        Err(AssetApplicationError::ContentDigestMismatch)
    );

    let staged_lease = store
        .open_staged_asset_content(staged.staged_content_ref().clone(), deadline)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(read(staged_lease.try_take_stream().unwrap()).await, bytes);
    store
        .publish_staged_asset_content(
            staged.staged_content_ref().clone(),
            descriptor.clone(),
            deadline,
        )
        .await
        .unwrap();
    assert!(store.verify_managed_asset_content(descriptor.clone(), deadline).await.unwrap());
    let managed =
        store.open_managed_asset_content(descriptor.clone(), deadline).await.unwrap().unwrap();
    assert_eq!(read(managed.try_take_stream().unwrap()).await, bytes);

    let page = store
        .list_stale_asset_staged_content(created_at(2), None, AssetPageLimit::from_u16(10).unwrap())
        .await
        .unwrap();
    assert_eq!(page.staged_contents(), std::slice::from_ref(&staged));
    store.remove_asset_staged_content(staged.staged_content_ref().clone(), deadline).await.unwrap();
    assert!(
        store
            .open_staged_asset_content(staged.staged_content_ref().clone(), deadline)
            .await
            .unwrap()
            .is_none()
    );
    assert!(store.verify_managed_asset_content(descriptor, deadline).await.unwrap());
}

#[tokio::test]
async fn store_rejects_empty_and_expired_sources_without_partial_staging() {
    let directory = tempfile::tempdir().unwrap();
    let store =
        LocalFilesystemAssetManagedContentStoreAdapterImpl::try_new(directory.path().to_owned())
            .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    assert_eq!(
        store
            .stage_node_output_asset_content(
                AssetNodeOutputSourceLease::new(deadline, Box::pin(Cursor::new(Vec::new()))),
                AssetMediaKind::Image,
                created_at(1),
            )
            .await,
        Err(AssetApplicationError::InvalidMedia)
    );
    assert_eq!(
        store
            .stage_imported_asset_content(
                AssetImportSourceLease::new(
                    Instant::now() - Duration::from_millis(1),
                    Box::pin(Cursor::new(vec![1])),
                ),
                AssetMediaKind::Image,
                created_at(2),
            )
            .await,
        Err(AssetApplicationError::DeadlineExceeded)
    );
    assert_eq!(std::fs::read_dir(directory.path().join("staging")).unwrap().count(), 0);
}

async fn read(mut stream: Pin<Box<dyn AsyncRead + Send>>) -> Vec<u8> {
    let mut bytes = Vec::new();
    stream.read_to_end(&mut bytes).await.unwrap();
    bytes
}

fn created_at(value: i64) -> AssetCreatedAt {
    AssetCreatedAt::from_utc_milliseconds(value).unwrap()
}
