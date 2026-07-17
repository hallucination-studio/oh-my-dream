//! Verified managed-content identity and descriptor.

use std::fmt;

use super::{AssetDomainError, AssetMediaKind, AssetMediaMimeType};

/// Exact SHA-256 digest bytes.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct AssetContentDigest([u8; 32]);

impl AssetContentDigest {
    /// Wraps an already computed SHA-256 digest.
    #[must_use]
    pub const fn from_bytes(value: [u8; 32]) -> Self {
        Self(value)
    }

    /// Returns exact digest bytes.
    #[must_use]
    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Versioned immutable managed byte-object identity derived from its digest.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct AssetManagedContentId(AssetContentDigest);

impl AssetManagedContentId {
    /// Derives the version-one identity from verified digest bytes.
    #[must_use]
    pub const fn from_digest(digest: AssetContentDigest) -> Self {
        Self(digest)
    }

    /// Returns the identity's authoritative digest.
    #[must_use]
    pub const fn digest(self) -> AssetContentDigest {
        self.0
    }

    /// Returns scheme byte `1` followed by the exact digest.
    #[must_use]
    pub fn canonical_bytes(self) -> [u8; 33] {
        let mut bytes = [0_u8; 33];
        bytes[0] = 1;
        bytes[1..].copy_from_slice(&self.0.as_bytes());
        bytes
    }
}

impl fmt::Display for AssetManagedContentId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("sha256-v1:")?;
        for byte in self.0.as_bytes() {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

/// Exact immutable verified managed-content description.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct AssetContentDescriptor {
    content_id: AssetManagedContentId,
    digest: AssetContentDigest,
    byte_length: u64,
    mime_type: AssetMediaMimeType,
    media_kind: AssetMediaKind,
}

impl AssetContentDescriptor {
    /// Creates a descriptor only when identity, digest, size, MIME, and kind agree.
    pub fn try_new(
        content_id: AssetManagedContentId,
        digest: AssetContentDigest,
        byte_length: u64,
        mime_type: AssetMediaMimeType,
        media_kind: AssetMediaKind,
    ) -> Result<Self, AssetDomainError> {
        let valid_size = (1..=maximum_bytes(media_kind)).contains(&byte_length);
        if content_id.digest() != digest || mime_type.media_kind() != media_kind || !valid_size {
            return Err(AssetDomainError::InvalidDescriptor);
        }
        Ok(Self { content_id, digest, byte_length, mime_type, media_kind })
    }

    /// Returns the immutable managed-content identity.
    #[must_use]
    pub const fn content_id(&self) -> AssetManagedContentId {
        self.content_id
    }
    /// Returns the verified digest.
    #[must_use]
    pub const fn digest(&self) -> AssetContentDigest {
        self.digest
    }
    /// Returns the verified byte length.
    #[must_use]
    pub const fn byte_length(&self) -> u64 {
        self.byte_length
    }
    /// Returns the sniffed MIME value.
    #[must_use]
    pub const fn mime_type(&self) -> AssetMediaMimeType {
        self.mime_type
    }
    /// Returns the exact managed-media kind.
    #[must_use]
    pub const fn media_kind(&self) -> AssetMediaKind {
        self.media_kind
    }
}

const fn maximum_bytes(media_kind: AssetMediaKind) -> u64 {
    match media_kind {
        AssetMediaKind::Image => 32 * 1024 * 1024,
        AssetMediaKind::Video => 512 * 1024 * 1024,
        AssetMediaKind::Audio => 64 * 1024 * 1024,
    }
}
