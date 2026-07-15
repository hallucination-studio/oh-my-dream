//! Deterministic, project-scoped workflow result cache.

use crate::node::NodeRunResult;
use crate::registry::NodeParams;
use crate::value::{InputValue, NodeInputs, Value};
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};

/// In-memory node outputs retained across workflow runs.
#[derive(Default)]
pub struct ResultCache {
    entries: BTreeMap<CacheKey, CacheEntry>,
}

impl ResultCache {
    /// Creates an empty cache.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn get(
        &self,
        project_id: &str,
        node_id: &str,
        fingerprint: u64,
    ) -> Option<NodeRunResult> {
        let key = CacheKey { project_id: project_id.to_owned(), node_id: node_id.to_owned() };
        self.entries
            .get(&key)
            .filter(|entry| entry.fingerprint == fingerprint)
            .map(|entry| entry.result.clone())
    }

    pub(crate) fn insert(
        &mut self,
        project_id: &str,
        node_id: &str,
        fingerprint: u64,
        result: NodeRunResult,
    ) {
        let key = CacheKey { project_id: project_id.to_owned(), node_id: node_id.to_owned() };
        self.entries.insert(key, CacheEntry { fingerprint, result });
    }
}

#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
struct CacheKey {
    project_id: String,
    node_id: String,
}

#[derive(Clone)]
struct CacheEntry {
    fingerprint: u64,
    result: NodeRunResult,
}

pub(crate) fn cache_fingerprint(
    project_id: &str,
    type_id: &str,
    params: &NodeParams,
    inputs: &NodeInputs,
) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    project_id.hash(&mut hasher);
    type_id.hash(&mut hasher);
    hash_params(params, &mut hasher);
    hash_node_inputs(inputs, &mut hasher);
    hasher.finish()
}

fn hash_params(params: &NodeParams, state: &mut impl Hasher) {
    for (key, value) in params {
        key.hash(state);
        hash_json_value(value, state);
    }
}

fn hash_json_value(value: &serde_json::Value, state: &mut impl Hasher) {
    match value {
        serde_json::Value::Null => 0_u8.hash(state),
        serde_json::Value::Bool(value) => {
            1_u8.hash(state);
            value.hash(state);
        }
        serde_json::Value::Number(value) => hash_json_number(value, state),
        serde_json::Value::String(value) => {
            3_u8.hash(state);
            value.hash(state);
        }
        serde_json::Value::Array(values) => {
            4_u8.hash(state);
            for value in values {
                hash_json_value(value, state);
            }
        }
        serde_json::Value::Object(values) => {
            5_u8.hash(state);
            hash_params(values, state);
        }
    }
}

fn hash_json_number(value: &serde_json::Number, state: &mut impl Hasher) {
    2_u8.hash(state);
    if let Some(value) = value.as_i64() {
        0_u8.hash(state);
        value.hash(state);
    } else if let Some(value) = value.as_u64() {
        1_u8.hash(state);
        value.hash(state);
    } else if let Some(value) = value.as_f64() {
        2_u8.hash(state);
        value.to_bits().hash(state);
    }
}

fn hash_node_inputs(values: &NodeInputs, state: &mut impl Hasher) {
    for (key, value) in values {
        key.hash(state);
        match value {
            InputValue::Single(value) => {
                0_u8.hash(state);
                hash_value(value, state);
            }
            InputValue::OrderedMany(values) => {
                1_u8.hash(state);
                values.len().hash(state);
                for value in values {
                    hash_value(value, state);
                }
            }
        }
    }
}

fn hash_value(value: &Value, state: &mut impl Hasher) {
    match value {
        Value::String(value) => hash_tagged_string(0, value, state),
        Value::Image(value) => hash_tagged_string(1, value, state),
        Value::Video(value) => hash_tagged_string(2, value, state),
        Value::Audio(value) => hash_tagged_string(3, value, state),
        Value::Model(value) => hash_tagged_string(4, value, state),
        Value::Int(value) => {
            5_u8.hash(state);
            value.hash(state);
        }
        Value::Float(value) => {
            6_u8.hash(state);
            value.to_bits().hash(state);
        }
    }
}

fn hash_tagged_string(tag: u8, value: &str, state: &mut impl Hasher) {
    tag.hash(state);
    value.hash(state);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fingerprint(inputs: NodeInputs) -> u64 {
        cache_fingerprint("project", "Recording", &NodeParams::new(), &inputs)
    }

    #[test]
    fn distinguishes_scalar_and_ordered_cardinality() {
        let scalar = NodeInputs::from([(
            "clips".to_owned(),
            InputValue::Single(Value::Video("a".to_owned())),
        )]);
        let ordered = NodeInputs::from([(
            "clips".to_owned(),
            InputValue::OrderedMany(vec![Value::Video("a".to_owned())]),
        )]);
        assert_ne!(fingerprint(scalar), fingerprint(ordered));
    }

    #[test]
    fn distinguishes_order_from_delimiter_content() {
        let separated = NodeInputs::from([(
            "clips".to_owned(),
            InputValue::OrderedMany(vec![
                Value::Video("a".to_owned()),
                Value::Video("b|c".to_owned()),
            ]),
        )]);
        let joined = NodeInputs::from([(
            "clips".to_owned(),
            InputValue::OrderedMany(vec![
                Value::Video("a|b".to_owned()),
                Value::Video("c".to_owned()),
            ]),
        )]);
        assert_ne!(fingerprint(separated), fingerprint(joined));
    }

    #[test]
    fn distinguishes_reversed_ordered_values() {
        let forward = NodeInputs::from([(
            "clips".to_owned(),
            InputValue::OrderedMany(vec![
                Value::Video("a".to_owned()),
                Value::Video("b".to_owned()),
            ]),
        )]);
        let reverse = NodeInputs::from([(
            "clips".to_owned(),
            InputValue::OrderedMany(vec![
                Value::Video("b".to_owned()),
                Value::Video("a".to_owned()),
            ]),
        )]);
        assert_ne!(fingerprint(forward), fingerprint(reverse));
    }
}
