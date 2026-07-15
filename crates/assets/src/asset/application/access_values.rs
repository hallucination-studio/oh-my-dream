//! Project-scoped Asset access request and result values.

use std::time::Instant;

use projects::project::domain::ProjectId;

use super::AssetManagedContentLease;
use crate::asset::domain::{AssetContentDescriptor, AssetId, AssetMediaKind};

/// One Project-scoped Asset lookup.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct AssetGetQuery {
    project_id: ProjectId,
    asset_id: AssetId,
}

impl AssetGetQuery {
    /// Creates one exact Project-scoped lookup.
    #[must_use]
    pub const fn new(project_id: ProjectId, asset_id: AssetId) -> Self {
        Self { project_id, asset_id }
    }
    /// Returns the expected owning Project.
    #[must_use]
    pub const fn project_id(self) -> ProjectId {
        self.project_id
    }
    /// Returns the requested Asset identity.
    #[must_use]
    pub const fn asset_id(self) -> AssetId {
        self.asset_id
    }
}

/// One exact managed-content resolution request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AssetResolveContentQuery {
    project_id: ProjectId,
    asset_id: AssetId,
    expected_media_kind: AssetMediaKind,
    deadline: Instant,
}

impl AssetResolveContentQuery {
    /// Creates one deadline-bounded exact-content request.
    #[must_use]
    pub const fn new(
        project_id: ProjectId,
        asset_id: AssetId,
        expected_media_kind: AssetMediaKind,
        deadline: Instant,
    ) -> Self {
        Self { project_id, asset_id, expected_media_kind, deadline }
    }
    /// Returns the expected owning Project.
    #[must_use]
    pub const fn project_id(self) -> ProjectId {
        self.project_id
    }
    /// Returns the requested Asset identity.
    #[must_use]
    pub const fn asset_id(self) -> AssetId {
        self.asset_id
    }
    /// Returns the exact expected media kind.
    #[must_use]
    pub const fn expected_media_kind(self) -> AssetMediaKind {
        self.expected_media_kind
    }
    /// Returns the caller's monotonic deadline.
    #[must_use]
    pub const fn deadline(self) -> Instant {
        self.deadline
    }
}

/// Exact descriptor and one-shot managed-content access.
pub struct AssetResolvedContent {
    descriptor: AssetContentDescriptor,
    content_lease: AssetManagedContentLease,
}

impl AssetResolvedContent {
    pub(crate) const fn new(
        descriptor: AssetContentDescriptor,
        content_lease: AssetManagedContentLease,
    ) -> Self {
        Self { descriptor, content_lease }
    }
    /// Returns the exact content descriptor.
    #[must_use]
    pub const fn descriptor(&self) -> &AssetContentDescriptor {
        &self.descriptor
    }
    /// Returns the matching one-shot content lease.
    #[must_use]
    pub const fn content_lease(&self) -> &AssetManagedContentLease {
        &self.content_lease
    }
    /// Consumes the result and returns the one-shot content lease.
    #[must_use]
    pub fn into_content_lease(self) -> AssetManagedContentLease {
        self.content_lease
    }
}

/// One Project-scoped preview permission request.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct AssetIssuePreviewCommand {
    project_id: ProjectId,
    asset_id: AssetId,
}

impl AssetIssuePreviewCommand {
    /// Creates one exact preview permission request.
    #[must_use]
    pub const fn new(project_id: ProjectId, asset_id: AssetId) -> Self {
        Self { project_id, asset_id }
    }
    /// Returns the expected owning Project.
    #[must_use]
    pub const fn project_id(self) -> ProjectId {
        self.project_id
    }
    /// Returns the requested Asset identity.
    #[must_use]
    pub const fn asset_id(self) -> AssetId {
        self.asset_id
    }
}
