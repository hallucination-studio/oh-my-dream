//! Canonical Asset projections returned by Desktop commands.

use assets::asset::{
    application::AssetListPage,
    domain::{
        AssetAggregate, AssetManagedContentState, AssetMediaFacts, AssetMediaKind, AssetOrigin,
    },
};
use serde::Serialize;
use serde_json::{Value, json};

/// One Project-local managed media Asset without paths or preview tokens.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AssetDto {
    pub asset_id: String,
    pub project_id: String,
    pub media_kind: String,
    pub content_state: String,
    pub display_name: String,
    pub created_at_epoch_ms: String,
    pub content: AssetContentDto,
    pub media_facts: Value,
    pub origin: Value,
}

/// Exact immutable managed-content facts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AssetContentDto {
    pub content_fingerprint_hex: String,
    pub byte_length: String,
    pub mime_type: String,
}

/// One bounded stable Asset page.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AssetListPageDto {
    pub assets: Vec<AssetDto>,
    pub next_cursor: Option<String>,
}

/// One short-lived signed preview projection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AssetPreviewDto {
    pub asset_id: String,
    pub preview_uri: String,
    pub expires_at_epoch_ms: String,
}

pub(crate) fn asset_dto(asset: &AssetAggregate) -> AssetDto {
    let descriptor = asset.content_state().descriptor();
    AssetDto {
        asset_id: asset.id().as_uuid().hyphenated().to_string(),
        project_id: asset.project_id().as_uuid().hyphenated().to_string(),
        media_kind: media_kind(asset.media_kind()).to_owned(),
        content_state: content_state(asset.content_state()).to_owned(),
        display_name: asset.display_name().as_str().to_owned(),
        created_at_epoch_ms: asset.created_at().as_utc_milliseconds().to_string(),
        content: AssetContentDto {
            content_fingerprint_hex: hex(descriptor.digest().as_bytes()),
            byte_length: descriptor.byte_length().to_string(),
            mime_type: descriptor.mime_type().as_str().to_owned(),
        },
        media_facts: media_facts(asset.media_facts()),
        origin: origin(asset.origin()),
    }
}

pub(crate) fn page_dto(
    page: &AssetListPage,
    encode_cursor: impl Fn(assets::asset::application::AssetListCursor) -> String,
) -> AssetListPageDto {
    AssetListPageDto {
        assets: page.assets().iter().map(asset_dto).collect(),
        next_cursor: page.next_cursor().map(encode_cursor),
    }
}

pub(crate) const fn media_kind(value: AssetMediaKind) -> &'static str {
    match value {
        AssetMediaKind::Image => "image",
        AssetMediaKind::Video => "video",
        AssetMediaKind::Audio => "audio",
    }
}

fn content_state(value: &AssetManagedContentState) -> &'static str {
    match value {
        AssetManagedContentState::Pending { .. } => "pending",
        AssetManagedContentState::Available { .. } => "available",
        AssetManagedContentState::Missing { .. } => "missing",
    }
}

fn media_facts(value: AssetMediaFacts) -> Value {
    match value {
        AssetMediaFacts::Image(value) => {
            json!({"kind":"image","width":value.width(),"height":value.height()})
        }
        AssetMediaFacts::Video(value) => json!({
            "kind":"video",
            "width":value.width(),
            "height":value.height(),
            "duration_ms":value.duration_ms().to_string(),
            "has_audio":value.has_audio(),
        }),
        AssetMediaFacts::Audio(value) => json!({
            "kind":"audio",
            "duration_ms":value.duration_ms().to_string(),
            "sample_rate_hz":value.sample_rate_hz(),
            "channels":value.channels(),
        }),
    }
}

fn origin(value: &AssetOrigin) -> Value {
    match value {
        AssetOrigin::Imported(value) => json!({
            "kind":"imported",
            "original_file_name":value.original_file_name().as_str(),
        }),
        AssetOrigin::WorkflowNodeOutput(value) => {
            let producer = value.producer();
            json!({
                "kind":"workflow_node_output",
                "workflow_id":producer.workflow_id().as_uuid().hyphenated().to_string(),
                "workflow_revision":producer.workflow_revision().get().to_string(),
                "workflow_run_id":producer.workflow_run_id().as_uuid().hyphenated().to_string(),
                "workflow_node_id":producer.workflow_node_id().as_uuid().hyphenated().to_string(),
                "node_execution_id":producer.node_execution_id().as_uuid().hyphenated().to_string(),
            })
        }
    }
}

fn hex(bytes: [u8; 32]) -> String {
    let mut value = String::with_capacity(64);
    for byte in bytes {
        use std::fmt::Write;
        let _ = write!(value, "{byte:02x}");
    }
    value
}
