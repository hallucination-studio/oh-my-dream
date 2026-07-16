//! Contract-validated Workflow runtime values.

use std::collections::BTreeMap;

use super::{
    NodeCapabilityContract, NodeCapabilityInputBindingContract, NodeCapabilityInputKey,
    NodeCapabilityInputRoleKey, NodeCapabilityOutputKey, WorkflowDataType, WorkflowInputItemId,
    WorkflowManagedAssetIdBoundaryValue,
};

/// Exact SHA-256 fingerprint bytes for managed content.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct WorkflowManagedContentFingerprint([u8; 32]);

impl WorkflowManagedContentFingerprint {
    /// Restores exact SHA-256 bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
    /// Returns exact SHA-256 bytes.
    #[must_use]
    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

macro_rules! managed_media_ref {
    ($name:ident, $description:literal) => {
        #[doc = $description]
        #[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
        pub struct $name {
            asset_id: WorkflowManagedAssetIdBoundaryValue,
            content_fingerprint: WorkflowManagedContentFingerprint,
        }

        impl $name {
            #[doc = "Creates a typed managed-media boundary reference."]
            #[must_use]
            pub const fn new(
                asset_id: WorkflowManagedAssetIdBoundaryValue,
                content_fingerprint: WorkflowManagedContentFingerprint,
            ) -> Self {
                Self { asset_id, content_fingerprint }
            }

            #[doc = "Returns the engine Asset-ID boundary value."]
            #[must_use]
            pub const fn asset_id(self) -> WorkflowManagedAssetIdBoundaryValue {
                self.asset_id
            }
            #[doc = "Returns the managed-content fingerprint."]
            #[must_use]
            pub const fn content_fingerprint(self) -> WorkflowManagedContentFingerprint {
                self.content_fingerprint
            }
        }
    };
}

managed_media_ref!(WorkflowManagedImageRef, "Available managed image boundary reference.");
managed_media_ref!(WorkflowManagedVideoRef, "Available managed video boundary reference.");
managed_media_ref!(WorkflowManagedAudioRef, "Available managed audio boundary reference.");

/// One structured text part.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum WorkflowTextPart {
    /// Literal UTF-8 text.
    Literal(String),
    /// Stable reference to another runtime input item.
    InputItemReference(WorkflowInputItemId),
}

/// Non-empty normalized structured text.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct WorkflowTextValue(Vec<WorkflowTextPart>);

impl WorkflowTextValue {
    /// Normalizes literals and enforces part and byte limits.
    pub fn try_new(
        parts: impl IntoIterator<Item = WorkflowTextPart>,
    ) -> Result<Self, WorkflowRuntimeValueError> {
        let mut normalized = Vec::new();
        let mut literal_bytes = 0usize;
        let mut supplied_parts = 0usize;
        for part in parts {
            supplied_parts += 1;
            if supplied_parts > 1_024 {
                return Err(WorkflowRuntimeValueError::TextLimitExceeded);
            }
            match part {
                WorkflowTextPart::Literal(literal) if literal.is_empty() => {}
                WorkflowTextPart::Literal(literal) => {
                    literal_bytes = literal_bytes
                        .checked_add(literal.len())
                        .ok_or(WorkflowRuntimeValueError::TextLimitExceeded)?;
                    if let Some(WorkflowTextPart::Literal(previous)) = normalized.last_mut() {
                        previous.push_str(&literal);
                    } else {
                        normalized.push(WorkflowTextPart::Literal(literal));
                    }
                }
                reference => normalized.push(reference),
            }
            if normalized.len() > 1_024 || literal_bytes > 65_536 {
                return Err(WorkflowRuntimeValueError::TextLimitExceeded);
            }
        }
        if normalized.is_empty() {
            return Err(WorkflowRuntimeValueError::TextEmpty);
        }
        Ok(Self(normalized))
    }

    /// Returns normalized parts in semantic order.
    #[must_use]
    pub fn parts(&self) -> &[WorkflowTextPart] {
        &self.0
    }
}

/// Invalid runtime value or set construction.
#[derive(Clone, Copy, Debug, thiserror::Error, PartialEq, Eq)]
pub enum WorkflowRuntimeValueError {
    /// Structured text had no remaining parts.
    #[error("workflow text is empty")]
    TextEmpty,
    /// Structured text exceeded its frozen part or byte bound.
    #[error("workflow text exceeds its limit")]
    TextLimitExceeded,
    /// An input set did not match the exact capability contract.
    #[error("workflow node input set does not match its capability contract")]
    InvalidInputSet,
    /// An output set did not match the exact capability contract.
    #[error("workflow node output set does not match its capability contract")]
    InvalidOutputSet,
}

/// Closed runtime value union.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum WorkflowRuntimeValue {
    /// Structured text.
    Text(WorkflowTextValue),
    /// Available managed image.
    Image(WorkflowManagedImageRef),
    /// Available managed video.
    Video(WorkflowManagedVideoRef),
    /// Available managed audio.
    Audio(WorkflowManagedAudioRef),
}

impl WorkflowRuntimeValue {
    /// Returns the exact runtime data type.
    #[must_use]
    pub const fn data_type(&self) -> WorkflowDataType {
        match self {
            Self::Text(_) => WorkflowDataType::Text,
            Self::Image(_) => WorkflowDataType::Image,
            Self::Video(_) => WorkflowDataType::Video,
            Self::Audio(_) => WorkflowDataType::Audio,
        }
    }
}

/// One stable runtime input item.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct WorkflowRuntimeInputItem {
    /// Stable item identity.
    pub input_item_id: WorkflowInputItemId,
    /// Capability-owned role for ordered references.
    pub input_role_key: Option<NodeCapabilityInputRoleKey>,
    /// Exact runtime value.
    pub value: WorkflowRuntimeValue,
}

/// Runtime shape of one named node input.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum WorkflowNodeInputValue {
    /// One role-free runtime item.
    Single(WorkflowRuntimeInputItem),
    /// Non-empty role-bearing items in semantic order.
    OrderedReferences(Vec<WorkflowRuntimeInputItem>),
}

/// Complete runtime inputs for one exact capability invocation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowNodeInputSet(BTreeMap<NodeCapabilityInputKey, WorkflowNodeInputValue>);

impl WorkflowNodeInputSet {
    /// Validates all supplied inputs against one exact capability contract.
    pub fn try_new(
        contract: &NodeCapabilityContract,
        values: BTreeMap<NodeCapabilityInputKey, WorkflowNodeInputValue>,
    ) -> Result<Self, WorkflowRuntimeValueError> {
        if values.len() > 64
            || values.keys().any(|key| !contract.inputs().iter().any(|input| input.key() == key))
        {
            return Err(WorkflowRuntimeValueError::InvalidInputSet);
        }
        for input in contract.inputs() {
            match (input.binding(), values.get(input.key())) {
                (NodeCapabilityInputBindingContract::OptionalSingleValue { .. }, None)
                | (
                    NodeCapabilityInputBindingContract::OrderedReferences {
                        minimum_items: 0, ..
                    },
                    None,
                ) => {}
                (
                    NodeCapabilityInputBindingContract::RequiredSingleValue { .. }
                    | NodeCapabilityInputBindingContract::OrderedReferences { .. },
                    None,
                ) => return Err(WorkflowRuntimeValueError::InvalidInputSet),
                (binding, Some(value)) if input_matches(binding, value) => {}
                _ => return Err(WorkflowRuntimeValueError::InvalidInputSet),
            }
        }
        Ok(Self(values))
    }

    /// Returns one named runtime input.
    #[must_use]
    pub fn get(&self, key: &NodeCapabilityInputKey) -> Option<&WorkflowNodeInputValue> {
        self.0.get(key)
    }

    /// Reports whether the validated input set has no values.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns the number of named runtime inputs.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }
}

fn input_matches(
    contract: &NodeCapabilityInputBindingContract,
    value: &WorkflowNodeInputValue,
) -> bool {
    match (contract, value) {
        (
            NodeCapabilityInputBindingContract::OptionalSingleValue { data_type }
            | NodeCapabilityInputBindingContract::RequiredSingleValue { data_type },
            WorkflowNodeInputValue::Single(item),
        ) => item.input_role_key.is_none() && item.value.data_type() == *data_type,
        (
            NodeCapabilityInputBindingContract::OrderedReferences {
                minimum_items,
                maximum_items,
                accepted_data_types_by_role,
            },
            WorkflowNodeInputValue::OrderedReferences(items),
        ) => {
            let count = u32::try_from(items.len()).ok();
            !items.is_empty()
                && count.is_some_and(|count| {
                    count >= *minimum_items && maximum_items.is_none_or(|maximum| count <= maximum)
                })
                && items.iter().all(|item| {
                    item.input_role_key
                        .as_ref()
                        .and_then(|role| accepted_data_types_by_role.get(role))
                        .is_some_and(|types| types.contains(item.value.data_type()))
                })
        }
        _ => false,
    }
}

/// Complete outputs returned by one capability invocation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowNodeOutputSet(BTreeMap<NodeCapabilityOutputKey, WorkflowRuntimeValue>);

impl WorkflowNodeOutputSet {
    /// Validates a complete output map against one exact capability contract.
    pub fn try_new(
        contract: &NodeCapabilityContract,
        values: BTreeMap<NodeCapabilityOutputKey, WorkflowRuntimeValue>,
    ) -> Result<Self, WorkflowRuntimeValueError> {
        let valid = values.len() == contract.outputs().len()
            && !values.is_empty()
            && values.len() <= 64
            && contract.outputs().iter().all(|output| {
                values
                    .get(output.key())
                    .is_some_and(|value| value.data_type() == output.data_type())
            });
        if valid { Ok(Self(values)) } else { Err(WorkflowRuntimeValueError::InvalidOutputSet) }
    }

    /// Restores a previously contract-validated complete output map.
    pub fn try_restore(
        expected_data_types: &BTreeMap<NodeCapabilityOutputKey, WorkflowDataType>,
        values: BTreeMap<NodeCapabilityOutputKey, WorkflowRuntimeValue>,
    ) -> Result<Self, WorkflowRuntimeValueError> {
        let valid = !values.is_empty()
            && values.len() <= 64
            && values.len() == expected_data_types.len()
            && expected_data_types.iter().all(|(key, data_type)| {
                values.get(key).is_some_and(|value| value.data_type() == *data_type)
            });
        if !valid { Err(WorkflowRuntimeValueError::InvalidOutputSet) } else { Ok(Self(values)) }
    }

    /// Returns one named runtime output.
    #[must_use]
    pub fn get(&self, key: &NodeCapabilityOutputKey) -> Option<&WorkflowRuntimeValue> {
        self.0.get(key)
    }
    /// Iterates complete outputs in ascending key order.
    pub fn iter(&self) -> impl Iterator<Item = (&NodeCapabilityOutputKey, &WorkflowRuntimeValue)> {
        self.0.iter()
    }
}
