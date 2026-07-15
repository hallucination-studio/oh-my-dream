//! Opaque process-local Asset lease values.

use std::pin::Pin;
use std::time::Instant;

use projects::project::domain::ProjectId;
use tokio::io::AsyncRead;

use super::AssetApplicationError;
use crate::asset::domain::{AssetId, AssetManagedContentId, AssetPreviewLeaseId};

/// One-shot access to an exact managed-content stream.
pub struct AssetManagedContentLease {
    content_id: AssetManagedContentId,
    byte_length: u64,
    deadline: Instant,
    stream: Pin<Box<dyn AsyncRead + Send>>,
}

impl AssetManagedContentLease {
    /// Creates a lease over an already-open exact-content stream.
    #[must_use]
    pub fn new(
        content_id: AssetManagedContentId,
        byte_length: u64,
        deadline: Instant,
        stream: Pin<Box<dyn AsyncRead + Send>>,
    ) -> Self {
        Self { content_id, byte_length, deadline, stream }
    }

    /// Returns the exact managed-content identity.
    #[must_use]
    pub const fn content_id(&self) -> AssetManagedContentId {
        self.content_id
    }

    /// Returns the exact expected byte length.
    #[must_use]
    pub const fn byte_length(&self) -> u64 {
        self.byte_length
    }

    /// Returns the caller-supplied monotonic deadline.
    #[must_use]
    pub const fn deadline(&self) -> Instant {
        self.deadline
    }

    /// Consumes the lease and returns its stream before expiry.
    pub fn try_take_stream(self) -> Result<Pin<Box<dyn AsyncRead + Send>>, AssetApplicationError> {
        if Instant::now() >= self.deadline {
            return Err(AssetApplicationError::DeadlineExceeded);
        }
        Ok(self.stream)
    }
}

/// One-shot access to an already-open trusted import source.
pub struct AssetImportSourceLease {
    deadline: Instant,
    stream: Pin<Box<dyn AsyncRead + Send>>,
}

impl AssetImportSourceLease {
    /// Creates a lease over an already-open trusted source stream.
    #[must_use]
    pub fn new(deadline: Instant, stream: Pin<Box<dyn AsyncRead + Send>>) -> Self {
        Self { deadline, stream }
    }

    /// Returns the caller-supplied monotonic deadline.
    #[must_use]
    pub const fn deadline(&self) -> Instant {
        self.deadline
    }

    /// Consumes the lease and returns its stream before expiry.
    pub fn try_take_stream(self) -> Result<Pin<Box<dyn AsyncRead + Send>>, AssetApplicationError> {
        if Instant::now() >= self.deadline {
            return Err(AssetApplicationError::DeadlineExceeded);
        }
        Ok(self.stream)
    }
}

/// Immutable five-minute permission to issue a process-local preview token.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct AssetPreviewLease {
    lease_id: AssetPreviewLeaseId,
    project_id: ProjectId,
    asset_id: AssetId,
    content_id: AssetManagedContentId,
    issued_at_utc_milliseconds: i64,
    expires_at_utc_milliseconds: i64,
}

impl AssetPreviewLease {
    const LIFETIME_MILLISECONDS: i64 = 300_000;

    /// Creates a preview lease and derives its exact five-minute expiry.
    pub fn try_new(
        lease_id: AssetPreviewLeaseId,
        project_id: ProjectId,
        asset_id: AssetId,
        content_id: AssetManagedContentId,
        issued_at_utc_milliseconds: i64,
    ) -> Result<Self, AssetApplicationError> {
        if issued_at_utc_milliseconds < 0 {
            return Err(AssetApplicationError::PreviewLeaseInvalid);
        }
        let expires_at_utc_milliseconds = issued_at_utc_milliseconds
            .checked_add(Self::LIFETIME_MILLISECONDS)
            .ok_or(AssetApplicationError::PreviewLeaseInvalid)?;
        Ok(Self {
            lease_id,
            project_id,
            asset_id,
            content_id,
            issued_at_utc_milliseconds,
            expires_at_utc_milliseconds,
        })
    }

    /// Returns the preview permission identity.
    #[must_use]
    pub const fn lease_id(&self) -> AssetPreviewLeaseId {
        self.lease_id
    }

    /// Returns the owning Project identity.
    #[must_use]
    pub const fn project_id(&self) -> ProjectId {
        self.project_id
    }

    /// Returns the visible Asset identity.
    #[must_use]
    pub const fn asset_id(&self) -> AssetId {
        self.asset_id
    }

    /// Returns the exact previewed content identity.
    #[must_use]
    pub const fn content_id(&self) -> AssetManagedContentId {
        self.content_id
    }

    /// Returns the issue time in UTC epoch milliseconds.
    #[must_use]
    pub const fn issued_at_utc_milliseconds(&self) -> i64 {
        self.issued_at_utc_milliseconds
    }

    /// Returns the derived expiry in UTC epoch milliseconds.
    #[must_use]
    pub const fn expires_at_utc_milliseconds(&self) -> i64 {
        self.expires_at_utc_milliseconds
    }
}
