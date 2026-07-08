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
    /// An audio clip.
    Audio,
}

/// Sort order for asset list queries.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetSort {
    /// Newest assets first.
    #[default]
    Newest,
    /// Oldest assets first.
    Oldest,
    /// Highest cost first.
    CostDesc,
    /// Lowest cost first.
    CostAsc,
}

/// Filters for listing stored assets.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetQuery {
    /// Optional media kind filter.
    #[serde(default)]
    pub kind: Option<AssetKind>,
    /// Optional project id filter.
    #[serde(default)]
    pub project_id: Option<String>,
    /// Optional model filter.
    #[serde(default)]
    pub model: Option<String>,
    /// Optional prompt text search.
    #[serde(default)]
    pub prompt: Option<String>,
    /// Sort order.
    #[serde(default)]
    pub sort: AssetSort,
}

/// A persisted project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    /// Unique project id.
    pub id: String,
    /// User-visible project name.
    pub name: String,
    /// Creation time as a Unix timestamp (seconds).
    pub created_at: i64,
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
    /// Prompt text that ultimately produced this asset, when available.
    pub prompt: Option<String>,
    /// Project id this asset belongs to.
    pub project_id: Option<String>,
    /// Project name captured for display denormalization.
    pub project_name: Option<String>,
    /// The workflow node id that produced this asset.
    pub source_node_id: Option<String>,
    /// The workflow node type that produced this asset.
    pub source_node_type: Option<String>,
    /// Model identifier used to produce this asset.
    pub model: Option<String>,
    /// Seed used to produce this asset.
    pub seed: Option<u64>,
    /// Estimated cost in micro-USD.
    pub cost: Option<i64>,
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
    /// Prompt text that ultimately produced this asset, when available.
    #[serde(default)]
    pub prompt: Option<String>,
    /// Project id this asset belongs to.
    #[serde(default)]
    pub project_id: Option<String>,
    /// Project name captured for display denormalization.
    #[serde(default)]
    pub project_name: Option<String>,
    /// The workflow node id that produced this asset.
    #[serde(default)]
    pub source_node_id: Option<String>,
    /// The workflow node type that produced this asset.
    #[serde(default)]
    pub source_node_type: Option<String>,
    /// Model identifier used to produce this asset.
    #[serde(default)]
    pub model: Option<String>,
    /// Seed used to produce this asset.
    #[serde(default)]
    pub seed: Option<u64>,
    /// Estimated cost in micro-USD.
    #[serde(default)]
    pub cost: Option<i64>,
    /// Free-form tags for filtering.
    #[serde(default)]
    pub tags: Vec<String>,
}
