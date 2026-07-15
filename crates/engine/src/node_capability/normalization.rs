//! Complete normalized parameter sets and canonical domain encoding.

use super::{
    NodeCapabilityNormalizedParameterMap, NodeCapabilityParameterKey, NodeCapabilityParameterSet,
    NodeCapabilityParameterValue,
};

/// Complete validated and defaulted parameter set.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeCapabilityNormalizedParameters(NodeCapabilityParameterSet);

impl NodeCapabilityNormalizedParameters {
    /// Returns one normalized value.
    #[must_use]
    pub fn get(&self, key: &NodeCapabilityParameterKey) -> Option<&NodeCapabilityParameterValue> {
        self.0.get(key)
    }

    /// Returns the number of normalized values.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Reports whether no normalized values are present.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Encodes values by ascending key using the frozen domain format.
    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        append_u32(&mut bytes, self.0.len() as u32);
        for (key, value) in self.0.iter() {
            append_variable_bytes(&mut bytes, key.as_str().as_bytes());
            append_parameter_value(&mut bytes, value);
        }
        bytes
    }

    pub(crate) const fn from_validated(values: NodeCapabilityNormalizedParameterMap) -> Self {
        Self(NodeCapabilityParameterSet::from_validated_map(values))
    }
}

fn append_parameter_value(bytes: &mut Vec<u8>, value: &NodeCapabilityParameterValue) {
    match value {
        NodeCapabilityParameterValue::UnsignedInteger(value) => {
            bytes.push(0);
            bytes.extend_from_slice(&value.to_be_bytes());
        }
        NodeCapabilityParameterValue::Text(value) => {
            bytes.push(1);
            append_variable_bytes(bytes, value.as_bytes());
        }
        NodeCapabilityParameterValue::Choice(value) => {
            bytes.push(2);
            append_variable_bytes(bytes, value.as_str().as_bytes());
        }
        NodeCapabilityParameterValue::GenerationProfile(value) => {
            bytes.push(3);
            append_variable_bytes(bytes, value.profile_id().as_bytes());
            bytes.extend_from_slice(&value.version().to_be_bytes());
        }
        NodeCapabilityParameterValue::ManagedAsset(value) => {
            bytes.push(4);
            bytes.extend_from_slice(&value.asset_id().as_bytes());
        }
    }
}

fn append_variable_bytes(target: &mut Vec<u8>, value: &[u8]) {
    append_u32(target, value.len() as u32);
    target.extend_from_slice(value);
}

fn append_u32(target: &mut Vec<u8>, value: u32) {
    target.extend_from_slice(&value.to_be_bytes());
}
