use std::collections::BTreeMap;

use engine::{
    node_capability::{
        NodeCapabilityOutputKey, WorkflowDataType, WorkflowInputItemId,
        WorkflowManagedAssetIdBoundaryValue, WorkflowManagedAudioRef,
        WorkflowManagedContentFingerprint, WorkflowManagedImageRef, WorkflowManagedVideoRef,
        WorkflowNodeOutputSet, WorkflowRuntimeValue, WorkflowTextPart, WorkflowTextValue,
    },
    workflow::WorkflowApplicationError,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::super::persistence;

#[derive(Serialize, Deserialize)]
pub(super) struct OutputSetPayload {
    expected: Vec<(String, DataTypePayload)>,
    values: Vec<(String, RuntimeValuePayload)>,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
enum DataTypePayload {
    Text,
    Image,
    Video,
    Audio,
}

#[derive(Serialize, Deserialize)]
enum RuntimeValuePayload {
    Text(Vec<TextPartPayload>),
    Image(MediaRefPayload),
    Video(MediaRefPayload),
    Audio(MediaRefPayload),
}

#[derive(Serialize, Deserialize)]
enum TextPartPayload {
    Literal(String),
    InputItemReference(Uuid),
}

#[derive(Serialize, Deserialize)]
struct MediaRefPayload {
    asset_id: [u8; 16],
    fingerprint: [u8; 32],
}

pub(super) fn encode_output_set(outputs: &WorkflowNodeOutputSet) -> OutputSetPayload {
    OutputSetPayload {
        expected: outputs
            .iter()
            .map(|(key, value)| (key.as_str().to_owned(), encode_data_type(value.data_type())))
            .collect(),
        values: outputs
            .iter()
            .map(|(key, value)| (key.as_str().to_owned(), encode_value(value)))
            .collect(),
    }
}

pub(super) fn decode_output_set(
    payload: OutputSetPayload,
) -> Result<WorkflowNodeOutputSet, WorkflowApplicationError> {
    let expected = payload
        .expected
        .into_iter()
        .map(|(key, data_type)| {
            Ok((
                NodeCapabilityOutputKey::new(key).map_err(|_| persistence())?,
                decode_data_type(data_type),
            ))
        })
        .collect::<Result<BTreeMap<_, _>, WorkflowApplicationError>>()?;
    let values = payload
        .values
        .into_iter()
        .map(|(key, value)| {
            Ok((
                NodeCapabilityOutputKey::new(key).map_err(|_| persistence())?,
                decode_value(value)?,
            ))
        })
        .collect::<Result<BTreeMap<_, _>, WorkflowApplicationError>>()?;
    WorkflowNodeOutputSet::try_restore(&expected, values).map_err(|_| persistence())
}

fn encode_value(value: &WorkflowRuntimeValue) -> RuntimeValuePayload {
    match value {
        WorkflowRuntimeValue::Text(text) => RuntimeValuePayload::Text(
            text.parts()
                .iter()
                .map(|part| match part {
                    WorkflowTextPart::Literal(value) => TextPartPayload::Literal(value.clone()),
                    WorkflowTextPart::InputItemReference(id) => {
                        TextPartPayload::InputItemReference(id.as_uuid())
                    }
                })
                .collect(),
        ),
        WorkflowRuntimeValue::Image(value) => {
            RuntimeValuePayload::Image(encode_media(value.asset_id(), value.content_fingerprint()))
        }
        WorkflowRuntimeValue::Video(value) => {
            RuntimeValuePayload::Video(encode_media(value.asset_id(), value.content_fingerprint()))
        }
        WorkflowRuntimeValue::Audio(value) => {
            RuntimeValuePayload::Audio(encode_media(value.asset_id(), value.content_fingerprint()))
        }
    }
}

fn decode_value(
    value: RuntimeValuePayload,
) -> Result<WorkflowRuntimeValue, WorkflowApplicationError> {
    match value {
        RuntimeValuePayload::Text(parts) => WorkflowTextValue::try_new(
            parts
                .into_iter()
                .map(|part| match part {
                    TextPartPayload::Literal(value) => Ok(WorkflowTextPart::Literal(value)),
                    TextPartPayload::InputItemReference(id) => WorkflowInputItemId::from_uuid(id)
                        .map(WorkflowTextPart::InputItemReference)
                        .ok_or_else(persistence),
                })
                .collect::<Result<Vec<_>, _>>()?,
        )
        .map(WorkflowRuntimeValue::Text)
        .map_err(|_| persistence()),
        RuntimeValuePayload::Image(value) => decode_media(value).map(|(id, digest)| {
            WorkflowRuntimeValue::Image(WorkflowManagedImageRef::new(id, digest))
        }),
        RuntimeValuePayload::Video(value) => decode_media(value).map(|(id, digest)| {
            WorkflowRuntimeValue::Video(WorkflowManagedVideoRef::new(id, digest))
        }),
        RuntimeValuePayload::Audio(value) => decode_media(value).map(|(id, digest)| {
            WorkflowRuntimeValue::Audio(WorkflowManagedAudioRef::new(id, digest))
        }),
    }
}

pub(crate) fn encode_value_bytes(
    value: &WorkflowRuntimeValue,
) -> Result<Vec<u8>, WorkflowApplicationError> {
    serde_json::to_vec(&encode_value(value)).map_err(|_| persistence())
}

pub(crate) fn decode_value_bytes(
    bytes: &[u8],
) -> Result<WorkflowRuntimeValue, WorkflowApplicationError> {
    let payload = serde_json::from_slice(bytes).map_err(|_| persistence())?;
    decode_value(payload)
}

pub(crate) fn restore_output_values(
    values: BTreeMap<NodeCapabilityOutputKey, WorkflowRuntimeValue>,
) -> Result<WorkflowNodeOutputSet, WorkflowApplicationError> {
    let expected = values
        .iter()
        .map(|(key, value)| (key.clone(), value.data_type()))
        .collect::<BTreeMap<_, _>>();
    WorkflowNodeOutputSet::try_restore(&expected, values).map_err(|_| persistence())
}

fn encode_media(
    asset_id: WorkflowManagedAssetIdBoundaryValue,
    fingerprint: WorkflowManagedContentFingerprint,
) -> MediaRefPayload {
    MediaRefPayload { asset_id: asset_id.as_bytes(), fingerprint: fingerprint.as_bytes() }
}

fn decode_media(
    value: MediaRefPayload,
) -> Result<
    (WorkflowManagedAssetIdBoundaryValue, WorkflowManagedContentFingerprint),
    WorkflowApplicationError,
> {
    Ok((
        WorkflowManagedAssetIdBoundaryValue::from_bytes(value.asset_id)
            .map_err(|_| persistence())?,
        WorkflowManagedContentFingerprint::from_bytes(value.fingerprint),
    ))
}

fn encode_data_type(value: WorkflowDataType) -> DataTypePayload {
    match value {
        WorkflowDataType::Text => DataTypePayload::Text,
        WorkflowDataType::Image => DataTypePayload::Image,
        WorkflowDataType::Video => DataTypePayload::Video,
        WorkflowDataType::Audio => DataTypePayload::Audio,
    }
}

fn decode_data_type(value: DataTypePayload) -> WorkflowDataType {
    match value {
        DataTypePayload::Text => WorkflowDataType::Text,
        DataTypePayload::Image => WorkflowDataType::Image,
        DataTypePayload::Video => WorkflowDataType::Video,
        DataTypePayload::Audio => WorkflowDataType::Audio,
    }
}
