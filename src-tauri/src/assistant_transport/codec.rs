use serde::{
    Deserialize, Deserializer, Serialize,
    de::{MapAccess, SeqAccess, Visitor},
};
use serde_json::{Map, Value};

use super::{
    AssistantFrame, AssistantFrameKind, AssistantTransportError, MAX_FRAME_SIZE, MAX_JSON_DEPTH,
    MAX_SAFE_JSON_NUMBER, PROTOCOL_VERSION,
};

const TOP_LEVEL_KEYS: [&str; 4] = ["protocol_version", "sequence", "kind", "payload"];

pub(super) fn decode_frame(json: &str) -> Result<AssistantFrame, AssistantTransportError> {
    let UniqueJson(value) = serde_json::from_str(json)
        .map_err(|source| AssistantTransportError::MalformedJson { source })?;
    validate_json_depth(&value, 0)?;
    validate_json_numbers(&value)?;
    frame_from_value(value)
}

pub(super) fn encode_frame(
    frame: &AssistantFrame,
    expected_sequence: u64,
) -> Result<Vec<u8>, AssistantTransportError> {
    validate_safe_u64(frame.sequence)?;
    validate_json_depth(&frame.payload, 1)?;
    validate_json_numbers(&frame.payload)?;
    if frame.sequence != expected_sequence {
        return Err(AssistantTransportError::UnexpectedSequence {
            expected: expected_sequence,
            actual: frame.sequence,
        });
    }
    if !frame.payload.is_object() {
        return Err(AssistantTransportError::PayloadMustBeObject);
    }

    let mut encoded = serde_json::to_vec(&WireFrame::from(frame))
        .map_err(|source| AssistantTransportError::Encode { source })?;
    encoded.push(b'\n');
    if encoded.len() > MAX_FRAME_SIZE {
        return Err(AssistantTransportError::FrameTooLarge {
            observed_size: encoded.len(),
            max: MAX_FRAME_SIZE,
        });
    }
    Ok(encoded)
}

fn frame_from_value(value: Value) -> Result<AssistantFrame, AssistantTransportError> {
    let object = value.as_object().ok_or(AssistantTransportError::TopLevelMustBeObject)?;
    validate_keys(object)?;
    let protocol_version = required_u64(object, "protocol_version")?;
    if protocol_version != PROTOCOL_VERSION {
        return Err(AssistantTransportError::UnsupportedProtocolVersion {
            actual: protocol_version,
        });
    }
    let sequence = required_u64(object, "sequence")?;
    let kind_name = required_str(object, "kind")?;
    let kind = serde_json::from_value(Value::String(kind_name.to_owned()))
        .map_err(|_| AssistantTransportError::UnknownKind { kind: kind_name.to_owned() })?;
    let payload = required_value(object, "payload")?.clone();
    AssistantFrame::new(sequence, kind, payload)
}

fn validate_json_numbers(value: &Value) -> Result<(), AssistantTransportError> {
    match value {
        Value::Number(number) => validate_number(number),
        Value::Array(values) => values.iter().try_for_each(validate_json_numbers),
        Value::Object(values) => values.values().try_for_each(validate_json_numbers),
        Value::Null | Value::Bool(_) | Value::String(_) => Ok(()),
    }
}

fn validate_json_depth(value: &Value, depth: usize) -> Result<(), AssistantTransportError> {
    match value {
        Value::Array(values) => {
            validate_container_depth(depth)?;
            values.iter().try_for_each(|child| validate_json_depth(child, depth + 1))
        }
        Value::Object(values) => {
            validate_container_depth(depth)?;
            values.values().try_for_each(|child| validate_json_depth(child, depth + 1))
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => Ok(()),
    }
}

fn validate_container_depth(depth: usize) -> Result<(), AssistantTransportError> {
    if depth <= MAX_JSON_DEPTH {
        Ok(())
    } else {
        Err(AssistantTransportError::JsonDepthExceeded { maximum: MAX_JSON_DEPTH })
    }
}

fn validate_number(number: &serde_json::Number) -> Result<(), AssistantTransportError> {
    let in_range = if let Some(value) = number.as_i64() {
        value.unsigned_abs() <= MAX_SAFE_JSON_NUMBER
    } else if let Some(value) = number.as_u64() {
        value <= MAX_SAFE_JSON_NUMBER
    } else if let Some(value) = number.as_f64() {
        value.is_finite() && value.abs() <= MAX_SAFE_JSON_NUMBER as f64
    } else {
        false
    };
    if in_range { Ok(()) } else { Err(number_out_of_range()) }
}

fn validate_safe_u64(value: u64) -> Result<(), AssistantTransportError> {
    if value <= MAX_SAFE_JSON_NUMBER { Ok(()) } else { Err(number_out_of_range()) }
}

fn number_out_of_range() -> AssistantTransportError {
    AssistantTransportError::JsonNumberOutOfRange { maximum: MAX_SAFE_JSON_NUMBER }
}

fn validate_keys(object: &Map<String, Value>) -> Result<(), AssistantTransportError> {
    if let Some(key) = object.keys().find(|key| !TOP_LEVEL_KEYS.contains(&key.as_str())) {
        return Err(AssistantTransportError::UnknownTopLevelKey { key: key.clone() });
    }
    for key in TOP_LEVEL_KEYS {
        if !object.contains_key(key) {
            return Err(AssistantTransportError::MissingTopLevelKey { key });
        }
    }
    Ok(())
}

fn required_value<'a>(
    object: &'a Map<String, Value>,
    key: &'static str,
) -> Result<&'a Value, AssistantTransportError> {
    object.get(key).ok_or(AssistantTransportError::MissingTopLevelKey { key })
}

fn required_u64(
    object: &Map<String, Value>,
    key: &'static str,
) -> Result<u64, AssistantTransportError> {
    required_value(object, key)?.as_u64().ok_or(AssistantTransportError::InvalidFieldType {
        field: key,
        expected: "a non-negative u64",
    })
}

fn required_str<'a>(
    object: &'a Map<String, Value>,
    key: &'static str,
) -> Result<&'a str, AssistantTransportError> {
    required_value(object, key)?
        .as_str()
        .ok_or(AssistantTransportError::InvalidFieldType { field: key, expected: "a string" })
}

#[derive(Serialize)]
struct WireFrame<'a> {
    protocol_version: u64,
    sequence: u64,
    kind: AssistantFrameKind,
    payload: &'a Value,
}

impl<'a> From<&'a AssistantFrame> for WireFrame<'a> {
    fn from(frame: &'a AssistantFrame) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            sequence: frame.sequence,
            kind: frame.kind,
            payload: &frame.payload,
        }
    }
}

struct UniqueJson(Value);

impl<'de> Deserialize<'de> for UniqueJson {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct UniqueJsonVisitor;

        impl<'de> Visitor<'de> for UniqueJsonVisitor {
            type Value = Value;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a JSON value without duplicate object keys")
            }

            fn visit_bool<E>(self, value: bool) -> Result<Value, E> {
                Ok(Value::Bool(value))
            }

            fn visit_i64<E>(self, value: i64) -> Result<Value, E> {
                Ok(Value::Number(value.into()))
            }

            fn visit_u64<E>(self, value: u64) -> Result<Value, E> {
                Ok(Value::Number(value.into()))
            }

            fn visit_f64<E: serde::de::Error>(self, value: f64) -> Result<Value, E> {
                serde_json::Number::from_f64(value)
                    .map(Value::Number)
                    .ok_or_else(|| E::custom("JSON number must be finite"))
            }

            fn visit_str<E>(self, value: &str) -> Result<Value, E> {
                Ok(Value::String(value.to_owned()))
            }

            fn visit_string<E>(self, value: String) -> Result<Value, E> {
                Ok(Value::String(value))
            }

            fn visit_unit<E>(self) -> Result<Value, E> {
                Ok(Value::Null)
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Value, A::Error> {
                let mut values = Vec::new();
                while let Some(UniqueJson(value)) = sequence.next_element()? {
                    values.push(value);
                }
                Ok(Value::Array(values))
            }

            fn visit_map<A: MapAccess<'de>>(self, mut entries: A) -> Result<Value, A::Error> {
                let mut values = Map::new();
                while let Some(key) = entries.next_key::<String>()? {
                    if values.contains_key(&key) {
                        return Err(serde::de::Error::custom("duplicate JSON object key"));
                    }
                    let UniqueJson(value) = entries.next_value()?;
                    values.insert(key, value);
                }
                Ok(Value::Object(values))
            }
        }

        deserializer.deserialize_any(UniqueJsonVisitor).map(Self)
    }
}
