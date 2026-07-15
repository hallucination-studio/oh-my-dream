//! Closed parameter values, constraints, and normalization.

use std::collections::{BTreeMap, BTreeSet};

use super::{
    NodeCapabilityChoiceKey, NodeCapabilityContractError,
    NodeCapabilityGenerationProfileRefParameterValue, NodeCapabilityManagedAssetIdParameterValue,
    NodeCapabilityParameterError, NodeCapabilityParameterErrorCategory,
    NodeCapabilityParameterErrorTarget, NodeCapabilityParameterKey,
};

/// Exact runtime data types supported by the frozen MVP.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum WorkflowDataType {
    /// Structured text.
    Text,
    /// Managed image.
    Image,
    /// Managed video.
    Video,
    /// Managed audio.
    Audio,
}

/// Closed value accepted by a node parameter.
#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum NodeCapabilityParameterValue {
    /// Unsigned integer value.
    UnsignedInteger(u64),
    /// UTF-8 text value.
    Text(String),
    /// Capability-owned choice key.
    Choice(NodeCapabilityChoiceKey),
    /// Provider-independent Generation Profile ref boundary value.
    GenerationProfile(NodeCapabilityGenerationProfileRefParameterValue),
    /// Asset ID boundary value.
    ManagedAsset(NodeCapabilityManagedAssetIdParameterValue),
}

/// Closed validation rule for one parameter value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NodeCapabilityParameterConstraint {
    /// Inclusive integer range.
    UnsignedIntegerRange {
        /// Inclusive minimum value.
        minimum: u64,
        /// Inclusive maximum value.
        maximum: u64,
    },
    /// Explicit allowed integer values.
    UnsignedIntegerAllowedValues(BTreeSet<u64>),
    /// Inclusive UTF-8 byte-length bounds.
    TextUtf8Bytes {
        /// Inclusive minimum UTF-8 byte length.
        minimum: usize,
        /// Inclusive maximum UTF-8 byte length.
        maximum: usize,
    },
    /// Explicit allowed choice keys.
    ChoiceAllowedKeys(BTreeSet<NodeCapabilityChoiceKey>),
    /// Any canonical Generation Profile ref boundary value.
    GenerationProfileRef,
    /// Asset identity of the declared media kind.
    ManagedAssetId {
        /// Exact non-Text managed media kind.
        media_kind: WorkflowDataType,
    },
}

impl NodeCapabilityParameterConstraint {
    /// Creates inclusive integer bounds.
    pub const fn unsigned_integer_range(
        minimum: u64,
        maximum: u64,
    ) -> Result<Self, NodeCapabilityContractError> {
        if minimum > maximum {
            Err(NodeCapabilityContractError::InvalidConstraint)
        } else {
            Ok(Self::UnsignedIntegerRange { minimum, maximum })
        }
    }

    /// Creates a non-empty sorted integer allow-list.
    pub fn unsigned_integer_allowed_values(
        values: impl IntoIterator<Item = u64>,
    ) -> Result<Self, NodeCapabilityContractError> {
        let values = values.into_iter().collect::<BTreeSet<_>>();
        if values.is_empty() {
            Err(NodeCapabilityContractError::InvalidConstraint)
        } else {
            Ok(Self::UnsignedIntegerAllowedValues(values))
        }
    }

    /// Creates inclusive UTF-8 byte bounds.
    pub const fn text_utf8_bytes(
        minimum: usize,
        maximum: usize,
    ) -> Result<Self, NodeCapabilityContractError> {
        if minimum > maximum || maximum > u32::MAX as usize {
            Err(NodeCapabilityContractError::InvalidConstraint)
        } else {
            Ok(Self::TextUtf8Bytes { minimum, maximum })
        }
    }

    /// Creates a non-empty sorted choice allow-list.
    pub fn choice_allowed_keys(
        values: impl IntoIterator<Item = NodeCapabilityChoiceKey>,
    ) -> Result<Self, NodeCapabilityContractError> {
        let values = values.into_iter().collect::<BTreeSet<_>>();
        if values.is_empty() {
            Err(NodeCapabilityContractError::InvalidConstraint)
        } else {
            Ok(Self::ChoiceAllowedKeys(values))
        }
    }

    /// Creates an Asset-ID constraint for one media kind.
    pub const fn managed_asset_id(
        media_kind: WorkflowDataType,
    ) -> Result<Self, NodeCapabilityContractError> {
        if matches!(media_kind, WorkflowDataType::Text) {
            Err(NodeCapabilityContractError::InvalidConstraint)
        } else {
            Ok(Self::ManagedAssetId { media_kind })
        }
    }

    /// Validates one parameter value against this exact constraint.
    pub fn validate_parameter_value(
        &self,
        value: &NodeCapabilityParameterValue,
    ) -> Result<(), NodeCapabilityParameterErrorCategory> {
        match (self, value) {
            (
                Self::UnsignedIntegerRange { minimum, maximum },
                NodeCapabilityParameterValue::UnsignedInteger(value),
            ) if (minimum..=maximum).contains(&value) => Ok(()),
            (
                Self::UnsignedIntegerAllowedValues(values),
                NodeCapabilityParameterValue::UnsignedInteger(value),
            ) if values.contains(value) => Ok(()),
            (
                Self::TextUtf8Bytes { minimum, maximum },
                NodeCapabilityParameterValue::Text(value),
            ) if (*minimum..=*maximum).contains(&value.len()) => Ok(()),
            (Self::ChoiceAllowedKeys(values), NodeCapabilityParameterValue::Choice(value))
                if values.contains(value) =>
            {
                Ok(())
            }
            (Self::GenerationProfileRef, NodeCapabilityParameterValue::GenerationProfile(_)) => {
                Ok(())
            }
            (Self::ManagedAssetId { .. }, NodeCapabilityParameterValue::ManagedAsset(_)) => Ok(()),
            (Self::ChoiceAllowedKeys(_), NodeCapabilityParameterValue::Choice(_)) => {
                Err(NodeCapabilityParameterErrorCategory::ParameterChoiceNotDeclared)
            }
            (constraint, value) if constraint.has_same_value_kind(value) => {
                Err(NodeCapabilityParameterErrorCategory::ParameterValueOutOfBounds)
            }
            _ => Err(NodeCapabilityParameterErrorCategory::ParameterValueKindMismatch),
        }
    }

    /// Revalidates directly constructed enum variants.
    pub fn validate_constraint_definition(&self) -> Result<(), NodeCapabilityContractError> {
        let valid = match self {
            Self::UnsignedIntegerRange { minimum, maximum } => minimum <= maximum,
            Self::UnsignedIntegerAllowedValues(values) => !values.is_empty(),
            Self::TextUtf8Bytes { minimum, maximum } => {
                minimum <= maximum && *maximum <= u32::MAX as usize
            }
            Self::ChoiceAllowedKeys(values) => !values.is_empty(),
            Self::GenerationProfileRef => true,
            Self::ManagedAssetId { media_kind } => *media_kind != WorkflowDataType::Text,
        };
        if valid { Ok(()) } else { Err(NodeCapabilityContractError::InvalidConstraint) }
    }

    fn has_same_value_kind(&self, value: &NodeCapabilityParameterValue) -> bool {
        matches!(
            (self, value),
            (
                Self::UnsignedIntegerRange { .. } | Self::UnsignedIntegerAllowedValues(_),
                NodeCapabilityParameterValue::UnsignedInteger(_)
            ) | (Self::TextUtf8Bytes { .. }, NodeCapabilityParameterValue::Text(_))
                | (Self::ChoiceAllowedKeys(_), NodeCapabilityParameterValue::Choice(_))
                | (Self::GenerationProfileRef, NodeCapabilityParameterValue::GenerationProfile(_))
                | (Self::ManagedAssetId { .. }, NodeCapabilityParameterValue::ManagedAsset(_))
        )
    }
}

/// Supplied parameter values keyed by declared identity.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NodeCapabilityParameterSet(
    BTreeMap<NodeCapabilityParameterKey, NodeCapabilityParameterValue>,
);

impl NodeCapabilityParameterSet {
    /// Builds a bounded set from an already unique map.
    pub fn try_from_map(
        values: BTreeMap<NodeCapabilityParameterKey, NodeCapabilityParameterValue>,
    ) -> Result<Self, NodeCapabilityParameterError> {
        if values.len() > 64 {
            return Err(NodeCapabilityParameterError::new(
                NodeCapabilityParameterErrorCategory::ParameterSetTooLarge,
                NodeCapabilityParameterErrorTarget::ParameterSet,
            ));
        }
        Ok(Self(values))
    }

    /// Returns a value by exact parameter key.
    #[must_use]
    pub fn get(&self, key: &NodeCapabilityParameterKey) -> Option<&NodeCapabilityParameterValue> {
        self.0.get(key)
    }

    /// Returns the number of values.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Reports whether no values are present.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
