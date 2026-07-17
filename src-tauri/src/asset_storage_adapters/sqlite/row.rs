use assets::asset::{application::*, domain::*};
use projects::project::domain::ProjectId;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

mod origin;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AssetBodyRow {
    state: StateRow,
    facts: FactsRow,
    origin: OriginRow,
    display_name: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind", deny_unknown_fields)]
enum StateRow {
    Pending { descriptor: DescriptorRow, finalization_id: [u8; 16] },
    Available { descriptor: DescriptorRow },
    Missing { descriptor: DescriptorRow, reason: u8 },
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DescriptorRow {
    digest: [u8; 32],
    byte_length: u64,
    mime: String,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct FinalizationBodyRow {
    descriptor: DescriptorRow,
    staged_ref: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind", deny_unknown_fields)]
enum FactsRow {
    Image { width: u32, height: u32 },
    Video { width: u32, height: u32, duration_ms: u64, has_audio: bool },
    Audio { duration_ms: u64, sample_rate_hz: u32, channels: u8 },
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind", deny_unknown_fields)]
enum OriginRow {
    Imported {
        import_id: [u8; 16],
        original_file_name: String,
    },
    WorkflowNodeOutput {
        workflow_id: [u8; 16],
        workflow_revision: u64,
        workflow_run_id: [u8; 16],
        workflow_node_id: [u8; 16],
        node_execution_id: [u8; 16],
        capability_id: String,
        capability_major: u16,
        capability_minor: u16,
        production: ProductionRow,
        output_key: String,
        ordinal: u32,
    },
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind", deny_unknown_fields)]
enum ProductionRow {
    ProviderGenerated { profile_id: String, profile_version: u32 },
    DeterministicDerived { source_asset_ids: Vec<[u8; 16]> },
    ProviderDerived { source_asset_ids: Vec<[u8; 16]>, profile_id: String, profile_version: u32 },
}

pub(super) struct AssetRow {
    pub asset_id: Vec<u8>,
    pub project_id: Vec<u8>,
    pub media_kind: i64,
    pub content_state: i64,
    pub created_at: i64,
    pub body_json: Vec<u8>,
}

pub(super) struct FinalizationRow {
    pub value: AssetContentFinalization,
    pub completed: bool,
}

pub(super) fn encode_asset(asset: &AssetAggregate) -> Result<Vec<u8>, AssetApplicationError> {
    let body = AssetBodyRow {
        state: StateRow::from_domain(asset.content_state()),
        facts: FactsRow::from_domain(asset.media_facts()),
        origin: OriginRow::from_domain(asset.origin()),
        display_name: asset.display_name().as_str().to_owned(),
    };
    serde_json::to_vec(&body).map_err(|_| storage())
}

pub(super) fn decode_asset(row: AssetRow) -> Result<AssetAggregate, AssetApplicationError> {
    let body: AssetBodyRow = serde_json::from_slice(&row.body_json).map_err(|_| storage())?;
    let media_kind = decode_media_kind(row.media_kind)?;
    let state = body.state.into_domain(media_kind)?;
    if encode_content_state(&state) != row.content_state {
        return Err(storage());
    }
    AssetAggregate::try_restore(
        AssetId::from_uuid(uuid(row.asset_id)?).map_err(|_| storage())?,
        ProjectId::from_uuid(uuid(row.project_id)?).ok_or_else(storage)?,
        media_kind,
        state,
        body.facts.into_domain()?,
        body.origin.into_domain()?,
        AssetDisplayName::try_new(body.display_name).map_err(|_| storage())?,
        AssetCreatedAt::from_utc_milliseconds(row.created_at).map_err(|_| storage())?,
    )
    .map_err(|_| storage())
}

pub(super) fn encode_finalization(
    value: &AssetContentFinalization,
) -> Result<Vec<u8>, AssetApplicationError> {
    serde_json::to_vec(&FinalizationBodyRow {
        descriptor: DescriptorRow::from_domain(value.descriptor()),
        staged_ref: hex(value.staged_content_ref().as_store_bytes()),
    })
    .map_err(|_| storage())
}

pub(super) fn decode_finalization(
    finalization_id: Vec<u8>,
    asset_id: Vec<u8>,
    created_at: i64,
    completed: i64,
    body: Vec<u8>,
) -> Result<FinalizationRow, AssetApplicationError> {
    let body: FinalizationBodyRow = serde_json::from_slice(&body).map_err(|_| storage())?;
    Ok(FinalizationRow {
        value: AssetContentFinalization::new(
            AssetContentFinalizationId::from_uuid(uuid(finalization_id)?).map_err(|_| storage())?,
            AssetId::from_uuid(uuid(asset_id)?).map_err(|_| storage())?,
            body.descriptor.into_domain()?,
            AssetStagedContentRef::try_from_store_bytes(unhex(&body.staged_ref)?)?,
            AssetCreatedAt::from_utc_milliseconds(created_at).map_err(|_| storage())?,
        ),
        completed: match completed {
            0 => false,
            1 => true,
            _ => return Err(storage()),
        },
    })
}

pub(super) const fn encode_media_kind(value: AssetMediaKind) -> i64 {
    match value {
        AssetMediaKind::Image => 0,
        AssetMediaKind::Video => 1,
        AssetMediaKind::Audio => 2,
    }
}

pub(super) const fn encode_content_state(value: &AssetManagedContentState) -> i64 {
    match value {
        AssetManagedContentState::Pending { .. } => 0,
        AssetManagedContentState::Available { .. } => 1,
        AssetManagedContentState::Missing { .. } => 2,
    }
}

impl StateRow {
    fn from_domain(value: &AssetManagedContentState) -> Self {
        match value {
            AssetManagedContentState::Pending { descriptor, finalization_id } => Self::Pending {
                descriptor: DescriptorRow::from_domain(descriptor),
                finalization_id: *finalization_id.as_uuid().as_bytes(),
            },
            AssetManagedContentState::Available { descriptor } => {
                Self::Available { descriptor: DescriptorRow::from_domain(descriptor) }
            }
            AssetManagedContentState::Missing { expected, reason } => Self::Missing {
                descriptor: DescriptorRow::from_domain(expected),
                reason: match reason {
                    AssetContentMissingReason::FinalizationSourceMissing => 1,
                    AssetContentMissingReason::ManagedContentMissing => 2,
                },
            },
        }
    }

    fn into_domain(
        self,
        media_kind: AssetMediaKind,
    ) -> Result<AssetManagedContentState, AssetApplicationError> {
        match self {
            Self::Pending { descriptor, finalization_id } => {
                Ok(AssetManagedContentState::Pending {
                    descriptor: descriptor.into_domain()?,
                    finalization_id: AssetContentFinalizationId::from_uuid(Uuid::from_bytes(
                        finalization_id,
                    ))
                    .map_err(|_| storage())?,
                })
            }
            Self::Available { descriptor } => {
                Ok(AssetManagedContentState::Available { descriptor: descriptor.into_domain()? })
            }
            Self::Missing { descriptor, reason } => Ok(AssetManagedContentState::Missing {
                expected: descriptor.into_domain()?,
                reason: match reason {
                    1 => AssetContentMissingReason::FinalizationSourceMissing,
                    2 => AssetContentMissingReason::ManagedContentMissing,
                    _ => return Err(storage()),
                },
            }),
        }
        .and_then(|state| {
            if state.descriptor().media_kind() == media_kind { Ok(state) } else { Err(storage()) }
        })
    }
}

impl DescriptorRow {
    fn from_domain(value: &AssetContentDescriptor) -> Self {
        Self {
            digest: value.digest().as_bytes(),
            byte_length: value.byte_length(),
            mime: value.mime_type().as_str().to_owned(),
        }
    }

    fn into_domain(self) -> Result<AssetContentDescriptor, AssetApplicationError> {
        let digest = AssetContentDigest::from_bytes(self.digest);
        let mime = decode_mime(&self.mime)?;
        AssetContentDescriptor::try_new(
            AssetManagedContentId::from_digest(digest),
            digest,
            self.byte_length,
            mime,
            mime.media_kind(),
        )
        .map_err(|_| storage())
    }
}

impl FactsRow {
    fn from_domain(value: AssetMediaFacts) -> Self {
        match value {
            AssetMediaFacts::Image(value) => {
                Self::Image { width: value.width(), height: value.height() }
            }
            AssetMediaFacts::Video(value) => Self::Video {
                width: value.width(),
                height: value.height(),
                duration_ms: value.duration_ms(),
                has_audio: value.has_audio(),
            },
            AssetMediaFacts::Audio(value) => Self::Audio {
                duration_ms: value.duration_ms(),
                sample_rate_hz: value.sample_rate_hz(),
                channels: value.channels(),
            },
        }
    }

    fn into_domain(self) -> Result<AssetMediaFacts, AssetApplicationError> {
        match self {
            Self::Image { width, height } => AssetMediaFacts::try_image(width, height),
            Self::Video { width, height, duration_ms, has_audio } => {
                AssetMediaFacts::try_video(width, height, duration_ms, has_audio)
            }
            Self::Audio { duration_ms, sample_rate_hz, channels } => {
                AssetMediaFacts::try_audio(duration_ms, sample_rate_hz, channels)
            }
        }
        .map_err(|_| storage())
    }
}

fn decode_media_kind(value: i64) -> Result<AssetMediaKind, AssetApplicationError> {
    match value {
        0 => Ok(AssetMediaKind::Image),
        1 => Ok(AssetMediaKind::Video),
        2 => Ok(AssetMediaKind::Audio),
        _ => Err(storage()),
    }
}

fn decode_mime(value: &str) -> Result<AssetMediaMimeType, AssetApplicationError> {
    match value {
        "image/png" => Ok(AssetMediaMimeType::ImagePng),
        "image/jpeg" => Ok(AssetMediaMimeType::ImageJpeg),
        "image/webp" => Ok(AssetMediaMimeType::ImageWebp),
        "video/mp4" => Ok(AssetMediaMimeType::VideoMp4),
        "video/webm" => Ok(AssetMediaMimeType::VideoWebm),
        "audio/mpeg" => Ok(AssetMediaMimeType::AudioMpeg),
        "audio/wav" => Ok(AssetMediaMimeType::AudioWav),
        "audio/ogg" => Ok(AssetMediaMimeType::AudioOgg),
        _ => Err(storage()),
    }
}

fn uuid(bytes: Vec<u8>) -> Result<Uuid, AssetApplicationError> {
    Uuid::from_slice(&bytes).map_err(|_| storage())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn unhex(value: &str) -> Result<Vec<u8>, AssetApplicationError> {
    if !value.len().is_multiple_of(2)
        || !value.bytes().all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(storage());
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let text = std::str::from_utf8(pair).map_err(|_| storage())?;
            u8::from_str_radix(text, 16).map_err(|_| storage())
        })
        .collect()
}

fn storage() -> AssetApplicationError {
    AssetApplicationError::ManagedStorageFailed
}
