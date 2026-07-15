//! Stable bounded Asset query and recovery page values.

use projects::project::domain::ProjectId;

use crate::asset::domain::{
    AssetAggregate, AssetContentFinalizationId, AssetCreatedAt, AssetId, AssetMediaKind,
};

use super::{AssetContentFinalization, AssetStagedContent, AssetStagedContentRef};

/// Shared validated Asset list and recovery page limit.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct AssetPageLimit(u16);

impl AssetPageLimit {
    /// Returns a limit only within the frozen inclusive range.
    #[must_use]
    pub const fn from_u16(value: u16) -> Option<Self> {
        if value == 0 || value > 100 { None } else { Some(Self(value)) }
    }

    /// Returns the accepted page size.
    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }
}

/// Stable position after one Project Asset in descending list order.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct AssetListCursor {
    created_at: AssetCreatedAt,
    asset_id: AssetId,
}

impl AssetListCursor {
    /// Creates a cursor from one already-returned Asset position.
    #[must_use]
    pub const fn new(created_at: AssetCreatedAt, asset_id: AssetId) -> Self {
        Self { created_at, asset_id }
    }

    /// Returns the cursor creation time.
    #[must_use]
    pub const fn created_at(self) -> AssetCreatedAt {
        self.created_at
    }

    /// Returns the cursor Asset identity.
    #[must_use]
    pub const fn asset_id(self) -> AssetId {
        self.asset_id
    }
}

/// One stable bounded Project Asset list request.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct AssetListQuery {
    project_id: ProjectId,
    media_kind: Option<AssetMediaKind>,
    cursor: Option<AssetListCursor>,
    limit: AssetPageLimit,
}

impl AssetListQuery {
    /// Creates one fully validated list request.
    #[must_use]
    pub const fn new(
        project_id: ProjectId,
        media_kind: Option<AssetMediaKind>,
        cursor: Option<AssetListCursor>,
        limit: AssetPageLimit,
    ) -> Self {
        Self { project_id, media_kind, cursor, limit }
    }

    /// Returns the owning Project filter.
    #[must_use]
    pub const fn project_id(self) -> ProjectId {
        self.project_id
    }

    /// Returns the optional media-kind filter.
    #[must_use]
    pub const fn media_kind(self) -> Option<AssetMediaKind> {
        self.media_kind
    }

    /// Returns the exclusive descending-list position.
    #[must_use]
    pub const fn cursor(self) -> Option<AssetListCursor> {
        self.cursor
    }

    /// Returns the maximum requested item count.
    #[must_use]
    pub const fn limit(self) -> AssetPageLimit {
        self.limit
    }
}

/// One stable bounded Project Asset page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssetListPage {
    assets: Vec<AssetAggregate>,
    next_cursor: Option<AssetListCursor>,
}

impl AssetListPage {
    /// Creates a page from repository-ordered Assets.
    #[must_use]
    pub const fn new(assets: Vec<AssetAggregate>, next_cursor: Option<AssetListCursor>) -> Self {
        Self { assets, next_cursor }
    }

    /// Returns ordered Assets.
    #[must_use]
    pub fn assets(&self) -> &[AssetAggregate] {
        &self.assets
    }

    /// Returns the cursor for the next non-empty page.
    #[must_use]
    pub const fn next_cursor(&self) -> Option<AssetListCursor> {
        self.next_cursor
    }
}

macro_rules! copy_recovery_cursor {
    ($name:ident, $id_type:ty, $id_field:ident, $id_doc:literal) => {
        #[doc = "Stable ascending Asset recovery position."]
        #[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
        pub struct $name {
            created_at: AssetCreatedAt,
            $id_field: $id_type,
        }

        impl $name {
            /// Creates a cursor from one already-returned recovery item.
            #[must_use]
            pub const fn new(created_at: AssetCreatedAt, $id_field: $id_type) -> Self {
                Self { created_at, $id_field }
            }

            /// Returns the cursor creation time.
            #[must_use]
            pub const fn created_at(self) -> AssetCreatedAt {
                self.created_at
            }

            #[doc = $id_doc]
            #[must_use]
            pub const fn $id_field(self) -> $id_type {
                self.$id_field
            }
        }
    };
}

copy_recovery_cursor!(
    AssetFinalizationRecoveryCursor,
    AssetContentFinalizationId,
    finalization_id,
    "Returns the finalization identity."
);
copy_recovery_cursor!(
    AssetAvailableContentRecoveryCursor,
    AssetId,
    asset_id,
    "Returns the Asset identity."
);

/// Stable ascending position after one staged-content recovery item.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct AssetStagedContentRecoveryCursor {
    created_at: AssetCreatedAt,
    staged_content_ref: AssetStagedContentRef,
}

impl AssetStagedContentRecoveryCursor {
    /// Creates a cursor from one already-returned staged object.
    #[must_use]
    pub const fn new(
        created_at: AssetCreatedAt,
        staged_content_ref: AssetStagedContentRef,
    ) -> Self {
        Self { created_at, staged_content_ref }
    }

    /// Returns the cursor creation time.
    #[must_use]
    pub const fn created_at(&self) -> AssetCreatedAt {
        self.created_at
    }

    /// Returns the opaque staged-content identity.
    #[must_use]
    pub const fn staged_content_ref(&self) -> &AssetStagedContentRef {
        &self.staged_content_ref
    }
}

/// Ascending page of unfinished finalizations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssetContentFinalizationRecoveryPage {
    finalizations: Vec<AssetContentFinalization>,
    next_cursor: Option<AssetFinalizationRecoveryCursor>,
}

impl AssetContentFinalizationRecoveryPage {
    /// Creates a page from repository-ordered finalizations.
    #[must_use]
    pub const fn new(
        finalizations: Vec<AssetContentFinalization>,
        next_cursor: Option<AssetFinalizationRecoveryCursor>,
    ) -> Self {
        Self { finalizations, next_cursor }
    }

    /// Returns ordered unfinished finalizations.
    #[must_use]
    pub fn finalizations(&self) -> &[AssetContentFinalization] {
        &self.finalizations
    }

    /// Returns the next page cursor.
    #[must_use]
    pub const fn next_cursor(&self) -> Option<AssetFinalizationRecoveryCursor> {
        self.next_cursor
    }
}

/// Ascending page of Available Assets to verify.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssetAvailableContentRecoveryPage {
    assets: Vec<AssetAggregate>,
    next_cursor: Option<AssetAvailableContentRecoveryCursor>,
}

impl AssetAvailableContentRecoveryPage {
    /// Creates a page from repository-ordered Available Assets.
    #[must_use]
    pub const fn new(
        assets: Vec<AssetAggregate>,
        next_cursor: Option<AssetAvailableContentRecoveryCursor>,
    ) -> Self {
        Self { assets, next_cursor }
    }

    /// Returns ordered Available Assets.
    #[must_use]
    pub fn assets(&self) -> &[AssetAggregate] {
        &self.assets
    }

    /// Returns the next page cursor.
    #[must_use]
    pub const fn next_cursor(&self) -> Option<AssetAvailableContentRecoveryCursor> {
        self.next_cursor
    }
}

/// Ascending page of stale staged objects.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssetStagedContentRecoveryPage {
    staged_contents: Vec<AssetStagedContent>,
    next_cursor: Option<AssetStagedContentRecoveryCursor>,
}

impl AssetStagedContentRecoveryPage {
    /// Creates a page from store-ordered staged objects.
    #[must_use]
    pub const fn new(
        staged_contents: Vec<AssetStagedContent>,
        next_cursor: Option<AssetStagedContentRecoveryCursor>,
    ) -> Self {
        Self { staged_contents, next_cursor }
    }

    /// Returns ordered staged objects.
    #[must_use]
    pub fn staged_contents(&self) -> &[AssetStagedContent] {
        &self.staged_contents
    }

    /// Returns the next page cursor.
    #[must_use]
    pub const fn next_cursor(&self) -> Option<&AssetStagedContentRecoveryCursor> {
        self.next_cursor.as_ref()
    }
}
