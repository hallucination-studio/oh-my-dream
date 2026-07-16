use std::path::{Path, PathBuf};

use assets::asset::{
    application::{AssetApplicationError, AssetStagedContentRef},
    domain::{AssetCreatedAt, AssetManagedContentId},
};

pub(super) fn staged_ref(
    created_at: AssetCreatedAt,
) -> Result<AssetStagedContentRef, AssetApplicationError> {
    let mut bytes = Vec::with_capacity(24);
    bytes.extend_from_slice(&created_at.as_utc_milliseconds().to_be_bytes());
    bytes.extend_from_slice(uuid::Uuid::new_v4().as_bytes());
    AssetStagedContentRef::try_from_store_bytes(bytes)
}

pub(super) fn parse_staged_ref(
    reference: &AssetStagedContentRef,
) -> Result<AssetCreatedAt, AssetApplicationError> {
    let bytes = reference.as_store_bytes();
    let encoded: [u8; 8] = bytes
        .get(..8)
        .ok_or(AssetApplicationError::ManagedStorageFailed)?
        .try_into()
        .map_err(|_| AssetApplicationError::ManagedStorageFailed)?;
    if bytes.len() != 24 {
        return Err(AssetApplicationError::ManagedStorageFailed);
    }
    AssetCreatedAt::from_utc_milliseconds(i64::from_be_bytes(encoded))
        .map_err(|_| AssetApplicationError::ManagedStorageFailed)
}

pub(super) fn staged_path(
    root: &Path,
    reference: &AssetStagedContentRef,
) -> Result<PathBuf, AssetApplicationError> {
    parse_staged_ref(reference)?;
    Ok(root.join(hex(reference.as_store_bytes())))
}

pub(super) fn managed_path(root: &Path, content_id: AssetManagedContentId) -> PathBuf {
    root.join(hex(&content_id.canonical_bytes()))
}

pub(super) fn decode_staged_name(
    name: &str,
) -> Result<AssetStagedContentRef, AssetApplicationError> {
    if name.len() != 48
        || !name.bytes().all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(AssetApplicationError::ManagedStorageFailed);
    }
    let bytes = name
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let text = std::str::from_utf8(pair)
                .map_err(|_| AssetApplicationError::ManagedStorageFailed)?;
            u8::from_str_radix(text, 16).map_err(|_| AssetApplicationError::ManagedStorageFailed)
        })
        .collect::<Result<Vec<_>, _>>()?;
    AssetStagedContentRef::try_from_store_bytes(bytes)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
