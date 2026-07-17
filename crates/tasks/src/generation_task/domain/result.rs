//! Closed single-result Generation Task values.

use assets::asset::domain::{AssetId, AssetMediaKind};

use super::GenerationTaskText;

/// Durable media-only result of one Generation Task.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct GenerationTaskAssetResult {
    asset_id: AssetId,
    media_kind: AssetMediaKind,
}

impl GenerationTaskAssetResult {
    /// Combines one durable Asset identity with its exact media kind.
    #[must_use]
    pub const fn new(asset_id: AssetId, media_kind: AssetMediaKind) -> Self {
        Self { asset_id, media_kind }
    }

    /// Returns the logical Asset identity.
    #[must_use]
    pub const fn asset_id(&self) -> AssetId {
        self.asset_id
    }

    /// Returns the exact generated media kind.
    #[must_use]
    pub const fn media_kind(&self) -> AssetMediaKind {
        self.media_kind
    }
}

/// Single durable Generation Task result.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum GenerationTaskResult {
    /// Inline generated Text.
    Text {
        /// Exact generated content.
        content: GenerationTaskText,
    },
    /// One durable generated Asset reference.
    Asset(GenerationTaskAssetResult),
}
