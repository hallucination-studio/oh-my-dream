//! Asset data model, mirroring the SQLite schema in docs/DESIGN.md §7.

use serde::{Deserialize, Serialize};

/// The kind of media an asset holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetKind {
    /// A still image.
    Image,
    /// A video clip.
    Video,
}

/// A stored asset with its metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Asset {
    /// Unique asset id.
    pub id: String,
    /// Media kind.
    pub kind: AssetKind,
    /// Path to the media file on disk.
    pub file_path: String,
    /// Path to the generated thumbnail, if any.
    pub thumbnail_path: Option<String>,
    /// Snapshot of the workflow params that produced this asset, enabling
    /// "trace this asset back to its recipe".
    pub workflow_snapshot: serde_json::Value,
    /// The workflow node id that produced this asset.
    pub source_node_id: Option<String>,
    /// Free-form tags for filtering.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Creation time as a Unix timestamp (seconds).
    pub created_at: i64,
}

/// The fields needed to insert a new asset; `id` and `created_at` are assigned
/// by the store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewAsset {
    /// Media kind.
    pub kind: AssetKind,
    /// Path to the media file on disk.
    pub file_path: String,
    /// Snapshot of the workflow params that produced this asset.
    pub workflow_snapshot: serde_json::Value,
    /// The workflow node id that produced this asset.
    pub source_node_id: Option<String>,
    /// Free-form tags for filtering.
    #[serde(default)]
    pub tags: Vec<String>,
}
