//! Opaque staged-content identity and observed byte facts.

use crate::asset::domain::{AssetContentDigest, AssetCreatedAt};

use super::AssetApplicationError;

/// Opaque identity of one staged byte object.
#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct AssetStagedContentRef(Vec<u8>);

impl AssetStagedContentRef {
    /// Restores a bounded opaque value returned by a content-store adapter.
    pub fn try_from_store_bytes(value: Vec<u8>) -> Result<Self, AssetApplicationError> {
        if value.is_empty() || value.len() > 512 {
            return Err(AssetApplicationError::ManagedStorageFailed);
        }
        Ok(Self(value))
    }

    /// Borrows the uninterpreted bytes for persistence or store adapters.
    #[must_use]
    pub fn as_store_bytes(&self) -> &[u8] {
        &self.0
    }
}

/// Digest, length, identity, and time observed while staging one source.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct AssetStagedContent {
    staged_content_ref: AssetStagedContentRef,
    digest: AssetContentDigest,
    byte_length: u64,
    created_at: AssetCreatedAt,
}

impl AssetStagedContent {
    /// Combines non-empty facts already verified by the managed-content store.
    pub fn try_new(
        staged_content_ref: AssetStagedContentRef,
        digest: AssetContentDigest,
        byte_length: u64,
        created_at: AssetCreatedAt,
    ) -> Result<Self, AssetApplicationError> {
        if byte_length == 0 {
            return Err(AssetApplicationError::InvalidMedia);
        }
        Ok(Self { staged_content_ref, digest, byte_length, created_at })
    }

    /// Returns the opaque staged-content identity.
    #[must_use]
    pub const fn staged_content_ref(&self) -> &AssetStagedContentRef {
        &self.staged_content_ref
    }

    /// Returns the digest calculated while staging.
    #[must_use]
    pub const fn digest(&self) -> AssetContentDigest {
        self.digest
    }

    /// Returns the byte length calculated while staging.
    #[must_use]
    pub const fn byte_length(&self) -> u64 {
        self.byte_length
    }

    /// Returns the caller-supplied creation time.
    #[must_use]
    pub const fn created_at(&self) -> AssetCreatedAt {
        self.created_at
    }
}
