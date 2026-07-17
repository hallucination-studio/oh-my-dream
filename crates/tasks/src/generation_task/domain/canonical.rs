//! Canonical encoding of immutable Generation Task admission facts.

use assets::asset::domain::AssetMediaKind;
use sha2::{Digest, Sha256};

use super::{
    AssetSnapshotRef, GenerationTaskOrigin, GenerationTaskRequest, GenerationTaskRequestHash,
    GenerationTaskTarget, GenerationTaskText, ImageAspectRatio, VideoDurationSeconds,
};

const GENERATION_TASK_REQUEST_SCHEMA_VERSION: u32 = 1;

pub(super) fn canonical_request_hash(
    origin: &GenerationTaskOrigin,
    request: &GenerationTaskRequest,
    target: &GenerationTaskTarget,
) -> GenerationTaskRequestHash {
    let mut bytes = Vec::new();
    append_u32(&mut bytes, GENERATION_TASK_REQUEST_SCHEMA_VERSION);
    bytes.extend_from_slice(origin.project_id().as_uuid().as_bytes());
    bytes.extend_from_slice(origin.workflow_id().as_uuid().as_bytes());
    bytes.extend_from_slice(origin.workflow_run_id().as_uuid().as_bytes());
    bytes.extend_from_slice(origin.workflow_node_id().as_uuid().as_bytes());
    bytes.extend_from_slice(origin.workflow_node_execution_id().as_uuid().as_bytes());
    append_request(&mut bytes, request);
    append_text(&mut bytes, target.generation_profile_ref().id().as_str());
    append_u32(&mut bytes, target.generation_profile_ref().version().get());
    append_text(&mut bytes, target.provider_id().as_str());
    append_text(&mut bytes, target.route_id().as_str());
    GenerationTaskRequestHash::from_bytes(Sha256::digest(bytes).into())
}

fn append_request(bytes: &mut Vec<u8>, request: &GenerationTaskRequest) {
    match request {
        GenerationTaskRequest::Text(spec) => {
            bytes.push(1);
            append_text(bytes, spec.prompt().as_str());
        }
        GenerationTaskRequest::Image(spec) => {
            bytes.push(2);
            append_text(bytes, spec.prompt().as_str());
            bytes.push(aspect_ratio_tag(spec.aspect_ratio()));
        }
        GenerationTaskRequest::Voice(spec) => {
            bytes.push(3);
            append_text(bytes, spec.text().as_str());
        }
        GenerationTaskRequest::Video(spec) => {
            bytes.push(4);
            append_asset_snapshot(bytes, spec.input_image());
            bytes.push(duration_tag(spec.duration_seconds()));
            append_optional_text(bytes, spec.prompt().map(GenerationTaskText::as_str));
        }
    }
}

fn append_asset_snapshot(bytes: &mut Vec<u8>, snapshot: AssetSnapshotRef) {
    bytes.extend_from_slice(snapshot.asset_id().as_uuid().as_bytes());
    bytes.push(media_kind_tag(snapshot.media_kind()));
    bytes.extend_from_slice(&snapshot.content_hash().as_bytes());
}

fn append_optional_text(bytes: &mut Vec<u8>, value: Option<&str>) {
    match value {
        Some(value) => {
            bytes.push(1);
            append_text(bytes, value);
        }
        None => bytes.push(0),
    }
}

fn append_text(bytes: &mut Vec<u8>, value: &str) {
    bytes.extend_from_slice(&(value.len() as u64).to_be_bytes());
    bytes.extend_from_slice(value.as_bytes());
}

fn append_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_be_bytes());
}

const fn aspect_ratio_tag(value: ImageAspectRatio) -> u8 {
    match value {
        ImageAspectRatio::Square => 1,
        ImageAspectRatio::Landscape4To3 => 2,
        ImageAspectRatio::Portrait3To4 => 3,
        ImageAspectRatio::Landscape16To9 => 4,
        ImageAspectRatio::Portrait9To16 => 5,
    }
}

const fn duration_tag(value: VideoDurationSeconds) -> u8 {
    match value {
        VideoDurationSeconds::Five => 1,
        VideoDurationSeconds::Ten => 2,
    }
}

const fn media_kind_tag(value: AssetMediaKind) -> u8 {
    match value {
        AssetMediaKind::Image => 1,
        AssetMediaKind::Video => 2,
        AssetMediaKind::Audio => 3,
    }
}
