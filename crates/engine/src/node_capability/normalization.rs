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
        self.0.canonical_bytes()
    }

    pub(crate) const fn from_validated(values: NodeCapabilityNormalizedParameterMap) -> Self {
        Self(NodeCapabilityParameterSet::from_validated_map(values))
    }
}
