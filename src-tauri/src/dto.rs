use assets::{Asset, AssetKind};
use engine::{RunOutputs, Value, ValueMap};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// JSON-friendly result returned by the `run_workflow` command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunWorkflowResultDto {
    /// Node id -> output name -> output value.
    pub outputs: BTreeMap<String, BTreeMap<String, RunOutputDto>>,
}

impl RunWorkflowResultDto {
    /// Converts engine outputs into the frontend run-output shape.
    #[must_use]
    pub fn from_outputs(outputs: &RunOutputs) -> Self {
        Self {
            outputs: outputs
                .iter()
                .map(|(node_id, values)| (node_id.clone(), value_map_to_dto(values)))
                .collect(),
        }
    }
}

/// A single output value as consumed by the frontend seam.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunOutputDto {
    /// Frontend output kind: `string`, `image`, or `video`.
    pub kind: String,
    /// String representation of the produced value.
    pub value: String,
}

/// Stored asset metadata as returned by Tauri commands.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssetDto {
    /// Unique asset id.
    pub id: String,
    /// `image` or `video`.
    pub kind: String,
    /// Stored media path.
    pub file_path: String,
    /// Stored thumbnail path, when available.
    pub thumbnail_path: Option<String>,
    /// Workflow snapshot captured when the asset was saved.
    pub workflow_snapshot: serde_json::Value,
    /// Source workflow node id.
    pub source_node_id: Option<String>,
    /// Free-form asset tags.
    pub tags: Vec<String>,
    /// Unix timestamp in seconds.
    pub created_at: i64,
}

impl From<Asset> for AssetDto {
    fn from(asset: Asset) -> Self {
        Self {
            id: asset.id,
            kind: asset_kind_as_str(asset.kind).to_owned(),
            file_path: asset.file_path,
            thumbnail_path: asset.thumbnail_path,
            workflow_snapshot: asset.workflow_snapshot,
            source_node_id: asset.source_node_id,
            tags: asset.tags,
            created_at: asset.created_at,
        }
    }
}

fn value_map_to_dto(values: &ValueMap) -> BTreeMap<String, RunOutputDto> {
    values.iter().map(|(name, value)| (name.clone(), run_output_to_dto(value))).collect()
}

fn run_output_to_dto(value: &Value) -> RunOutputDto {
    match value {
        Value::String(value) | Value::Model(value) => string_output(value),
        Value::Image(value) => media_output("image", value),
        Value::Video(value) => media_output("video", value),
        Value::Int(value) => string_output(&value.to_string()),
        Value::Float(value) => string_output(&value.to_string()),
    }
}

fn string_output(value: &str) -> RunOutputDto {
    media_output("string", value)
}

fn media_output(kind: &str, value: &str) -> RunOutputDto {
    RunOutputDto { kind: kind.to_owned(), value: value.to_owned() }
}

fn asset_kind_as_str(kind: AssetKind) -> &'static str {
    match kind {
        AssetKind::Image => "image",
        AssetKind::Video => "video",
    }
}
