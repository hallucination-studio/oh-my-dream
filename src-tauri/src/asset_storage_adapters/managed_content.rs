mod filesystem;
mod reference;

use std::{
    path::{Path, PathBuf},
    pin::Pin,
    time::Instant,
};

use assets::asset::{
    application::{
        AssetApplicationError, AssetImportSourceLease, AssetManagedContentLease,
        AssetNodeOutputSourceLease, AssetPageLimit, AssetStagedContent,
        AssetStagedContentRecoveryCursor, AssetStagedContentRecoveryPage, AssetStagedContentRef,
    },
    domain::{AssetContentDescriptor, AssetContentDigest, AssetCreatedAt, AssetMediaKind},
    interfaces::AssetManagedContentStoreInterface,
};
use async_trait::async_trait;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};

use filesystem::{reject_symlink, remove_stale_temporary_files, restrict_directory, restrict_file};
use reference::{managed_path, parse_staged_ref, staged_path, staged_ref};

#[derive(Clone)]
pub struct LocalFilesystemAssetManagedContentStoreAdapterImpl {
    staging_root: PathBuf,
    managed_root: PathBuf,
}

impl LocalFilesystemAssetManagedContentStoreAdapterImpl {
    pub fn try_new(root: PathBuf) -> Result<Self, AssetApplicationError> {
        reject_symlink(&root)?;
        std::fs::create_dir_all(&root).map_err(|_| storage())?;
        restrict_directory(&root)?;
        let staging_root = root.join("staging");
        let managed_root = root.join("managed");
        for directory in [&staging_root, &managed_root] {
            reject_symlink(directory)?;
            std::fs::create_dir_all(directory).map_err(|_| storage())?;
            restrict_directory(directory)?;
        }
        remove_stale_temporary_files(&staging_root)?;
        Ok(Self { staging_root, managed_root })
    }

    async fn stage(
        &self,
        stream: Pin<Box<dyn AsyncRead + Send>>,
        deadline: Instant,
        expected_media_kind: AssetMediaKind,
        created_at: AssetCreatedAt,
    ) -> Result<AssetStagedContent, AssetApplicationError> {
        let reference = staged_ref(created_at)?;
        let destination = staged_path(&self.staging_root, &reference)?;
        let temporary = destination.with_extension("tmp");
        let result = tokio::time::timeout_at(
            tokio::time::Instant::from_std(deadline),
            copy_bounded(stream, &temporary, maximum_bytes(expected_media_kind)),
        )
        .await
        .map_err(|_| AssetApplicationError::DeadlineExceeded)?;
        let (digest, byte_length) = match result {
            Ok(value) => value,
            Err(error) => {
                let _ = tokio::fs::remove_file(&temporary).await;
                return Err(error);
            }
        };
        tokio::fs::rename(&temporary, &destination).await.map_err(|_| storage())?;
        restrict_file(&destination)?;
        AssetStagedContent::try_new(reference, digest, byte_length, created_at)
    }

    async fn verify_path(
        path: &Path,
        descriptor: &AssetContentDescriptor,
        deadline: Instant,
    ) -> Result<bool, AssetApplicationError> {
        reject_symlink(path)?;
        let Some((digest, length)) = deadline_hash(path, deadline).await? else {
            return Ok(false);
        };
        Ok(digest == descriptor.digest() && length == descriptor.byte_length())
    }
}

#[async_trait]
impl AssetManagedContentStoreInterface for LocalFilesystemAssetManagedContentStoreAdapterImpl {
    async fn stage_imported_asset_content(
        &self,
        source: AssetImportSourceLease,
        expected_media_kind: AssetMediaKind,
        created_at: AssetCreatedAt,
    ) -> Result<AssetStagedContent, AssetApplicationError> {
        let deadline = source.deadline();
        self.stage(source.try_take_stream()?, deadline, expected_media_kind, created_at).await
    }

    async fn stage_node_output_asset_content(
        &self,
        source: AssetNodeOutputSourceLease,
        expected_media_kind: AssetMediaKind,
        created_at: AssetCreatedAt,
    ) -> Result<AssetStagedContent, AssetApplicationError> {
        let deadline = source.deadline();
        self.stage(source.try_take_stream()?, deadline, expected_media_kind, created_at).await
    }

    async fn open_staged_asset_content(
        &self,
        staged_content_ref: AssetStagedContentRef,
        deadline: Instant,
    ) -> Result<Option<AssetImportSourceLease>, AssetApplicationError> {
        reject_elapsed(deadline)?;
        let path = staged_path(&self.staging_root, &staged_content_ref)?;
        match tokio::fs::File::open(path).await {
            Ok(file) => Ok(Some(AssetImportSourceLease::new(deadline, Box::pin(file)))),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(_) => Err(storage()),
        }
    }

    async fn publish_staged_asset_content(
        &self,
        staged_content_ref: AssetStagedContentRef,
        descriptor: AssetContentDescriptor,
        deadline: Instant,
    ) -> Result<(), AssetApplicationError> {
        let source = staged_path(&self.staging_root, &staged_content_ref)?;
        if !Self::verify_path(&source, &descriptor, deadline).await? {
            return match tokio::fs::metadata(&source).await {
                Ok(_) => Err(AssetApplicationError::ContentDigestMismatch),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    Err(AssetApplicationError::ContentMissing)
                }
                Err(_) => Err(storage()),
            };
        }
        let destination = managed_path(&self.managed_root, descriptor.content_id());
        if destination.exists() {
            return if Self::verify_path(&destination, &descriptor, deadline).await? {
                Ok(())
            } else {
                Err(AssetApplicationError::ContentDigestMismatch)
            };
        }
        match tokio::fs::hard_link(&source, &destination).await {
            Ok(()) => {
                restrict_file(&destination)?;
                Ok(())
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                if Self::verify_path(&destination, &descriptor, deadline).await? {
                    Ok(())
                } else {
                    Err(AssetApplicationError::ContentDigestMismatch)
                }
            }
            Err(_) => Err(storage()),
        }
    }

    async fn open_managed_asset_content(
        &self,
        descriptor: AssetContentDescriptor,
        deadline: Instant,
    ) -> Result<Option<AssetManagedContentLease>, AssetApplicationError> {
        let path = managed_path(&self.managed_root, descriptor.content_id());
        if !Self::verify_path(&path, &descriptor, deadline).await? {
            return Ok(None);
        }
        let file = tokio::fs::File::open(path).await.map_err(|_| storage())?;
        Ok(Some(AssetManagedContentLease::new(
            descriptor.content_id(),
            descriptor.byte_length(),
            deadline,
            Box::pin(file),
        )))
    }

    async fn verify_managed_asset_content(
        &self,
        descriptor: AssetContentDescriptor,
        deadline: Instant,
    ) -> Result<bool, AssetApplicationError> {
        Self::verify_path(
            &managed_path(&self.managed_root, descriptor.content_id()),
            &descriptor,
            deadline,
        )
        .await
    }

    async fn list_stale_asset_staged_content(
        &self,
        exclusive_created_before: AssetCreatedAt,
        cursor: Option<AssetStagedContentRecoveryCursor>,
        limit: AssetPageLimit,
    ) -> Result<AssetStagedContentRecoveryPage, AssetApplicationError> {
        list_staged(&self.staging_root, exclusive_created_before, cursor, limit).await
    }

    async fn remove_asset_staged_content(
        &self,
        staged_content_ref: AssetStagedContentRef,
        deadline: Instant,
    ) -> Result<(), AssetApplicationError> {
        reject_elapsed(deadline)?;
        match tokio::fs::remove_file(staged_path(&self.staging_root, &staged_content_ref)?).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(storage()),
        }
    }
}

async fn copy_bounded(
    mut source: Pin<Box<dyn AsyncRead + Send>>,
    destination: &Path,
    maximum: u64,
) -> Result<(AssetContentDigest, u64), AssetApplicationError> {
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .await
        .map_err(|_| storage())?;
    let mut hasher = Sha256::new();
    let mut total = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = source.read(&mut buffer).await.map_err(|_| storage())?;
        if count == 0 {
            break;
        }
        total =
            total.checked_add(count as u64).ok_or(AssetApplicationError::MediaSizeLimitExceeded)?;
        if total > maximum {
            return Err(AssetApplicationError::MediaSizeLimitExceeded);
        }
        hasher.update(&buffer[..count]);
        file.write_all(&buffer[..count]).await.map_err(|_| storage())?;
    }
    if total == 0 {
        return Err(AssetApplicationError::InvalidMedia);
    }
    file.flush().await.map_err(|_| storage())?;
    file.sync_all().await.map_err(|_| storage())?;
    Ok((AssetContentDigest::from_bytes(hasher.finalize().into()), total))
}

async fn deadline_hash(
    path: &Path,
    deadline: Instant,
) -> Result<Option<(AssetContentDigest, u64)>, AssetApplicationError> {
    reject_elapsed(deadline)?;
    let path = path.to_owned();
    tokio::time::timeout_at(tokio::time::Instant::from_std(deadline), async move {
        let mut file = match tokio::fs::File::open(path).await {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(storage()),
        };
        let mut hasher = Sha256::new();
        let mut total = 0_u64;
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let count = file.read(&mut buffer).await.map_err(|_| storage())?;
            if count == 0 {
                break;
            }
            total = total.checked_add(count as u64).ok_or_else(storage)?;
            hasher.update(&buffer[..count]);
        }
        Ok(Some((AssetContentDigest::from_bytes(hasher.finalize().into()), total)))
    })
    .await
    .map_err(|_| AssetApplicationError::DeadlineExceeded)?
}

async fn list_staged(
    root: &Path,
    cutoff: AssetCreatedAt,
    cursor: Option<AssetStagedContentRecoveryCursor>,
    limit: AssetPageLimit,
) -> Result<AssetStagedContentRecoveryPage, AssetApplicationError> {
    let mut directory = tokio::fs::read_dir(root).await.map_err(|_| storage())?;
    let mut references = Vec::new();
    while let Some(entry) = directory.next_entry().await.map_err(|_| storage())? {
        if entry.file_type().await.map_err(|_| storage())?.is_symlink() {
            return Err(storage());
        }
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            return Err(storage());
        };
        if name.ends_with(".tmp") {
            continue;
        }
        let reference = reference::decode_staged_name(&name)?;
        let created_at = parse_staged_ref(&reference)?;
        let after_cursor = cursor.as_ref().is_none_or(|cursor| {
            (created_at, reference.as_store_bytes())
                > (cursor.created_at(), cursor.staged_content_ref().as_store_bytes())
        });
        if created_at < cutoff && after_cursor {
            references.push((created_at, reference));
        }
    }
    references.sort_by(|left, right| {
        (left.0, left.1.as_store_bytes()).cmp(&(right.0, right.1.as_store_bytes()))
    });
    let page_size = usize::from(limit.get());
    let has_more = references.len() > page_size;
    references.truncate(page_size);
    let mut staged_contents = Vec::with_capacity(references.len());
    for (created_at, reference) in references {
        let path = staged_path(root, &reference)?;
        let Some((digest, length)) = hash_path(&path).await? else {
            return Err(storage());
        };
        staged_contents.push(AssetStagedContent::try_new(reference, digest, length, created_at)?);
    }
    let next_cursor = has_more.then(|| staged_contents.last()).flatten().map(|value| {
        AssetStagedContentRecoveryCursor::new(
            value.created_at(),
            value.staged_content_ref().clone(),
        )
    });
    Ok(AssetStagedContentRecoveryPage::new(staged_contents, next_cursor))
}

fn reject_elapsed(deadline: Instant) -> Result<(), AssetApplicationError> {
    if Instant::now() >= deadline { Err(AssetApplicationError::DeadlineExceeded) } else { Ok(()) }
}

async fn hash_path(
    path: &Path,
) -> Result<Option<(AssetContentDigest, u64)>, AssetApplicationError> {
    let mut file = match tokio::fs::File::open(path).await {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(storage()),
    };
    let mut hasher = Sha256::new();
    let mut total = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer).await.map_err(|_| storage())?;
        if count == 0 {
            break;
        }
        total = total.checked_add(count as u64).ok_or_else(storage)?;
        hasher.update(&buffer[..count]);
    }
    Ok(Some((AssetContentDigest::from_bytes(hasher.finalize().into()), total)))
}

fn maximum_bytes(kind: AssetMediaKind) -> u64 {
    match kind {
        AssetMediaKind::Image => 32 * 1024 * 1024,
        AssetMediaKind::Video => 512 * 1024 * 1024,
        AssetMediaKind::Audio => 64 * 1024 * 1024,
    }
}

fn storage() -> AssetApplicationError {
    AssetApplicationError::ManagedStorageFailed
}

#[cfg(test)]
mod tests;
